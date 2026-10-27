//! Inbox merge and record queries (RFC-0001 "Merge").

use std::fs;
use std::time::Instant;

use atlys_adm::{Batch, Deletion, RecordDraft};
use atlys_crypto::kdf::Argon2Params;
use atlys_vault::records::Cursor;
use atlys_vault::{UnlockedVault, Vault, VaultPaths, inbox};
use serde_json::json;

const PASSWORD: &[u8] = b"synthetic correct horse battery";

fn setup() -> (tempfile::TempDir, VaultPaths, UnlockedVault) {
    let dir = tempfile::tempdir().unwrap();
    let paths = VaultPaths::new(dir.path().join("container"));
    let (vault, _) = Vault::create(&paths, PASSWORD, Argon2Params::OWASP_MINIMUM).unwrap();
    (dir, paths, vault)
}

fn steps(source_id: &str, start_ms: i64, value: i64) -> RecordDraft {
    RecordDraft::from_value(&json!({
        "type": "health.quantity",
        "source_id": source_id,
        "start_ms": start_ms,
        "end_ms": start_ms + 60_000,
        "data": { "identifier": "HKQuantityTypeIdentifierStepCount", "value": value, "unit": "count" },
    }))
    .unwrap()
}

fn battery(source_id: &str, start_ms: i64) -> RecordDraft {
    RecordDraft::from_value(&json!({
        "type": "device.battery",
        "source_id": source_id,
        "start_ms": start_ms,
        "data": { "level": 0.5, "state": "unplugged", "low_power_mode": false },
    }))
    .unwrap()
}

fn write(
    paths: &VaultPaths,
    collector: &str,
    records: Vec<RecordDraft>,
    deletions: Vec<Deletion>,
) -> std::path::PathBuf {
    let batch = Batch::new(collector.into(), 1_700_000_000_000, records, deletions).unwrap();
    let path = Vault::inbox_writer(paths).unwrap().write(&batch).unwrap();
    // Distinct, ordered file names.
    std::thread::sleep(std::time::Duration::from_millis(2));
    path
}

fn deletion(source_id: &str) -> Deletion {
    Deletion {
        record_type: "health.quantity".into(),
        source_id: source_id.into(),
    }
}

#[test]
fn merges_upserts_and_flags_deletions() {
    let (_dir, paths, vault) = setup();
    write(
        &paths,
        "health",
        vec![steps("a", 1_000, 10), steps("b", 2_000, 20)],
        vec![],
    );
    write(&paths, "device", vec![battery("bat-1", 3_000)], vec![]);
    let report = vault.merge_inbox(None).unwrap();
    assert_eq!((report.files_merged, report.records_added), (2, 3));
    assert!(inbox::pending(&paths.inbox()).unwrap().is_empty());

    let types = vault.store().record_types().unwrap();
    assert_eq!(types.len(), 2);
    assert_eq!(types[1].record_type, "health.quantity");
    assert_eq!(
        types[1].subtype.as_deref(),
        Some("HKQuantityTypeIdentifierStepCount")
    );
    assert_eq!(
        (
            types[1].count,
            types[1].first_start_ms,
            types[1].last_start_ms
        ),
        (2, 1_000, 2_000)
    );
    let collectors = vault.store().collector_summaries().unwrap();
    assert_eq!(
        collectors
            .iter()
            .map(|c| (c.collector.as_str(), c.count))
            .collect::<Vec<_>>(),
        [("device", 1), ("health", 2)]
    );

    let rows = vault
        .store()
        .records(
            "health.quantity",
            Some("HKQuantityTypeIdentifierStepCount"),
            None,
            10,
        )
        .unwrap();
    assert_eq!(
        rows.iter().map(|r| r.preview.as_str()).collect::<Vec<_>>(),
        ["20 count", "10 count"]
    );
    let id_b = rows[0].id.clone();

    // A newer copy replaces the record and keeps its ID; a deletion flags it.
    write(
        &paths,
        "health",
        vec![steps("b", 2_000, 25)],
        vec![deletion("a"), deletion("never-seen")],
    );
    let report = vault.merge_inbox(None).unwrap();
    assert_eq!(
        (
            report.records_added,
            report.records_updated,
            report.records_deleted_at_source
        ),
        (0, 1, 1)
    );
    let rows = vault
        .store()
        .records(
            "health.quantity",
            Some("HKQuantityTypeIdentifierStepCount"),
            None,
            10,
        )
        .unwrap();
    assert_eq!(rows[0].id, id_b);
    assert_eq!(rows[0].preview, "25 count");
    assert!(rows[1].deleted_at_source && !rows[0].deleted_at_source);
    assert_eq!(
        vault.store().record_count().unwrap(),
        3,
        "deleted records are kept"
    );

    let deleted = vault.store().record(&rows[1].id).unwrap().unwrap();
    let json = deleted.to_json();
    assert_eq!(json["data"]["value"], 10);
    assert!(json["provenance"]["deleted_at_source"].is_string());
    assert_eq!(
        vault.store().record_types().unwrap()[1].deleted_at_source,
        1
    );

    // If the source reports the item again, it is no longer deleted.
    write(&paths, "health", vec![steps("a", 1_000, 10)], vec![]);
    vault.merge_inbox(None).unwrap();
    assert!(
        vault
            .store()
            .record(&rows[1].id)
            .unwrap()
            .unwrap()
            .deleted_at_source_ms
            .is_none()
    );
    assert!(vault.store().record("missing").unwrap().is_none());
}

#[test]
fn replaying_a_merged_file_changes_nothing() {
    let (_dir, paths, vault) = setup();
    let file = write(&paths, "health", vec![steps("a", 1_000, 10)], vec![]);
    let copy = fs::read(&file).unwrap();
    vault.merge_inbox(None).unwrap();
    // A crash after the commit but before the delete leaves the file behind.
    fs::write(&file, &copy).unwrap();
    let report = vault.merge_inbox(None).unwrap();
    assert_eq!(
        (
            report.files_replayed,
            report.files_merged,
            report.records_added
        ),
        (1, 0, 0)
    );
    assert!(!file.exists());
    assert_eq!(vault.store().record_count().unwrap(), 1);
}

#[test]
fn bad_files_are_quarantined_and_newer_ones_wait() {
    let (_dir, paths, vault) = setup();
    write(&paths, "health", vec![steps("a", 1_000, 10)], vec![]);
    fs::write(
        paths.inbox().join("00000000000000000000000000.sealed"),
        b"not an inbox file",
    )
    .unwrap();
    let report = vault.merge_inbox(None).unwrap();
    assert_eq!((report.files_merged, report.files_quarantined), (1, 1));
    assert_eq!(
        inbox::quarantine_counts(&paths.quarantine()).unwrap()["bad-magic"],
        1
    );
    assert_eq!(vault.store().record_count().unwrap(), 1);
}

#[test]
fn deadline_stops_between_files() {
    let (_dir, paths, vault) = setup();
    for i in 0..3 {
        write(
            &paths,
            "device",
            vec![battery(&format!("bat-{i}"), i)],
            vec![],
        );
    }
    let report = vault.merge_inbox(Some(Instant::now())).unwrap();
    assert_eq!((report.files_merged, report.files_remaining), (0, 3));
    let report = vault.merge_inbox(None).unwrap();
    assert_eq!((report.files_merged, report.files_remaining), (3, 0));
}

#[test]
fn pages_are_newest_first_without_gaps_or_repeats() {
    let (_dir, paths, vault) = setup();
    // Ties on start time are broken by ID.
    let records = (0..25)
        .map(|i| battery(&format!("bat-{i}"), 1_000 * (i / 2)))
        .collect();
    write(&paths, "device", records, vec![]);
    vault.merge_inbox(None).unwrap();
    let mut seen = Vec::new();
    let mut cursor: Option<Cursor> = None;
    loop {
        let page = vault
            .store()
            .records("device.battery", None, cursor.as_ref(), 10)
            .unwrap();
        if page.is_empty() {
            break;
        }
        let last = page.last().unwrap();
        cursor = Some(Cursor {
            start_ms: last.start_ms,
            id: last.id.clone(),
        });
        seen.extend(page.into_iter().map(|r| (r.start_ms, r.id)));
    }
    assert_eq!(seen.len(), 25);
    let mut sorted = seen.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    sorted.dedup();
    assert_eq!(sorted, seen);
}

#[test]
fn records_survive_lock_and_unlock() {
    let (_dir, paths, vault) = setup();
    write(&paths, "device", vec![battery("bat-1", 1)], vec![]);
    vault.merge_inbox(None).unwrap();
    vault.lock().unwrap();
    let vault = Vault::unlock_with_password(&paths, PASSWORD).unwrap();
    assert_eq!(vault.store().record_count().unwrap(), 1);
    assert_eq!(vault.store().schema_version().unwrap(), 2);
}

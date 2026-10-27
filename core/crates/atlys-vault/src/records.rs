//! Stored records: the schema-2 tables, inbox merge (RFC-0001 "Merge"), and
//! the read API behind the app's data browser.

use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use atlys_adm::{Batch, Record, RecordDraft, Source};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::fsutil::{read_regular_bounded, sync_dir};
use crate::header::VaultId;
use crate::inbox::{self, InboxError, MAX_FILE_LEN, RejectReason};
use crate::store::{Store, storage};
use crate::{Result, ulid};
use atlys_crypto::hpke::IngestPrivateKey;

/// Tables added in schema version 2.
pub(crate) const SCHEMA_V2: &str = "
CREATE TABLE records (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    subtype TEXT,
    source_id TEXT NOT NULL,
    collector TEXT NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER,
    tz TEXT,
    source TEXT,
    data TEXT NOT NULL,
    ingested_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL,
    deleted_at_source_ms INTEGER,
    UNIQUE (type, source_id)
) STRICT;
CREATE INDEX records_browse ON records (type, subtype, start_ms DESC, id DESC);
CREATE INDEX records_collector ON records (collector, ingested_ms);
CREATE TABLE merge_log (
    sha256 BLOB PRIMARY KEY,
    file TEXT NOT NULL,
    collector TEXT NOT NULL,
    merged_ms INTEGER NOT NULL,
    records INTEGER NOT NULL,
    deletions INTEGER NOT NULL
) STRICT;
";

/// What one [`merge_inbox`] call did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    /// Files applied and deleted.
    pub files_merged: u32,
    /// Files already in the merge log (a crash after commit), deleted without
    /// applying them again.
    pub files_replayed: u32,
    /// Files moved to quarantine.
    pub files_quarantined: u32,
    /// Files from a newer Atlys version, left pending.
    pub files_need_update: u32,
    /// Files left pending after a transient I/O error (for example while the
    /// phone is locked).
    pub files_failed: u32,
    /// Files not reached before the deadline.
    pub files_remaining: u32,
    /// Records inserted.
    pub records_added: u32,
    /// Existing records replaced by a newer copy.
    pub records_updated: u32,
    /// Records newly marked as deleted at the source.
    pub records_deleted_at_source: u32,
}

/// Merges pending inbox files in name (write) order until `deadline`.
/// Each file is applied in one transaction, recorded in the merge log by
/// hash, and deleted only after the commit.
pub fn merge_inbox(
    store: &Store,
    vault_id: &VaultId,
    private_key: &IngestPrivateKey,
    inbox_dir: &Path,
    quarantine_dir: &Path,
    deadline: Option<Instant>,
) -> Result<MergeReport> {
    let mut report = MergeReport::default();
    let files = inbox::pending(inbox_dir)?;
    for (i, path) in files.iter().enumerate() {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            report.files_remaining = (files.len() - i) as u32;
            break;
        }
        let bytes = match read_regular_bounded(path, MAX_FILE_LEN) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                inbox::quarantine(path, quarantine_dir, RejectReason::TooLarge)?;
                report.files_quarantined += 1;
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                inbox::quarantine(path, quarantine_dir, RejectReason::NotRegularFile)?;
                report.files_quarantined += 1;
                continue;
            }
            Err(_) => {
                report.files_failed += 1;
                continue;
            }
        };
        let hash: [u8; 32] = Sha256::digest(&bytes).into();
        if store.merge_logged(&hash)? {
            remove(path)?;
            report.files_replayed += 1;
            continue;
        }
        let batch = match inbox::open_batch(vault_id, private_key, &bytes) {
            Ok(batch) => batch,
            Err(InboxError::Rejected(reason)) => {
                inbox::quarantine(path, quarantine_dir, reason)?;
                report.files_quarantined += 1;
                continue;
            }
            Err(InboxError::NeedsUpdate) => {
                report.files_need_update += 1;
                continue;
            }
            Err(InboxError::Io(_)) => {
                report.files_failed += 1;
                continue;
            }
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        store.apply_batch(&batch, &hash, &name, &mut report)?;
        remove(path)?;
        report.files_merged += 1;
    }
    Ok(report)
}

fn remove(path: &Path) -> Result<()> {
    std::fs::remove_file(path)?;
    if let Some(parent) = path.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

/// Count of records per type and subtype, for the browser's first screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSummary {
    pub record_type: String,
    /// The source's type name inside generic types (see
    /// [`RecordDraft::subtype`]), for example `HKQuantityTypeIdentifierStepCount`.
    pub subtype: Option<String>,
    pub count: u64,
    pub deleted_at_source: u64,
    pub first_start_ms: i64,
    pub last_start_ms: i64,
}

/// Records and the latest merge time per collector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorSummary {
    pub collector: String,
    pub count: u64,
    pub last_ingested_ms: i64,
    pub last_start_ms: i64,
}

/// One row in a record list.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordRow {
    pub id: String,
    pub record_type: String,
    pub subtype: Option<String>,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub tz: Option<String>,
    pub deleted_at_source: bool,
    /// A one-line preview of `data`, for example `42 count`.
    pub preview: String,
}

/// Position after the last row of a page: rows are ordered newest first by
/// `(start_ms, id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub start_ms: i64,
    pub id: String,
}

impl Store {
    fn merge_logged(&self, hash: &[u8; 32]) -> Result<bool> {
        self.conn()
            .query_row(
                "SELECT 1 FROM merge_log WHERE sha256 = ?1",
                params![&hash[..]],
                |_| Ok(()),
            )
            .optional()
            .map(|r| r.is_some())
            .map_err(storage)
    }

    /// Applies one validated batch in a single transaction.
    fn apply_batch(
        &self,
        batch: &Batch,
        hash: &[u8; 32],
        file: &str,
        report: &mut MergeReport,
    ) -> Result<()> {
        let now = now_ms();
        let tx = self.conn().unchecked_transaction().map_err(storage)?;
        for draft in &batch.records {
            upsert(&tx, draft, &batch.collector, now, report)?;
        }
        {
            let mut mark = tx
                .prepare_cached(
                    "UPDATE records SET deleted_at_source_ms = ?3
                     WHERE type = ?1 AND source_id = ?2 AND deleted_at_source_ms IS NULL",
                )
                .map_err(storage)?;
            for deletion in &batch.deletions {
                let changed = mark
                    .execute(params![deletion.record_type, deletion.source_id, now])
                    .map_err(storage)?;
                report.records_deleted_at_source += changed as u32;
            }
        }
        tx.execute(
            "INSERT INTO merge_log (sha256, file, collector, merged_ms, records, deletions)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &hash[..],
                file,
                batch.collector,
                now,
                batch.records.len() as i64,
                batch.deletions.len() as i64
            ],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// Record counts per type and subtype, sorted by type then subtype.
    pub fn record_types(&self) -> Result<Vec<TypeSummary>> {
        let mut stmt = self
            .conn()
            .prepare(
                "SELECT type, subtype, count(*), count(deleted_at_source_ms), min(start_ms), max(start_ms)
                 FROM records GROUP BY type, subtype ORDER BY type, subtype",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(TypeSummary {
                    record_type: r.get(0)?,
                    subtype: r.get(1)?,
                    count: r.get::<_, i64>(2)? as u64,
                    deleted_at_source: r.get::<_, i64>(3)? as u64,
                    first_start_ms: r.get(4)?,
                    last_start_ms: r.get(5)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<rusqlite::Result<_>>().map_err(storage)
    }

    /// Record counts and latest times per collector.
    pub fn collector_summaries(&self) -> Result<Vec<CollectorSummary>> {
        let mut stmt = self
            .conn()
            .prepare(
                "SELECT collector, count(*), max(ingested_ms), max(start_ms)
                 FROM records GROUP BY collector ORDER BY collector",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(CollectorSummary {
                    collector: r.get(0)?,
                    count: r.get::<_, i64>(1)? as u64,
                    last_ingested_ms: r.get(2)?,
                    last_start_ms: r.get(3)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<rusqlite::Result<_>>().map_err(storage)
    }

    /// Total number of records.
    pub fn record_count(&self) -> Result<u64> {
        self.conn()
            .query_row("SELECT count(*) FROM records", [], |r| r.get::<_, i64>(0))
            .map(|n| n as u64)
            .map_err(storage)
    }

    /// A page of records of one type (and subtype, if given), newest first,
    /// starting after `after`.
    pub fn records(
        &self,
        record_type: &str,
        subtype: Option<&str>,
        after: Option<&Cursor>,
        limit: u32,
    ) -> Result<Vec<RecordRow>> {
        // "~" sorts after every ULID character, so the first page starts at the top.
        let (start, id) = after.map_or((i64::MAX, "~"), |c| (c.start_ms, c.id.as_str()));
        let mut stmt = self
            .conn()
            .prepare_cached(
                "SELECT id, type, subtype, start_ms, end_ms, tz, deleted_at_source_ms, data
                 FROM records
                 WHERE type = ?1 AND subtype IS ?2
                   AND (start_ms < ?3 OR (start_ms = ?3 AND id < ?4))
                 ORDER BY start_ms DESC, id DESC
                 LIMIT ?5",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(
                params![record_type, subtype, start, id, i64::from(limit)],
                |r| {
                    let data: String = r.get(7)?;
                    Ok(RecordRow {
                        id: r.get(0)?,
                        record_type: r.get(1)?,
                        subtype: r.get(2)?,
                        start_ms: r.get(3)?,
                        end_ms: r.get(4)?,
                        tz: r.get(5)?,
                        deleted_at_source: r.get::<_, Option<i64>>(6)?.is_some(),
                        preview: preview(&data),
                    })
                },
            )
            .map_err(storage)?;
        rows.collect::<rusqlite::Result<_>>().map_err(storage)
    }

    /// One stored record by ID.
    pub fn record(&self, id: &str) -> Result<Option<Record>> {
        self.conn()
            .query_row(
                "SELECT id, type, source_id, collector, start_ms, end_ms, tz, source, data,
                        ingested_ms, deleted_at_source_ms
                 FROM records WHERE id = ?1",
                params![id],
                |r| {
                    let source: Option<String> = r.get(7)?;
                    let data: String = r.get(8)?;
                    Ok(Record {
                        id: r.get(0)?,
                        collector: r.get(3)?,
                        ingested_ms: r.get(9)?,
                        deleted_at_source_ms: r.get(10)?,
                        draft: RecordDraft {
                            record_type: r.get(1)?,
                            source_id: r.get(2)?,
                            start_ms: r.get(4)?,
                            end_ms: r.get(5)?,
                            tz: r.get(6)?,
                            source: source.as_deref().map(source_from_json),
                            data: serde_json::from_str(&data).unwrap_or_default(),
                        },
                    })
                },
            )
            .optional()
            .map_err(storage)
    }
}

fn upsert(
    tx: &Transaction<'_>,
    draft: &RecordDraft,
    collector: &str,
    now: i64,
    report: &mut MergeReport,
) -> Result<()> {
    let data = serde_json::to_string(&draft.data).expect("JSON values always serialize");
    let source = draft.source.as_ref().map(source_to_json);
    let mut update = tx
        .prepare_cached(
            "UPDATE records SET collector = ?3, start_ms = ?4, end_ms = ?5, tz = ?6, source = ?7,
                    data = ?8, subtype = ?9, updated_ms = ?10, deleted_at_source_ms = NULL
             WHERE type = ?1 AND source_id = ?2",
        )
        .map_err(storage)?;
    let changed = update
        .execute(params![
            draft.record_type,
            draft.source_id,
            collector,
            draft.start_ms,
            draft.end_ms,
            draft.tz,
            source,
            data,
            draft.subtype(),
            now
        ])
        .map_err(storage)?;
    if changed > 0 {
        report.records_updated += 1;
        return Ok(());
    }
    let mut insert = tx
        .prepare_cached(
            "INSERT INTO records (id, type, subtype, source_id, collector, start_ms, end_ms, tz,
                                 source, data, ingested_ms, updated_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        )
        .map_err(storage)?;
    insert
        .execute(params![
            ulid::new()?,
            draft.record_type,
            draft.subtype(),
            draft.source_id,
            collector,
            draft.start_ms,
            draft.end_ms,
            draft.tz,
            source,
            data,
            now
        ])
        .map_err(storage)?;
    report.records_added += 1;
    Ok(())
}

fn source_to_json(source: &Source) -> String {
    let mut out = Map::new();
    for (key, value) in [
        ("name", &source.name),
        ("bundle_id", &source.bundle_id),
        ("device", &source.device),
    ] {
        if let Some(v) = value {
            out.insert(key.into(), Value::String(v.clone()));
        }
    }
    Value::Object(out).to_string()
}

fn source_from_json(text: &str) -> Source {
    let value: Value = serde_json::from_str(text).unwrap_or_default();
    let field = |k: &str| value.get(k).and_then(Value::as_str).map(str::to_owned);
    Source {
        name: field("name"),
        bundle_id: field("bundle_id"),
        device: field("device"),
    }
}

/// A one-line preview of a record's data: `value unit` for quantities, the
/// value name for categories, otherwise the first few scalar fields.
pub(crate) fn preview(data: &str) -> String {
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(data) else {
        return String::new();
    };
    if let (Some(value), Some(Value::String(unit))) = (map.get("value"), map.get("unit")) {
        return format!("{} {unit}", scalar(value));
    }
    if let Some(Value::String(name)) = map.get("value_name") {
        return name.clone();
    }
    if let (Some(lat), Some(lon)) = (map.get("latitude"), map.get("longitude")) {
        return format!("{}, {}", scalar(lat), scalar(lon));
    }
    let parts: Vec<String> = map
        .iter()
        .filter(|(k, v)| *k != "identifier" && !v.is_object() && !v.is_array())
        .filter(|(_, v)| !matches!(v, Value::Bool(false)))
        .take(3)
        .map(|(k, v)| match v {
            Value::Bool(true) => k.clone(),
            _ => format!("{k}: {}", scalar(v)),
        })
        .collect();
    parts.join(", ")
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => {
                let rounded = format!("{f:.3}");
                rounded
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_owned()
            }
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use atlys_crypto::SecretKey;

    use super::preview;
    use crate::store::Store;

    #[test]
    fn schema_1_vaults_gain_the_record_tables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.db");
        let key = SecretKey::try_from_slice(&[7; 32]).unwrap();
        let store = Store::create(&path, &key).unwrap();
        // Roll back to what an M1 build wrote.
        store
            .conn()
            .execute_batch("DROP TABLE records; DROP TABLE merge_log; PRAGMA user_version = 1;")
            .unwrap();
        store.put_meta("kept", b"yes").unwrap();
        store.close().unwrap();
        let store = Store::open(&path, &key).unwrap();
        assert_eq!(store.schema_version().unwrap(), 2);
        assert_eq!(store.record_count().unwrap(), 0);
        assert_eq!(
            store.get_meta("kept").unwrap().as_deref(),
            Some(&b"yes"[..])
        );
    }

    #[test]
    fn previews() {
        assert_eq!(
            preview(
                r#"{"identifier":"HKQuantityTypeIdentifierStepCount","value":42,"unit":"count"}"#
            ),
            "42 count"
        );
        assert_eq!(preview(r#"{"value":0.123456,"unit":"km"}"#), "0.123 km");
        assert_eq!(
            preview(r#"{"identifier":"X","value":3,"value_name":"asleepCore"}"#),
            "asleepCore"
        );
        assert_eq!(
            preview(r#"{"latitude":52.52,"longitude":13.405,"horizontal_accuracy_m":35}"#),
            "52.52, 13.405"
        );
        assert_eq!(
            preview(r#"{"automotive":false,"confidence":2,"walking":true}"#),
            "confidence: 2, walking"
        );
        assert_eq!(preview("not json"), "");
    }
}

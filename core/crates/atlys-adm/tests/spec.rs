//! Keeps the crate, the JSON Schemas, and the synthetic fixtures in
//! `spec/adm/v0/` in agreement (RFC-0001).
use std::collections::BTreeSet;
use std::path::PathBuf;

use atlys_adm::{ADM_VERSION, Batch, Record, RecordDraft, schema, types};
use serde_json::Value;

fn spec_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../spec/adm/v0")
}

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../../spec/adm/v0/fixtures.json")).unwrap()
}

fn list<'a>(fixtures: &'a Value, key: &str) -> &'a Vec<Value> {
    fixtures[key].as_array().unwrap()
}

/// Run with `ATLYS_UPDATE_SCHEMAS=1` to rewrite the files after changing the
/// type table.
#[test]
fn type_schemas_match_the_type_table() {
    let dir = spec_dir().join("types");
    let update = std::env::var_os("ATLYS_UPDATE_SCHEMAS").is_some();
    let mut expected = BTreeSet::new();
    for record_type in types::TYPES {
        let name = schema::file_name(record_type);
        let text = schema::to_file(&schema::render(record_type));
        let path = dir.join(&name);
        if update {
            std::fs::write(&path, &text).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            committed, text,
            "{name} is stale; rerun with ATLYS_UPDATE_SCHEMAS=1"
        );
        expected.insert(name);
    }
    let present: BTreeSet<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        present, expected,
        "spec/adm/v0/types has extra or missing files"
    );
}

#[test]
fn valid_records_cover_every_type() {
    let fixtures = fixtures();
    assert_eq!(fixtures["adm_version"], ADM_VERSION);
    let mut seen = BTreeSet::new();
    for value in list(&fixtures, "valid_records") {
        let draft = RecordDraft::from_value(value).unwrap_or_else(|e| panic!("{value}: {e}"));
        assert_eq!(&draft.to_value(), value, "round trip");
        seen.insert(draft.record_type);
    }
    let all: BTreeSet<_> = types::TYPES.iter().map(|t| t.name.to_owned()).collect();
    assert_eq!(seen, all);
}

#[test]
fn invalid_records_fail_with_the_listed_code_and_path() {
    for case in list(&fixtures(), "invalid_records") {
        let name = case["name"].as_str().unwrap();
        let err = RecordDraft::from_value(&case["record"]).expect_err(name);
        assert_eq!(err.code(), case["error"], "{name}: {err}");
        assert_eq!(err.path, case["path"], "{name}: {err}");
    }
}

#[test]
fn batches() {
    let fixtures = fixtures();
    for value in list(&fixtures, "valid_batches") {
        let bytes = serde_json::to_vec(value).unwrap();
        let batch = Batch::from_json(&bytes).unwrap_or_else(|e| panic!("{value}: {e}"));
        assert_eq!(Batch::from_json(&batch.to_json()).unwrap(), batch);
    }
    for case in list(&fixtures, "invalid_batches") {
        let name = case["name"].as_str().unwrap();
        let err = Batch::from_json(&serde_json::to_vec(&case["batch"]).unwrap()).expect_err(name);
        assert_eq!(err.code(), case["error"], "{name}: {err}");
        assert_eq!(err.path, case["path"], "{name}: {err}");
    }
    let duplicate = fixtures["duplicate_key_batch_text"].as_str().unwrap();
    assert_eq!(
        Batch::from_json(duplicate.as_bytes()).unwrap_err().code(),
        "malformed"
    );
}

#[test]
fn stored_records_render_the_listed_json() {
    for case in list(&fixtures(), "stored_records") {
        let record = Record {
            id: case["id"].as_str().unwrap().into(),
            collector: case["collector"].as_str().unwrap().into(),
            ingested_ms: case["ingested_ms"].as_i64().unwrap(),
            draft: RecordDraft::from_value(&case["draft"]).unwrap(),
        };
        assert_eq!(record.to_json(), case["json"]);
    }
}

//! Batches: the plaintext of one sealed inbox file (RFC-0001, "Batches").

use serde_json::{Map, Value};

use crate::error::{AdmError, ErrorKind};
use crate::record::{Deletion, RecordDraft, integer, is_ident, object, string};
use crate::time::MAX_MS;
use crate::{ADM_VERSION, strict};

/// Longest collector name.
const COLLECTOR_MAX: usize = 32;

/// The records and deletions from one collector run.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// ADM version the batch was written with.
    pub adm_version: String,
    /// Collector that produced the batch, for example `health`.
    pub collector: String,
    /// When the batch was sealed, Unix epoch milliseconds.
    pub created_ms: i64,
    /// New or updated records.
    pub records: Vec<RecordDraft>,
    /// Records to delete. Applied after `records`.
    pub deletions: Vec<Deletion>,
}

impl Batch {
    /// Creates a batch with the current ADM version after validating it.
    pub fn new(
        collector: String,
        created_ms: i64,
        records: Vec<RecordDraft>,
        deletions: Vec<Deletion>,
    ) -> Result<Self, AdmError> {
        let batch = Self {
            adm_version: ADM_VERSION.to_owned(),
            collector,
            created_ms,
            records,
            deletions,
        };
        batch.validate()?;
        Ok(batch)
    }

    /// Validates every part of the batch.
    pub fn validate(&self) -> Result<(), AdmError> {
        check_version(&self.adm_version)?;
        if !is_ident(&self.collector, COLLECTOR_MAX) {
            return Err(AdmError::new(ErrorKind::InvalidValue, "collector"));
        }
        if !(0..=MAX_MS).contains(&self.created_ms) {
            return Err(AdmError::new(ErrorKind::OutOfRange, "created_ms"));
        }
        if self.records.is_empty() && self.deletions.is_empty() {
            return Err(AdmError::new(ErrorKind::InvalidValue, "records"));
        }
        for (i, record) in self.records.iter().enumerate() {
            record
                .validate()
                .map_err(|e| e.within(&format!("records[{i}]")))?;
        }
        for (i, deletion) in self.deletions.iter().enumerate() {
            deletion
                .validate()
                .map_err(|e| e.within(&format!("deletions[{i}]")))?;
        }
        Ok(())
    }

    /// Serializes the batch to JSON.
    pub fn to_json(&self) -> Vec<u8> {
        let mut out = Map::new();
        out.insert("adm_version".into(), self.adm_version.clone().into());
        out.insert("collector".into(), self.collector.clone().into());
        out.insert("created_ms".into(), self.created_ms.into());
        out.insert(
            "records".into(),
            self.records.iter().map(RecordDraft::to_value).collect(),
        );
        if !self.deletions.is_empty() {
            out.insert(
                "deletions".into(),
                self.deletions.iter().map(Deletion::to_value).collect(),
            );
        }
        serde_json::to_vec(&Value::Object(out)).expect("JSON values always serialize")
    }

    /// Parses and validates a batch. A batch from a newer ADM version fails
    /// with [`ErrorKind::UnsupportedVersion`] before anything else is checked.
    pub fn from_json(bytes: &[u8]) -> Result<Self, AdmError> {
        Self::from_value(&strict::parse(bytes)?)
    }

    /// Parses and validates a batch from its JSON value.
    pub fn from_value(value: &Value) -> Result<Self, AdmError> {
        let obj = object(value, "")?;
        let version = obj
            .get("adm_version")
            .ok_or_else(|| AdmError::new(ErrorKind::MissingField, "adm_version"))
            .and_then(|v| string(v, "adm_version"))?;
        check_version(version)?;
        for key in obj.keys() {
            if !matches!(
                key.as_str(),
                "adm_version" | "collector" | "created_ms" | "records" | "deletions"
            ) {
                return Err(AdmError::new(ErrorKind::UnknownField, key.as_str()));
            }
        }
        let field = |name: &'static str| {
            obj.get(name)
                .ok_or_else(|| AdmError::new(ErrorKind::MissingField, name))
        };
        let records = array(field("records")?, "records")?
            .iter()
            .enumerate()
            .map(|(i, v)| {
                RecordDraft::from_value(v).map_err(|e| e.within(&format!("records[{i}]")))
            })
            .collect::<Result<_, _>>()?;
        let deletions = match obj.get("deletions") {
            None => Vec::new(),
            Some(v) => array(v, "deletions")?
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    Deletion::from_value(v).map_err(|e| e.within(&format!("deletions[{i}]")))
                })
                .collect::<Result<_, _>>()?,
        };
        let batch = Self {
            adm_version: version.to_owned(),
            collector: string(field("collector")?, "collector")?.to_owned(),
            created_ms: integer(field("created_ms")?, "created_ms")?,
            records,
            deletions,
        };
        batch.validate()?;
        Ok(batch)
    }
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, AdmError> {
    value
        .as_array()
        .ok_or_else(|| AdmError::new(ErrorKind::WrongType("array"), path))
}

/// Accepts exactly [`ADM_VERSION`]. A well-formed later `major.minor` is
/// unsupported (the batch waits for an app update); anything else is invalid.
fn check_version(version: &str) -> Result<(), AdmError> {
    if version == ADM_VERSION {
        return Ok(());
    }
    match (parse_version(version), parse_version(ADM_VERSION)) {
        (Some(theirs), Some(ours)) if theirs > ours => Err(AdmError::new(
            ErrorKind::UnsupportedVersion(version.to_owned()),
            "adm_version",
        )),
        _ => Err(AdmError::new(ErrorKind::InvalidValue, "adm_version")),
    }
}

fn parse_version(v: &str) -> Option<(u32, u32)> {
    let (major, minor) = v.split_once('.')?;
    let digits = |s: &str| !s.is_empty() && s.len() <= 9 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(major) || !digits(minor) {
        return None;
    }
    Some((major.parse().ok()?, minor.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn steps(source_id: &str) -> RecordDraft {
        RecordDraft::from_value(&json!({
            "type": "health.steps",
            "source_id": source_id,
            "start_ms": 1_700_000_000_000_i64,
            "end_ms": 1_700_000_060_000_i64,
            "data": { "count": 42 }
        }))
        .unwrap()
    }

    #[test]
    fn round_trip() {
        let deletion = Deletion {
            record_type: "health.steps".into(),
            source_id: "gone".into(),
        };
        let batch = Batch::new(
            "health".into(),
            1,
            vec![steps("a"), steps("b")],
            vec![deletion],
        )
        .unwrap();
        assert_eq!(Batch::from_json(&batch.to_json()).unwrap(), batch);
        let only_records = Batch::new("health".into(), 1, vec![steps("a")], vec![]).unwrap();
        assert!(
            !String::from_utf8(only_records.to_json())
                .unwrap()
                .contains("deletions")
        );
    }

    #[test]
    fn errors_carry_the_record_path() {
        let mut bad = steps("a");
        bad.data.insert("count".into(), json!(-1));
        let err = Batch::new("health".into(), 1, vec![steps("a"), bad], vec![]).unwrap_err();
        assert_eq!(err.code(), "out_of_range");
        assert_eq!(err.path, "records[1].data.count");
        assert_eq!(err.to_string(), "records[1].data.count: value out of range");
    }

    #[test]
    fn rejects_empty_batches_and_bad_collectors() {
        assert_eq!(
            Batch::new("health".into(), 1, vec![], vec![])
                .unwrap_err()
                .path,
            "records"
        );
        for bad in [
            "",
            "Health",
            "health!",
            "a_very_long_collector_name_over_32",
        ] {
            assert_eq!(
                Batch::new(bad.into(), 1, vec![steps("a")], vec![])
                    .unwrap_err()
                    .path,
                "collector",
                "{bad}"
            );
        }
    }

    #[test]
    fn versions() {
        assert!(check_version(ADM_VERSION).is_ok());
        assert_eq!(
            check_version("0.2").unwrap_err().code(),
            "unsupported_version"
        );
        assert_eq!(
            check_version("1.0").unwrap_err().code(),
            "unsupported_version"
        );
        for bad in ["0.0", "0-draft", "", "0.1.0", "+1.0", "0.01x"] {
            assert_eq!(
                check_version(bad).unwrap_err().code(),
                "invalid_value",
                "{bad}"
            );
        }
        // A newer batch reports the version even if the rest would not parse.
        let newer =
            br#"{"adm_version":"0.9","collector":"x","created_ms":0,"records":[],"future":1}"#;
        assert_eq!(
            Batch::from_json(newer).unwrap_err().code(),
            "unsupported_version"
        );
    }
}

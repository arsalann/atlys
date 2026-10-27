//! The M0 spike's draft record format (`"0-draft"`). Only the spike app and
//! `atlys_vault::m0` use it; it is deleted with them after the M0 device phase.

use serde::{Deserialize, Serialize};

use crate::error::{AdmError, ErrorKind};

/// Version string of the draft format.
pub const DRAFT_VERSION: &str = "0-draft";

/// A spike record: any `domain.name` type and an untyped payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordDraft {
    #[serde(rename = "type")]
    pub record_type: String,
    pub source_id: String,
    pub start_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<i64>,
    pub data: serde_json::Value,
}

impl RecordDraft {
    /// Checks the envelope fields only.
    pub fn validate(&self) -> Result<(), AdmError> {
        if !is_record_type(&self.record_type) {
            return Err(AdmError::new(ErrorKind::InvalidValue, "type"));
        }
        if self.source_id.is_empty() {
            return Err(AdmError::new(ErrorKind::MissingField, "source_id"));
        }
        if self.end_ms.is_some_and(|end| end < self.start_ms) {
            return Err(AdmError::new(ErrorKind::InvalidTime, "end_ms"));
        }
        Ok(())
    }
}

/// A spike batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    pub adm_version: String,
    pub collector: String,
    pub created_ms: i64,
    pub records: Vec<RecordDraft>,
}

impl Batch {
    /// Creates a batch after validating every record.
    pub fn new(
        collector: String,
        created_ms: i64,
        records: Vec<RecordDraft>,
    ) -> Result<Self, AdmError> {
        if collector.is_empty() {
            return Err(AdmError::new(ErrorKind::MissingField, "collector"));
        }
        for record in &records {
            record.validate()?;
        }
        Ok(Self {
            adm_version: DRAFT_VERSION.to_owned(),
            collector,
            created_ms,
            records,
        })
    }

    /// Serializes the batch to JSON.
    pub fn to_json(&self) -> Result<Vec<u8>, AdmError> {
        serde_json::to_vec(self).map_err(|e| AdmError::malformed(e.to_string()))
    }

    /// Parses and validates a batch from JSON.
    pub fn from_json(bytes: &[u8]) -> Result<Self, AdmError> {
        let batch: Self =
            serde_json::from_slice(bytes).map_err(|e| AdmError::malformed(e.to_string()))?;
        for record in &batch.records {
            record.validate()?;
        }
        Ok(batch)
    }
}

/// `domain.name[.more]`: lowercase ASCII letters, digits, and `_`, with at least one dot.
fn is_record_type(record_type: &str) -> bool {
    let parts: Vec<_> = record_type.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| crate::record::is_ident(p, usize::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_types() {
        for ok in ["health.steps", "device.motion_activity", "a.b.c"] {
            assert!(is_record_type(ok), "{ok}");
        }
        for bad in [
            "",
            "health",
            "Health.steps",
            "health.",
            ".steps",
            "health.steps!",
            "1health.x",
        ] {
            assert!(!is_record_type(bad), "{bad}");
        }
    }

    #[test]
    fn batch_round_trip() {
        let record = RecordDraft {
            record_type: "health.steps".into(),
            source_id: "a".into(),
            start_ms: 1,
            end_ms: Some(2),
            data: serde_json::json!({ "count": 42 }),
        };
        let batch = Batch::new("health".into(), 1, vec![record]).unwrap();
        assert_eq!(Batch::from_json(&batch.to_json().unwrap()).unwrap(), batch);
    }
}

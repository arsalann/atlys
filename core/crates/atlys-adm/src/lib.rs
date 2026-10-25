//! The Atlys Data Model (ADM): record envelopes, record types, and validation.
//!
//! The v0 types are specified in RFC-0001 (milestone M2). JSON Schemas live in
//! `spec/adm/v0/`. Until then this crate defines only the minimal draft
//! record and batch shapes that collectors seal into the inbox.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Version of the ADM implemented by this crate.
pub const ADM_VERSION: &str = "0-draft";

/// Errors from validating or decoding ADM data.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdmError {
    /// A record type is not of the form `domain.name` (lowercase, `_` allowed).
    #[error("invalid record type {0:?}")]
    InvalidRecordType(String),
    /// A required field is empty.
    #[error("empty field: {0}")]
    EmptyField(&'static str),
    /// `end_ms` is before `start_ms`.
    #[error("record ends before it starts")]
    InvalidTimeRange,
    /// The batch could not be encoded or decoded.
    #[error("malformed batch: {0}")]
    Malformed(String),
}

/// A record produced by a collector, before it is merged into the vault.
///
/// Draft shape. RFC-0001 adds provenance, time zone, and schema versioning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordDraft {
    /// Record type, for example `health.steps` or `location.visit`.
    #[serde(rename = "type")]
    pub record_type: String,
    /// Stable identifier from the source (for example a HealthKit UUID), used to dedupe.
    pub source_id: String,
    /// Start time, Unix epoch milliseconds.
    pub start_ms: i64,
    /// End time for interval records, Unix epoch milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<i64>,
    /// Type-specific payload.
    pub data: serde_json::Value,
}

impl RecordDraft {
    /// Checks the envelope fields. Type-specific validation arrives with RFC-0001.
    pub fn validate(&self) -> Result<(), AdmError> {
        validate_record_type(&self.record_type)?;
        if self.source_id.is_empty() {
            return Err(AdmError::EmptyField("source_id"));
        }
        if let Some(end) = self.end_ms
            && end < self.start_ms
        {
            return Err(AdmError::InvalidTimeRange);
        }
        Ok(())
    }
}

/// A batch of records from one collector run. This is the plaintext of an
/// inbox file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    /// ADM version the batch was written with.
    pub adm_version: String,
    /// Collector that produced the batch, for example `health`.
    pub collector: String,
    /// When the batch was sealed, Unix epoch milliseconds.
    pub created_ms: i64,
    /// The records.
    pub records: Vec<RecordDraft>,
}

impl Batch {
    /// Creates a batch with the current ADM version after validating every record.
    pub fn new(
        collector: String,
        created_ms: i64,
        records: Vec<RecordDraft>,
    ) -> Result<Self, AdmError> {
        if collector.is_empty() {
            return Err(AdmError::EmptyField("collector"));
        }
        for record in &records {
            record.validate()?;
        }
        Ok(Self {
            adm_version: ADM_VERSION.to_owned(),
            collector,
            created_ms,
            records,
        })
    }

    /// Serializes the batch to JSON.
    pub fn to_json(&self) -> Result<Vec<u8>, AdmError> {
        serde_json::to_vec(self).map_err(|e| AdmError::Malformed(e.to_string()))
    }

    /// Parses and validates a batch from JSON.
    pub fn from_json(bytes: &[u8]) -> Result<Self, AdmError> {
        let batch: Self =
            serde_json::from_slice(bytes).map_err(|e| AdmError::Malformed(e.to_string()))?;
        for record in &batch.records {
            record.validate()?;
        }
        Ok(batch)
    }
}

/// Checks that a record type looks like `domain.name[.more]`: lowercase ASCII
/// letters, digits, and `_`, with at least one dot.
pub fn validate_record_type(record_type: &str) -> Result<(), AdmError> {
    let invalid = || AdmError::InvalidRecordType(record_type.to_owned());
    let mut parts = 0;
    for part in record_type.split('.') {
        let mut chars = part.chars();
        match chars.next() {
            Some(c) if c.is_ascii_lowercase() => {}
            _ => return Err(invalid()),
        }
        if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(invalid());
        }
        parts += 1;
    }
    if parts < 2 {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn steps(source_id: &str) -> RecordDraft {
        RecordDraft {
            record_type: "health.steps".into(),
            source_id: source_id.into(),
            start_ms: 1_700_000_000_000,
            end_ms: Some(1_700_000_060_000),
            data: json!({ "count": 42 }),
        }
    }

    #[test]
    fn record_types() {
        for ok in [
            "health.steps",
            "location.visit",
            "device.motion_activity",
            "health.sleep_stage",
        ] {
            assert!(validate_record_type(ok).is_ok(), "{ok}");
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
            assert!(validate_record_type(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn batch_round_trip() {
        let batch = Batch::new("health".into(), 1, vec![steps("a"), steps("b")]).unwrap();
        let decoded = Batch::from_json(&batch.to_json().unwrap()).unwrap();
        assert_eq!(decoded, batch);
        assert_eq!(decoded.adm_version, ADM_VERSION);
    }

    #[test]
    fn rejects_invalid_records() {
        assert_eq!(
            Batch::new("health".into(), 1, vec![steps("")]).unwrap_err(),
            AdmError::EmptyField("source_id")
        );
        let mut backwards = steps("a");
        backwards.end_ms = Some(0);
        assert_eq!(
            backwards.validate().unwrap_err(),
            AdmError::InvalidTimeRange
        );
        assert!(Batch::from_json(b"{not json").is_err());
    }
}

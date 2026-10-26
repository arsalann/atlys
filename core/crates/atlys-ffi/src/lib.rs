//! UniFFI surface of the Atlys core.
//!
//! Everything exported here becomes part of the Swift `AtlysCore` module. Keep
//! the surface small: Swift owns platform concerns, Rust owns data and keys.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use atlys_adm::Batch;
use atlys_crypto::hpke::IngestPublicKey;
use atlys_vault::VaultError;
use atlys_vault::m0::inbox::InboxWriter;

#[cfg(feature = "spike")]
pub mod spike;

uniffi::setup_scaffolding!();

/// Errors surfaced to Swift. Messages never contain keys or record contents.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AtlysError {
    /// An argument was malformed.
    #[error("invalid input: {message}")]
    InvalidInput {
        /// What was wrong.
        message: String,
    },
    /// The key is wrong or the data was tampered with.
    #[error("wrong key or tampered data")]
    WrongKeyOrTampered,
    /// A storage or file system operation failed.
    #[error("storage error: {message}")]
    Storage {
        /// Underlying error.
        message: String,
    },
    /// An unexpected internal failure.
    #[error("internal error: {message}")]
    Internal {
        /// Underlying error.
        message: String,
    },
}

impl From<VaultError> for AtlysError {
    fn from(err: VaultError) -> Self {
        use atlys_crypto::CryptoError;
        match err {
            VaultError::WrongKeyOrCorrupt
            | VaultError::WrongCredentials
            | VaultError::IntegrityFailure
            | VaultError::InvalidInboxFile(_)
            | VaultError::Crypto(CryptoError::Decrypt) => Self::WrongKeyOrTampered,
            // Coarse mapping until the product vault is exported (M1.3b).
            VaultError::InvalidHeader(m) | VaultError::WeakPassword(m) => {
                Self::InvalidInput { message: m.into() }
            }
            VaultError::InvalidRecoveryPhrase => Self::InvalidInput {
                message: "invalid recovery phrase".into(),
            },
            e @ (VaultError::UnsupportedVersion
            | VaultError::UnsupportedSchema
            | VaultError::NoVault
            | VaultError::VaultExists
            | VaultError::MissingDatabase
            | VaultError::PartialDeletion
            | VaultError::InboxFull) => Self::Storage {
                message: e.to_string(),
            },
            VaultError::Crypto(CryptoError::InvalidInput(m) | CryptoError::InvalidParams(m)) => {
                Self::InvalidInput { message: m.into() }
            }
            VaultError::Adm(e) => Self::InvalidInput {
                message: e.to_string(),
            },
            VaultError::Storage(m) => Self::Storage { message: m },
            VaultError::Io(e) => Self::Storage {
                message: e.to_string(),
            },
            VaultError::Crypto(e) => Self::Internal {
                message: e.to_string(),
            },
        }
    }
}

impl From<atlys_crypto::CryptoError> for AtlysError {
    fn from(err: atlys_crypto::CryptoError) -> Self {
        VaultError::from(err).into()
    }
}

/// Static information about the linked core library.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CoreInfo {
    /// Version of the Rust core (the workspace version).
    pub version: String,
    /// Version of the Atlys Data Model the core understands.
    pub adm_version: String,
}

/// Returns version information about the core.
#[uniffi::export]
pub fn core_info() -> CoreInfo {
    CoreInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        adm_version: atlys_adm::ADM_VERSION.to_owned(),
    }
}

/// A record from a collector, as Swift builds it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RecordDraft {
    /// Record type, for example `health.steps`.
    pub record_type: String,
    /// Stable identifier from the source, for dedupe.
    pub source_id: String,
    /// Start time, Unix epoch milliseconds.
    pub start_ms: i64,
    /// End time for interval records, Unix epoch milliseconds.
    pub end_ms: Option<i64>,
    /// Type-specific payload as a JSON object.
    pub data_json: String,
}

impl TryFrom<RecordDraft> for atlys_adm::RecordDraft {
    type Error = AtlysError;

    fn try_from(r: RecordDraft) -> Result<Self, AtlysError> {
        let data: serde_json::Value =
            serde_json::from_str(&r.data_json).map_err(|e| AtlysError::InvalidInput {
                message: format!("data_json is not valid JSON: {e}"),
            })?;
        if !data.is_object() {
            return Err(AtlysError::InvalidInput {
                message: "data_json must be a JSON object".into(),
            });
        }
        Ok(Self {
            record_type: r.record_type,
            source_id: r.source_id,
            start_ms: r.start_ms,
            end_ms: r.end_ms,
            data,
        })
    }
}

/// Seals collector output into the inbox. Holds only the ingest **public**
/// key, so it is safe to use in the background while the vault is locked.
#[derive(Debug, uniffi::Object)]
pub struct IngestSealer {
    writer: InboxWriter,
}

#[uniffi::export]
impl IngestSealer {
    /// Creates a sealer that writes into `inbox_dir`.
    #[uniffi::constructor]
    pub fn new(inbox_dir: String, ingest_public_key: Vec<u8>) -> Result<Arc<Self>, AtlysError> {
        let public_key = IngestPublicKey::from_bytes(&ingest_public_key)?;
        Ok(Arc::new(Self {
            writer: InboxWriter::new(inbox_dir, public_key)?,
        }))
    }

    /// Validates and seals one batch. Returns the new file's name.
    pub fn seal(&self, collector: String, records: Vec<RecordDraft>) -> Result<String, AtlysError> {
        let records = records
            .into_iter()
            .map(atlys_adm::RecordDraft::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let batch = Batch::new(collector, now_ms(), records)?;
        let path = self.writer.write(&batch)?;
        Ok(path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default())
    }
}

impl From<atlys_adm::AdmError> for AtlysError {
    fn from(err: atlys_adm::AdmError) -> Self {
        Self::InvalidInput {
            message: err.to_string(),
        }
    }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use atlys_crypto::hpke::IngestKeyPair;
    use atlys_vault::m0::inbox;

    use super::*;

    #[test]
    fn core_info_reports_versions() {
        let info = core_info();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.adm_version, atlys_adm::ADM_VERSION);
    }

    #[test]
    fn sealer_writes_files_only_the_private_key_opens() {
        let dir = tempfile::tempdir().unwrap();
        let kp = IngestKeyPair::generate();
        let sealer = IngestSealer::new(
            dir.path().to_string_lossy().into(),
            kp.public.to_bytes().to_vec(),
        )
        .unwrap();
        let name = sealer
            .seal(
                "device".into(),
                vec![RecordDraft {
                    record_type: "device.battery".into(),
                    source_id: "synthetic-1".into(),
                    start_ms: 1,
                    end_ms: None,
                    data_json: r#"{"level":0.5}"#.into(),
                }],
            )
            .unwrap();
        assert!(name.ends_with(".sealed"));
        let batch = inbox::read(&kp.private, &dir.path().join(&name)).unwrap();
        assert_eq!(batch.collector, "device");
        assert_eq!(batch.records[0].data["level"], 0.5);
    }

    #[test]
    fn sealer_rejects_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            IngestSealer::new(dir.path().to_string_lossy().into(), vec![0; 5]),
            Err(AtlysError::InvalidInput { .. })
        ));
        let kp = IngestKeyPair::generate();
        let sealer = IngestSealer::new(
            dir.path().to_string_lossy().into(),
            kp.public.to_bytes().to_vec(),
        )
        .unwrap();
        let bad = |record_type: &str, json: &str| RecordDraft {
            record_type: record_type.into(),
            source_id: "s".into(),
            start_ms: 0,
            end_ms: None,
            data_json: json.into(),
        };
        for draft in [
            bad("health.steps", "not json"),
            bad("health.steps", "[1]"),
            bad("Steps", "{}"),
        ] {
            assert!(matches!(
                sealer.seal("health".into(), vec![draft]),
                Err(AtlysError::InvalidInput { .. })
            ));
        }
    }
}

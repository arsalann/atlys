//! UniFFI surface of the Atlys core.
//!
//! Everything exported here becomes part of the Swift `AtlysCore` module. Keep
//! the surface small: Swift owns platform concerns, Rust owns data and keys.
//! Vault keys never cross this boundary; for Face ID, Swift passes only the
//! Secure Enclave ECDH shared secret.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use atlys_adm::Batch;
use atlys_vault::inbox::InboxWriter;
use atlys_vault::{Vault, VaultError, VaultPaths};

#[cfg(feature = "spike")]
pub mod spike;
mod vault;

pub use vault::*;

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
    /// The password, recovery phrase, or Face ID key did not unlock the vault.
    #[error("wrong password or recovery phrase")]
    WrongCredentials,
    /// The vault metadata failed its integrity check after unlocking.
    #[error("vault integrity check failed")]
    IntegrityFailure,
    /// The new password does not meet the strength policy.
    #[error("password too weak: {message}")]
    WeakPassword {
        /// Why it was rejected.
        message: String,
    },
    /// The recovery phrase has the wrong length, an unknown word, or a bad
    /// checksum.
    #[error("invalid recovery phrase")]
    InvalidRecoveryPhrase,
    /// No vault exists at the given location.
    #[error("no vault found")]
    NoVault,
    /// A vault already exists at the given location.
    #[error("a vault already exists")]
    VaultExists,
    /// The vault was written by a newer version of Atlys.
    #[error("this vault needs a newer version of Atlys")]
    UpdateRequired,
    /// The session was already locked.
    #[error("the vault is locked")]
    Locked,
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
            VaultError::WrongCredentials => Self::WrongCredentials,
            VaultError::IntegrityFailure => Self::IntegrityFailure,
            VaultError::WeakPassword(m) => Self::WeakPassword { message: m.into() },
            VaultError::InvalidRecoveryPhrase => Self::InvalidRecoveryPhrase,
            VaultError::NoVault => Self::NoVault,
            VaultError::VaultExists => Self::VaultExists,
            VaultError::UnsupportedVersion | VaultError::UnsupportedSchema => Self::UpdateRequired,
            VaultError::WrongKeyOrCorrupt
            | VaultError::InvalidInboxFile(_)
            | VaultError::Crypto(CryptoError::Decrypt) => Self::WrongKeyOrTampered,
            VaultError::InvalidHeader(m)
            | VaultError::Crypto(CryptoError::InvalidInput(m) | CryptoError::InvalidParams(m)) => {
                Self::InvalidInput { message: m.into() }
            }
            VaultError::Adm(e) => Self::InvalidInput {
                message: e.to_string(),
            },
            e @ (VaultError::MissingDatabase
            | VaultError::PartialDeletion
            | VaultError::InboxFull) => Self::Storage {
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

impl From<atlys_adm::AdmError> for AtlysError {
    fn from(err: atlys_adm::AdmError) -> Self {
        Self::InvalidInput {
            message: err.to_string(),
        }
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

pub(crate) fn drafts(records: Vec<RecordDraft>) -> Result<Vec<atlys_adm::RecordDraft>, AtlysError> {
    records
        .into_iter()
        .map(atlys_adm::RecordDraft::try_from)
        .collect()
}

/// Seals collector output into the vault's inbox (format version 2). Holds
/// only the vault ID and ingest **public** key from the header, so it is safe
/// to use in the background while the vault is locked.
#[derive(Debug, uniffi::Object)]
pub struct IngestSealer {
    writer: InboxWriter,
}

#[uniffi::export]
impl IngestSealer {
    /// Creates a sealer for the vault in `vault_root`.
    #[uniffi::constructor]
    pub fn open(vault_root: String) -> Result<Arc<Self>, AtlysError> {
        Ok(Arc::new(Self {
            writer: Vault::inbox_writer(&VaultPaths::new(vault_root))?,
        }))
    }

    /// Validates and seals one batch. Returns the new file's name.
    pub fn seal(&self, collector: String, records: Vec<RecordDraft>) -> Result<String, AtlysError> {
        let batch = Batch::new(collector, now_ms(), drafts(records)?)?;
        let path = self.writer.write(&batch)?;
        Ok(path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default())
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
    use super::*;

    #[test]
    fn core_info_reports_versions() {
        let info = core_info();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.adm_version, atlys_adm::ADM_VERSION);
    }

    #[test]
    fn sealer_needs_a_vault() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            IngestSealer::open(dir.path().to_string_lossy().into()),
            Err(AtlysError::NoVault)
        ));
    }
}

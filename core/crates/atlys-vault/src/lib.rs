//! The Atlys vault: encrypted store, sealed ingest inbox, and unlock flows.
//!
//! The vault format and key hierarchy are specified in RFC-0002 (milestone
//! M1). The on-disk formats in this crate are provisional until then.

#![forbid(unsafe_code)]

pub mod envelope;
pub mod inbox;
pub mod store;
mod ulid;

use atlys_adm::AdmError;
use atlys_crypto::CryptoError;

/// Errors from vault operations. Messages never include keys or record contents.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    /// The database key is wrong, or the file is not an Atlys database.
    #[error("wrong key or not a vault database")]
    WrongKeyOrCorrupt,
    /// A cryptographic operation failed.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    /// Record data failed validation.
    #[error(transparent)]
    Adm(#[from] AdmError),
    /// An inbox file is malformed or failed authentication.
    #[error("invalid inbox file: {0}")]
    InvalidInboxFile(&'static str),
    /// SQLite reported an error.
    #[error("storage error: {0}")]
    Storage(String),
    /// A file system operation failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for this crate.
pub type Result<T> = core::result::Result<T, VaultError>;

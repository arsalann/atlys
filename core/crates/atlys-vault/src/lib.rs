//! The Atlys vault: encrypted store, sealed ingest inbox, and unlock flows.
//!
//! Formats and lifecycle follow RFC-0002 (`spec/rfcs/0002-vault-format.md`):
//!
//! | Module | Contents |
//! | --- | --- |
//! | [`header`] | `vault/header.json`: strict parsing, encoding, MAC verification |
//! | [`envelope`] | Password, recovery, ingest, and device (Face ID) key envelopes |
//! | [`inbox`] | Sealed inbox version 2 and quarantine |
//! | [`store`] | The SQLCipher database |
//! | [`records`] | Stored records, inbox merge, and the read API (RFC-0001) |
//! | [`vault`] | Create, unlock, recover, lock, change credentials, delete |
//! | [`password`] | New-password policy and strength feedback (zxcvbn) |
//! | [`recovery`] | 24-word recovery phrase (BIP-39 English wordlist) |
//!
//! [`m0`] holds the throwaway spike formats until the FFI moves to the product
//! vault.

#![forbid(unsafe_code)]

pub mod envelope;
mod fsutil;
pub mod header;
mod hex;
pub mod inbox;
pub mod m0;
pub mod password;
pub mod records;
pub mod recovery;
pub mod store;
mod ulid;
pub mod vault;

use atlys_adm::AdmError;
use atlys_crypto::CryptoError;

pub use header::VaultId;
pub use recovery::RecoverySecret;
pub use vault::{UnlockedVault, Vault, VaultPaths};

/// Errors from vault operations. Messages never include keys or record contents.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    /// The database key is wrong, or the file is not an Atlys database.
    #[error("wrong key or not a vault database")]
    WrongKeyOrCorrupt,
    /// The password, recovery phrase, or device key did not unlock the vault,
    /// or the envelope is corrupt. Deliberately one error for both.
    #[error("wrong password or recovery phrase, or corrupt vault")]
    WrongCredentials,
    /// The header failed authentication after a key was unwrapped: the vault
    /// metadata was modified. Stop collectors and report it to the user.
    #[error("vault integrity check failed")]
    IntegrityFailure,
    /// The vault header is malformed.
    #[error("invalid vault header: {0}")]
    InvalidHeader(&'static str),
    /// The vault was written by a newer version of Atlys.
    #[error("unsupported vault format version; update Atlys")]
    UnsupportedVersion,
    /// There is no vault at the given location.
    #[error("no vault found")]
    NoVault,
    /// A vault already exists at the given location.
    #[error("a vault already exists")]
    VaultExists,
    /// The header exists but the database is missing.
    #[error("vault database is missing")]
    MissingDatabase,
    /// The database schema is newer than this version of Atlys supports.
    #[error("unsupported database schema version; update Atlys")]
    UnsupportedSchema,
    /// The new password does not meet the strength policy.
    #[error("password does not meet the strength policy: {0}")]
    WeakPassword(&'static str),
    /// The recovery phrase has the wrong length, an unknown word, or a bad
    /// checksum.
    #[error("invalid recovery phrase")]
    InvalidRecoveryPhrase,
    /// Deleting the vault removed some files but not all. Retrying is safe.
    #[error("vault deletion incomplete; retry")]
    PartialDeletion,
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
    /// The inbox and quarantine together exceed their storage budget.
    /// Collection should pause with a visible status; nothing is deleted.
    #[error("inbox storage budget exceeded")]
    InboxFull,
    /// A file system operation failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for this crate.
pub type Result<T> = core::result::Result<T, VaultError>;

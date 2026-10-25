//! Cryptographic primitives for the Atlys vault.
//!
//! This crate wraps well-reviewed RustCrypto and HPKE implementations behind a
//! small, misuse-resistant API. It contains no custom cryptography.
//!
//! | Purpose | Primitive | Module |
//! | --- | --- | --- |
//! | Master password stretching | Argon2id (RFC 9106) | [`kdf`] |
//! | Subkey derivation | HKDF-SHA256 (RFC 5869) | [`hkdf`] |
//! | Envelopes, blobs | XChaCha20-Poly1305 | [`aead`] |
//! | Write-only ingest inbox | HPKE base mode, X25519 + HKDF-SHA256 + ChaCha20-Poly1305 (RFC 9180) | [`hpke`] |
//!
//! Key material lives in types that zeroize on drop ([`SecretKey`],
//! [`hpke::IngestPrivateKey`]) and never implement `Debug` with their contents.
//!
//! Formats, labels, and parameters are provisional until RFC-0002 (vault
//! format and key hierarchy) is merged in milestone M1.

#![forbid(unsafe_code)]

pub mod aead;
pub mod hkdf;
pub mod hpke;
pub mod kdf;
mod secret;

pub use secret::{KEY_LEN, SecretKey, random_bytes};

/// Errors from cryptographic operations.
///
/// Messages never include key material or plaintext.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CryptoError {
    /// Authentication failed: wrong key, wrong associated data, or tampered data.
    #[error("decryption failed: wrong key or tampered data")]
    Decrypt,
    /// An input had the wrong length or encoding.
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    /// Key derivation parameters are outside the accepted range.
    #[error("invalid key derivation parameters: {0}")]
    InvalidParams(&'static str),
    /// The operating system's random number generator failed.
    #[error("random number generator failure")]
    Rng,
    /// An internal primitive failed unexpectedly.
    #[error("internal cryptographic failure")]
    Internal,
}

/// Result type for this crate.
pub type Result<T> = core::result::Result<T, CryptoError>;

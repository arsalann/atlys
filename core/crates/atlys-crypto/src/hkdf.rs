//! Subkey derivation with HKDF-SHA256 (RFC 5869).
//!
//! Every derived key has its own fixed, versioned label, so keys for different
//! purposes are independent even though they come from the same vault key.

use hkdf::Hkdf;
use sha2::Sha256;

use crate::{CryptoError, KEY_LEN, Result, SecretKey};

/// A domain-separation label used as the HKDF `info` input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label(&'static str);

impl Label {
    /// The label as bytes.
    pub fn as_bytes(&self) -> &'static [u8] {
        self.0.as_bytes()
    }
}

/// Labels for keys derived from the vault key. Provisional until RFC-0002.
pub mod labels {
    use super::Label;

    /// SQLCipher raw key for `vault.db`.
    pub const SQLCIPHER: Label = Label("atlys/v1/sqlcipher");
    /// Encryption key for files in `blobs/`.
    pub const BLOB: Label = Label("atlys/v1/blob");
    /// MAC key for the vault header.
    pub const HEADER_MAC: Label = Label("atlys/v1/header-mac");
    /// Wrapping key for the ingest private key.
    pub const INGEST_WRAP: Label = Label("atlys/v1/ingest-wrap");
    /// Key-encryption key from the 256-bit recovery phrase.
    pub const RECOVERY: Label = Label("atlys/v1/recovery");
    /// Key-encryption key from a Secure Enclave ECDH shared secret.
    pub const SECURE_ENCLAVE_ENVELOPE: Label = Label("atlys/v1/envelope/secure-enclave");
}

/// Derives a purpose-specific key from a 256-bit key (no salt).
///
/// Use for keys derived from the vault key, which is already uniformly random.
pub fn derive_subkey(ikm: &SecretKey, label: Label) -> SecretKey {
    derive(None, ikm.expose_secret(), label).expect("32-byte HKDF-SHA256 output is always valid")
}

/// Derives a key from non-uniform input keying material, such as an ECDH
/// shared secret, with a context-binding salt.
pub fn derive_from_shared_secret(
    shared_secret: &[u8],
    salt: &[u8],
    label: Label,
) -> Result<SecretKey> {
    if shared_secret.len() < 32 {
        return Err(CryptoError::InvalidInput(
            "shared secret must be at least 32 bytes",
        ));
    }
    derive(Some(salt), shared_secret, label)
}

fn derive(salt: Option<&[u8]>, ikm: &[u8], label: Label) -> Result<SecretKey> {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut okm = [0u8; KEY_LEN];
    hk.expand(label.as_bytes(), &mut okm)
        .map_err(|_| CryptoError::Internal)?;
    Ok(SecretKey::from_bytes(okm))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression vectors computed with an independent HKDF implementation
    // (Python standard library `hmac` + `hashlib`, RFC 5869 extract/expand).
    #[test]
    fn subkey_matches_reference() {
        let mut ikm = [0u8; 32];
        for (i, b) in ikm.iter_mut().enumerate() {
            *b = i as u8;
        }
        let key = derive_subkey(&SecretKey::from_bytes(ikm), labels::SQLCIPHER);
        assert_eq!(
            hex::encode(key.expose_secret()),
            "65e7a3d4974536f3736f24a62298f72c4724e15e61d639568bfd18e4c1da2255"
        );
    }

    #[test]
    fn shared_secret_derivation_matches_reference() {
        let mut salt = vec![4u8; 65];
        salt.extend_from_slice(&[5u8; 65]);
        let key =
            derive_from_shared_secret(&[7u8; 32], &salt, labels::SECURE_ENCLAVE_ENVELOPE).unwrap();
        assert_eq!(
            hex::encode(key.expose_secret()),
            "36b584cd129b46122e5a9422dd418e4b17eb222fe76233620d7a30a04b592005"
        );
    }

    #[test]
    fn labels_separate_domains() {
        let root = SecretKey::from_bytes([9u8; 32]);
        let all = [
            labels::SQLCIPHER,
            labels::BLOB,
            labels::HEADER_MAC,
            labels::INGEST_WRAP,
            labels::RECOVERY,
            labels::SECURE_ENCLAVE_ENVELOPE,
        ];
        let keys: Vec<_> = all.iter().map(|l| derive_subkey(&root, *l)).collect();
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert!(!keys[i].ct_eq(&keys[j]), "{:?} vs {:?}", all[i], all[j]);
            }
        }
    }

    #[test]
    fn rejects_short_shared_secret() {
        assert!(derive_from_shared_secret(&[1u8; 16], b"salt", labels::RECOVERY).is_err());
    }
}

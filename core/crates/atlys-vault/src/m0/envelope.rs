//! Key envelopes: encrypted copies of the vault key, one per unlock method.
//!
//! M0 format, superseded by the RFC-0002 device envelope (version 2) in
//! [`crate::envelope`]. This module holds the Secure Enclave
//! (Face ID) envelope explored in M0 task 0.7.
//!
//! ## Secure Enclave envelope
//!
//! The Secure Enclave can only do P-256 operations on keys it holds, and it
//! never exports private keys. Instead of handing the raw vault key to Swift
//! for ECIES (the original plan), the split is:
//!
//! - **Swift** (CryptoKit) generates an ephemeral P-256 key and performs ECDH
//!   with the Secure Enclave key's public half (wrap), or asks the Secure
//!   Enclave to perform ECDH with the stored ephemeral public key (unwrap,
//!   gated by Face ID). Swift sees only the 32-byte ECDH shared secret.
//! - **Rust** derives a key-encryption key with HKDF-SHA256 (salt = both
//!   public keys, label `atlys/v1/envelope/secure-enclave`) and wraps or
//!   unwraps the vault key with XChaCha20-Poly1305.
//!
//! The vault key never crosses the FFI boundary.
//!
//! ```text
//! offset  size  field
//! 0       1     version (1)
//! 1       65    ephemeral public key (X9.63 uncompressed P-256)
//! 66      65    Secure Enclave public key (X9.63 uncompressed P-256)
//! 131     72    XChaCha20-Poly1305(KEK, vault key), AAD = bytes 0..131
//! ```

use atlys_crypto::hkdf::{derive_from_shared_secret, labels};
use atlys_crypto::{SecretKey, aead};

use crate::{Result, VaultError};

/// Envelope format version.
pub const SECURE_ENCLAVE_ENVELOPE_VERSION: u8 = 1;
/// Length of an X9.63 uncompressed P-256 public key.
pub const P256_PUBLIC_KEY_LEN: usize = 65;
const HEADER_LEN: usize = 1 + 2 * P256_PUBLIC_KEY_LEN;
/// Total envelope length.
pub const SECURE_ENCLAVE_ENVELOPE_LEN: usize = HEADER_LEN + aead::OVERHEAD + atlys_crypto::KEY_LEN;

/// Wraps `vault_key` using an ECDH shared secret computed by the platform.
pub fn seal_secure_enclave(
    vault_key: &SecretKey,
    shared_secret: &[u8],
    ephemeral_public_key: &[u8],
    enclave_public_key: &[u8],
) -> Result<Vec<u8>> {
    check_p256_public_key(ephemeral_public_key)?;
    check_p256_public_key(enclave_public_key)?;
    let mut header = Vec::with_capacity(SECURE_ENCLAVE_ENVELOPE_LEN);
    header.push(SECURE_ENCLAVE_ENVELOPE_VERSION);
    header.extend_from_slice(ephemeral_public_key);
    header.extend_from_slice(enclave_public_key);
    let kek = kek(shared_secret, &header[1..])?;
    let wrapped = aead::wrap_key(&kek, vault_key, &header)?;
    header.extend_from_slice(&wrapped);
    Ok(header)
}

/// Returns the ephemeral public key the platform needs for ECDH on unwrap.
pub fn secure_enclave_ephemeral_key(envelope: &[u8]) -> Result<&[u8]> {
    check_envelope(envelope)?;
    Ok(&envelope[1..1 + P256_PUBLIC_KEY_LEN])
}

/// Unwraps the vault key using the shared secret the Secure Enclave computed.
pub fn open_secure_enclave(envelope: &[u8], shared_secret: &[u8]) -> Result<SecretKey> {
    check_envelope(envelope)?;
    let (header, wrapped) = envelope.split_at(HEADER_LEN);
    let kek = kek(shared_secret, &header[1..])?;
    Ok(aead::unwrap_key(&kek, wrapped, header)?)
}

fn kek(shared_secret: &[u8], public_keys: &[u8]) -> Result<SecretKey> {
    if shared_secret.len() != 32 {
        return Err(VaultError::Crypto(atlys_crypto::CryptoError::InvalidInput(
            "P-256 shared secret must be 32 bytes",
        )));
    }
    Ok(derive_from_shared_secret(
        shared_secret,
        public_keys,
        labels::SECURE_ENCLAVE_ENVELOPE,
    )?)
}

fn check_envelope(envelope: &[u8]) -> Result<()> {
    if envelope.len() != SECURE_ENCLAVE_ENVELOPE_LEN {
        return Err(VaultError::Crypto(atlys_crypto::CryptoError::InvalidInput(
            "wrong Secure Enclave envelope length",
        )));
    }
    if envelope[0] != SECURE_ENCLAVE_ENVELOPE_VERSION {
        return Err(VaultError::Crypto(atlys_crypto::CryptoError::InvalidInput(
            "unsupported Secure Enclave envelope version",
        )));
    }
    Ok(())
}

fn check_p256_public_key(key: &[u8]) -> Result<()> {
    if key.len() != P256_PUBLIC_KEY_LEN || key[0] != 0x04 {
        return Err(VaultError::Crypto(atlys_crypto::CryptoError::InvalidInput(
            "expected an X9.63 uncompressed P-256 public key",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(byte: u8) -> Vec<u8> {
        let mut k = vec![byte; P256_PUBLIC_KEY_LEN];
        k[0] = 0x04;
        k
    }

    #[test]
    fn round_trip() {
        let vault_key = SecretKey::generate().unwrap();
        let env = seal_secure_enclave(&vault_key, &[7u8; 32], &pk(1), &pk(2)).unwrap();
        assert_eq!(env.len(), SECURE_ENCLAVE_ENVELOPE_LEN);
        assert_eq!(secure_enclave_ephemeral_key(&env).unwrap(), &pk(1)[..]);
        assert!(
            open_secure_enclave(&env, &[7u8; 32])
                .unwrap()
                .ct_eq(&vault_key)
        );
    }

    #[test]
    fn rejects_wrong_secret_and_tampering() {
        let vault_key = SecretKey::generate().unwrap();
        let env = seal_secure_enclave(&vault_key, &[7u8; 32], &pk(1), &pk(2)).unwrap();
        assert!(open_secure_enclave(&env, &[8u8; 32]).is_err());
        for pos in [0usize, 1, 70, 131, 160, SECURE_ENCLAVE_ENVELOPE_LEN - 1] {
            let mut t = env.clone();
            t[pos] ^= 1;
            assert!(open_secure_enclave(&t, &[7u8; 32]).is_err(), "byte {pos}");
        }
        assert!(seal_secure_enclave(&vault_key, &[7u8; 32], &[4u8; 33], &pk(2)).is_err());
        assert!(seal_secure_enclave(&vault_key, &[7u8; 16], &pk(1), &pk(2)).is_err());
    }
}

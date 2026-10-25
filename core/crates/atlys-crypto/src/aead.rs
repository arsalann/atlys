//! Authenticated encryption with XChaCha20-Poly1305.
//!
//! Output format: `nonce (24 bytes) || ciphertext || tag (16 bytes)`. The
//! 192-bit nonce is random, so there is no nonce bookkeeping and no realistic
//! risk of reuse.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::{CryptoError, Result, SecretKey, random_bytes};

/// Nonce length in bytes.
pub const NONCE_LEN: usize = 24;
/// Authentication tag length in bytes.
pub const TAG_LEN: usize = 16;
/// Bytes added to every plaintext.
pub const OVERHEAD: usize = NONCE_LEN + TAG_LEN;

/// Encrypts and authenticates `plaintext`, binding it to `aad`.
pub fn seal(key: &SecretKey, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.expose_secret().into());
    let nonce_bytes = random_bytes::<NONCE_LEN>()?;
    let nonce = XNonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::Internal)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Verifies and decrypts output from [`seal`]. The plaintext is zeroized on drop.
pub fn open(key: &SecretKey, sealed: &[u8], aad: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if sealed.len() < OVERHEAD {
        return Err(CryptoError::Decrypt);
    }
    let (nonce_bytes, ciphertext) = sealed.split_at(NONCE_LEN);
    let nonce = XNonce::try_from(nonce_bytes).map_err(|_| CryptoError::Decrypt)?;
    let cipher = XChaCha20Poly1305::new(key.expose_secret().into());
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

/// Encrypts one key under another (an "envelope").
pub fn wrap_key(kek: &SecretKey, key: &SecretKey, aad: &[u8]) -> Result<Vec<u8>> {
    seal(kek, key.expose_secret(), aad)
}

/// Decrypts an envelope produced by [`wrap_key`].
pub fn unwrap_key(kek: &SecretKey, wrapped: &[u8], aad: &[u8]) -> Result<SecretKey> {
    let bytes = open(kek, wrapped, aad)?;
    SecretKey::try_from_slice(&bytes).map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn key(byte: u8) -> SecretKey {
        SecretKey::from_bytes([byte; 32])
    }

    #[test]
    fn round_trip_and_overhead() {
        let sealed = seal(&key(1), b"synthetic record", b"aad").unwrap();
        assert_eq!(sealed.len(), b"synthetic record".len() + OVERHEAD);
        assert_eq!(
            &open(&key(1), &sealed, b"aad").unwrap()[..],
            b"synthetic record"
        );
    }

    #[test]
    fn nonces_are_unique() {
        let a = seal(&key(1), b"same", b"").unwrap();
        let b = seal(&key(1), b"same", b"").unwrap();
        assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN]);
    }

    #[test]
    fn rejects_wrong_key_wrong_aad_and_truncation() {
        let sealed = seal(&key(1), b"data", b"aad").unwrap();
        assert_eq!(
            open(&key(2), &sealed, b"aad").unwrap_err(),
            CryptoError::Decrypt
        );
        assert_eq!(
            open(&key(1), &sealed, b"other").unwrap_err(),
            CryptoError::Decrypt
        );
        assert_eq!(
            open(&key(1), &sealed[..10], b"aad").unwrap_err(),
            CryptoError::Decrypt
        );
        assert_eq!(
            open(&key(1), &[], b"aad").unwrap_err(),
            CryptoError::Decrypt
        );
    }

    #[test]
    fn key_wrap_round_trip() {
        let vault_key = SecretKey::generate().unwrap();
        let wrapped = wrap_key(&key(3), &vault_key, b"envelope/password").unwrap();
        let unwrapped = unwrap_key(&key(3), &wrapped, b"envelope/password").unwrap();
        assert!(unwrapped.ct_eq(&vault_key));
        assert!(unwrap_key(&key(3), &wrapped, b"envelope/recovery").is_err());
    }

    proptest! {
        #[test]
        fn any_plaintext_round_trips(pt in proptest::collection::vec(any::<u8>(), 0..2048),
                                     aad in proptest::collection::vec(any::<u8>(), 0..64)) {
            let sealed = seal(&key(7), &pt, &aad).unwrap();
            prop_assert_eq!(&open(&key(7), &sealed, &aad).unwrap()[..], &pt[..]);
        }

        #[test]
        fn any_single_bit_flip_is_rejected(pt in proptest::collection::vec(any::<u8>(), 0..256),
                                           bit in any::<prop::sample::Index>()) {
            let mut sealed = seal(&key(7), &pt, b"aad").unwrap();
            let i = bit.index(sealed.len() * 8);
            sealed[i / 8] ^= 1 << (i % 8);
            prop_assert_eq!(open(&key(7), &sealed, b"aad").unwrap_err(), CryptoError::Decrypt);
        }
    }
}

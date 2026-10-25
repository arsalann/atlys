//! Vault header authentication with HMAC-SHA256 (RFC-0002).
//!
//! The tag covers `"atlys/v1/header" || 0x00 || payload`, where `payload` is
//! the exact header bytes as stored, never a reserialized object. The key is
//! the [`labels::HEADER_MAC`](crate::hkdf::labels::HEADER_MAC) subkey of the
//! vault key.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::{CryptoError, Result, SecretKey};

/// Length of a header MAC tag.
pub const TAG_LEN: usize = 32;

/// Domain-separation prefix for header MACs, followed by a 0x00 byte.
pub const HEADER_CONTEXT: &[u8] = b"atlys/v1/header";

fn header_mac(key: &SecretKey, payload: &[u8]) -> Hmac<Sha256> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key.expose_secret())
        .expect("HMAC accepts any key length");
    mac.update(HEADER_CONTEXT);
    mac.update(&[0]);
    mac.update(payload);
    mac
}

/// Computes the header tag over the exact `payload` bytes.
pub fn header_tag(key: &SecretKey, payload: &[u8]) -> [u8; TAG_LEN] {
    header_mac(key, payload).finalize().into_bytes().into()
}

/// Verifies a header tag in constant time. All 32 tag bytes are required.
pub fn verify_header_tag(key: &SecretKey, payload: &[u8], tag: &[u8]) -> Result<()> {
    if tag.len() != TAG_LEN {
        return Err(CryptoError::Decrypt);
    }
    header_mac(key, payload)
        .verify_slice(tag)
        .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> SecretKey {
        SecretKey::from_bytes([byte; 32])
    }

    #[test]
    fn verifies_own_tag() {
        let tag = header_tag(&key(1), b"{}");
        assert!(verify_header_tag(&key(1), b"{}", &tag).is_ok());
    }

    #[test]
    fn rejects_wrong_key_payload_and_tag() {
        let tag = header_tag(&key(1), b"{\"a\":1}");
        assert!(verify_header_tag(&key(2), b"{\"a\":1}", &tag).is_err());
        // Whitespace is authenticated: no reserialization tolerance.
        assert!(verify_header_tag(&key(1), b"{\"a\": 1}", &tag).is_err());
        for i in 0..TAG_LEN {
            let mut bad = tag;
            bad[i] ^= 1;
            assert!(verify_header_tag(&key(1), b"{\"a\":1}", &bad).is_err());
        }
        assert!(verify_header_tag(&key(1), b"{\"a\":1}", &tag[..16]).is_err());
        assert!(verify_header_tag(&key(1), b"{\"a\":1}", &[]).is_err());
    }
}

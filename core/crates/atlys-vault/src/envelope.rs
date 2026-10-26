//! Key envelopes (RFC-0002 "Key envelopes").
//!
//! Every envelope is XChaCha20-Poly1305, `nonce[24] || ciphertext || tag[16]`,
//! with a fresh random nonce per write. The associated data starts with
//! `C = "atlys/v1/envelope" || 0x00 || vault_id`, so an envelope copied into
//! another vault's header does not open.
//!
//! ## Device envelope (version 2)
//!
//! Kept in the device-local Keychain, never in the portable header.
//!
//! ```text
//! offset  size  field
//! 0       1     version (2)
//! 1       16    vault ID
//! 17      65    ephemeral P-256 public key (X9.63 uncompressed)
//! 82      65    Secure Enclave P-256 public key (X9.63 uncompressed)
//! 147     72    wrapped vault key, AAD = C || "device" || bytes[0..147)
//! ```
//!
//! The KEK is HKDF-SHA256(salt = both public keys, IKM = the ECDH shared
//! secret, info = `atlys/v1/envelope/secure-enclave`). Swift performs the
//! ECDH and validates the points; the vault key never crosses the FFI.

use atlys_crypto::hkdf::{derive_from_shared_secret, labels};
use atlys_crypto::kdf::{Argon2Params, SALT_LEN};
use atlys_crypto::{CryptoError, SecretKey, aead};

use crate::header::{ENVELOPE_LEN, VAULT_ID_LEN, VaultId};
use crate::{Result, VaultError};

/// Device envelope format version.
pub const DEVICE_ENVELOPE_VERSION: u8 = 2;
/// Length of an X9.63 uncompressed P-256 public key.
pub const P256_PUBLIC_KEY_LEN: usize = 65;
const DEVICE_HEADER_LEN: usize = 1 + VAULT_ID_LEN + 2 * P256_PUBLIC_KEY_LEN;
/// Total device envelope length (219 bytes).
pub const DEVICE_ENVELOPE_LEN: usize = DEVICE_HEADER_LEN + ENVELOPE_LEN;
/// Length of a P-256 ECDH shared secret.
pub const SHARED_SECRET_LEN: usize = 32;

const CONTEXT: &[u8] = b"atlys/v1/envelope";

fn context(vault_id: &VaultId) -> Vec<u8> {
    let mut c = Vec::with_capacity(CONTEXT.len() + 1 + VAULT_ID_LEN + 32);
    c.extend_from_slice(CONTEXT);
    c.push(0);
    c.extend_from_slice(vault_id.as_bytes());
    c
}

/// AAD of the password envelope. Binds the KDF parameters and salt, so
/// substituting them fails even before the header MAC is checked.
pub fn password_aad(vault_id: &VaultId, params: &Argon2Params, salt: &[u8; SALT_LEN]) -> Vec<u8> {
    let mut aad = context(vault_id);
    aad.extend_from_slice(b"password\0");
    aad.extend_from_slice(&params.memory_kib.to_be_bytes());
    aad.extend_from_slice(&params.iterations.to_be_bytes());
    aad.extend_from_slice(&params.parallelism.to_be_bytes());
    aad.extend_from_slice(salt);
    aad
}

/// AAD of the recovery envelope.
pub fn recovery_aad(vault_id: &VaultId) -> Vec<u8> {
    let mut aad = context(vault_id);
    aad.extend_from_slice(b"recovery");
    aad
}

/// AAD of the ingest private-key envelope.
pub fn ingest_aad(vault_id: &VaultId) -> Vec<u8> {
    let mut aad = context(vault_id);
    aad.extend_from_slice(b"ingest");
    aad
}

/// Wraps a 32-byte key under `kek` with `aad`.
pub fn wrap(kek: &SecretKey, key: &[u8; 32], aad: &[u8]) -> Result<[u8; ENVELOPE_LEN]> {
    let sealed = aead::seal(kek, key, aad)?;
    Ok(sealed.try_into().expect("32-byte plaintext gives 72 bytes"))
}

/// Unwraps an envelope. Any failure is [`VaultError::WrongCredentials`].
pub fn unwrap(kek: &SecretKey, envelope: &[u8; ENVELOPE_LEN], aad: &[u8]) -> Result<SecretKey> {
    aead::unwrap_key(kek, envelope, aad).map_err(|_| VaultError::WrongCredentials)
}

/// Creates a device envelope for `vault_key`. `shared_secret` is
/// ECDH(ephemeral private key, enclave public key), computed by the platform.
pub fn seal_device(
    vault_id: &VaultId,
    vault_key: &SecretKey,
    shared_secret: &[u8],
    ephemeral_public_key: &[u8],
    enclave_public_key: &[u8],
) -> Result<[u8; DEVICE_ENVELOPE_LEN]> {
    check_p256_public_key(ephemeral_public_key)?;
    check_p256_public_key(enclave_public_key)?;
    let mut out = [0u8; DEVICE_ENVELOPE_LEN];
    out[0] = DEVICE_ENVELOPE_VERSION;
    out[1..17].copy_from_slice(vault_id.as_bytes());
    out[17..82].copy_from_slice(ephemeral_public_key);
    out[82..147].copy_from_slice(enclave_public_key);
    let kek = device_kek(shared_secret, &out[17..DEVICE_HEADER_LEN])?;
    let wrapped = wrap(
        &kek,
        vault_key.expose_secret(),
        &device_aad(vault_id, &out[..DEVICE_HEADER_LEN]),
    )?;
    out[DEVICE_HEADER_LEN..].copy_from_slice(&wrapped);
    Ok(out)
}

/// The vault ID a device envelope belongs to.
pub fn device_vault_id(envelope: &[u8]) -> Result<VaultId> {
    check_device_envelope(envelope)?;
    Ok(VaultId::from_bytes(
        envelope[1..17].try_into().expect("16 bytes"),
    ))
}

/// The ephemeral public key the platform needs for ECDH on unwrap.
pub fn device_ephemeral_key(envelope: &[u8]) -> Result<&[u8]> {
    check_device_envelope(envelope)?;
    Ok(&envelope[17..82])
}

/// The Secure Enclave public key the envelope was made for.
pub fn device_enclave_key(envelope: &[u8]) -> Result<&[u8]> {
    check_device_envelope(envelope)?;
    Ok(&envelope[82..147])
}

/// Unwraps a device envelope for `vault_id`. `shared_secret` is
/// ECDH(enclave private key, ephemeral public key), gated by Face ID.
pub fn open_device(vault_id: &VaultId, envelope: &[u8], shared_secret: &[u8]) -> Result<SecretKey> {
    if device_vault_id(envelope)? != *vault_id {
        return Err(VaultError::WrongCredentials);
    }
    let (header, wrapped) = envelope.split_at(DEVICE_HEADER_LEN);
    let kek = device_kek(shared_secret, &header[17..])?;
    unwrap(
        &kek,
        wrapped.try_into().expect("72 bytes"),
        &device_aad(vault_id, header),
    )
}

fn device_aad(vault_id: &VaultId, header: &[u8]) -> Vec<u8> {
    let mut aad = context(vault_id);
    aad.extend_from_slice(b"device");
    aad.extend_from_slice(header);
    aad
}

fn device_kek(shared_secret: &[u8], public_keys: &[u8]) -> Result<SecretKey> {
    if shared_secret.len() != SHARED_SECRET_LEN {
        return Err(CryptoError::InvalidInput("P-256 shared secret must be 32 bytes").into());
    }
    Ok(derive_from_shared_secret(
        shared_secret,
        public_keys,
        labels::SECURE_ENCLAVE_ENVELOPE,
    )?)
}

fn check_device_envelope(envelope: &[u8]) -> Result<()> {
    if envelope.len() != DEVICE_ENVELOPE_LEN {
        return Err(CryptoError::InvalidInput("wrong device envelope length").into());
    }
    if envelope[0] != DEVICE_ENVELOPE_VERSION {
        return Err(CryptoError::InvalidInput("unsupported device envelope version").into());
    }
    Ok(())
}

/// Checks the encoding only. The platform validates that the point is on the
/// curve before computing ECDH.
fn check_p256_public_key(key: &[u8]) -> Result<()> {
    if key.len() != P256_PUBLIC_KEY_LEN || key[0] != 0x04 {
        return Err(
            CryptoError::InvalidInput("expected an X9.63 uncompressed P-256 public key").into(),
        );
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

    const ID: VaultId = VaultId::from_bytes([0xAA; 16]);

    #[test]
    fn aad_layouts_match_rfc_0002_vectors() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../spec/vault/v1/vectors.json")).unwrap();
        let id = VaultId::from_bytes(core::array::from_fn(|i| i as u8));
        let salt: [u8; 16] = core::array::from_fn(|i| i as u8);
        let hex = |v: &serde_json::Value| crate::hex::decode_vec(v.as_str().unwrap()).unwrap();
        assert_eq!(
            password_aad(&id, &Argon2Params::OWASP_MINIMUM, &salt),
            hex(&fixture["password_aad_hex"])
        );
        assert_eq!(recovery_aad(&id), hex(&fixture["recovery_aad_hex"]));
        assert_eq!(ingest_aad(&id), hex(&fixture["ingest_wrap_aad_hex"]));
    }

    #[test]
    fn wrap_unwrap_and_cross_vault_rejection() {
        let kek = SecretKey::from_bytes([1; 32]);
        let key = [7u8; 32];
        let env = wrap(&kek, &key, &recovery_aad(&ID)).unwrap();
        let out = unwrap(&kek, &env, &recovery_aad(&ID)).unwrap();
        assert_eq!(out.expose_secret(), &key);
        let other = VaultId::from_bytes([0xBB; 16]);
        assert!(matches!(
            unwrap(&kek, &env, &recovery_aad(&other)),
            Err(VaultError::WrongCredentials)
        ));
        assert!(unwrap(&kek, &env, &ingest_aad(&ID)).is_err());
    }

    #[test]
    fn device_envelope_round_trip() {
        let vault_key = SecretKey::generate().unwrap();
        let env = seal_device(&ID, &vault_key, &[7; 32], &pk(1), &pk(2)).unwrap();
        assert_eq!(env.len(), 219);
        assert_eq!(device_vault_id(&env).unwrap(), ID);
        assert_eq!(device_ephemeral_key(&env).unwrap(), &pk(1)[..]);
        assert_eq!(device_enclave_key(&env).unwrap(), &pk(2)[..]);
        assert!(open_device(&ID, &env, &[7; 32]).unwrap().ct_eq(&vault_key));
    }

    #[test]
    fn device_envelope_rejects_tampering_and_substitution() {
        let vault_key = SecretKey::generate().unwrap();
        let env = seal_device(&ID, &vault_key, &[7; 32], &pk(1), &pk(2)).unwrap();
        assert!(open_device(&ID, &env, &[8; 32]).is_err());
        assert!(open_device(&VaultId::from_bytes([0xBB; 16]), &env, &[7; 32]).is_err());
        for i in 0..env.len() {
            let mut t = env;
            t[i] ^= 1;
            assert!(open_device(&ID, &t, &[7; 32]).is_err(), "byte {i}");
        }
        // Rewriting the embedded vault ID to match another vault still fails.
        let mut moved = env;
        moved[1..17].copy_from_slice(&[0xBB; 16]);
        assert!(open_device(&VaultId::from_bytes([0xBB; 16]), &moved, &[7; 32]).is_err());
        assert!(seal_device(&ID, &vault_key, &[7; 32], &[4; 33], &pk(2)).is_err());
        assert!(seal_device(&ID, &vault_key, &[7; 16], &pk(1), &pk(2)).is_err());
        let mut v1 = env;
        v1[0] = 1;
        assert!(device_ephemeral_key(&v1).is_err());
    }
}

use core::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{CryptoError, Result};

/// Length in bytes of every symmetric key in Atlys.
pub const KEY_LEN: usize = 32;

/// A 256-bit symmetric key that is wiped from memory when dropped.
///
/// Deliberately not `Clone`, `Copy`, `PartialEq`, or `Debug`-printable, so
/// keys are not duplicated, compared in variable time, or logged by accident.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; KEY_LEN]);

impl SecretKey {
    /// Generates a new random key from the operating system's CSPRNG.
    pub fn generate() -> Result<Self> {
        let mut key = Self([0u8; KEY_LEN]);
        getrandom::fill(&mut key.0).map_err(|_| CryptoError::Rng)?;
        Ok(key)
    }

    /// Wraps existing key bytes. The caller should zeroize its copy.
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Copies key bytes from a slice, which must be exactly [`KEY_LEN`] long.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self> {
        let array: [u8; KEY_LEN] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidInput("key must be 32 bytes"))?;
        Ok(Self(array))
    }

    /// Borrows the raw key bytes. Keep the borrow as short as possible.
    pub fn expose_secret(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Constant-time equality check.
    pub fn ct_eq(&self, other: &Self) -> bool {
        // Fold all byte differences together so the comparison time does not
        // depend on where the first difference is.
        let diff = self
            .0
            .iter()
            .zip(other.0.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        core::hint::black_box(diff) == 0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

/// Returns `N` random bytes from the operating system's CSPRNG.
pub fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).map_err(|_| CryptoError::Rng)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_key_bytes() {
        let key = SecretKey::from_bytes([0xAB; KEY_LEN]);
        let printed = format!("{key:?}");
        assert_eq!(printed, "SecretKey(<redacted>)");
        assert!(!printed.to_lowercase().contains("ab"));
    }

    #[test]
    fn generated_keys_differ() {
        let a = SecretKey::generate().unwrap();
        let b = SecretKey::generate().unwrap();
        assert!(!a.ct_eq(&b));
        assert!(a.ct_eq(&SecretKey::from_bytes(*a.expose_secret())));
    }

    #[test]
    fn try_from_slice_checks_length() {
        assert!(SecretKey::try_from_slice(&[0u8; 31]).is_err());
        assert!(SecretKey::try_from_slice(&[0u8; 33]).is_err());
        assert!(SecretKey::try_from_slice(&[0u8; 32]).is_ok());
    }

    #[test]
    fn zeroize_clears_bytes() {
        let mut key = SecretKey::from_bytes([0x5A; KEY_LEN]);
        key.zeroize();
        assert_eq!(key.expose_secret(), &[0u8; KEY_LEN]);
    }
}

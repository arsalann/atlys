//! Password-based key derivation with Argon2id (RFC 9106).

use zeroize::Zeroizing;

use argon2::{Algorithm, Argon2, Params, Version};

use crate::{CryptoError, KEY_LEN, Result, SecretKey};

/// Length of the per-vault random salt.
pub const SALT_LEN: usize = 16;

/// Argon2id cost parameters. Stored in the vault header so they can be raised
/// later without breaking existing vaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2Params {
    /// Memory cost in KiB.
    pub memory_kib: u32,
    /// Number of passes over memory.
    pub iterations: u32,
    /// Degree of parallelism (lanes).
    pub parallelism: u32,
}

impl Argon2Params {
    /// The v0 starting point from `plan/v0/01-account-setup.md`:
    /// m = 64 MiB, t = 3, p = 1. Calibrated in M0 (see `plan/v0/spike-report.md`).
    pub const V0_DEFAULT: Self = Self {
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };

    /// The OWASP minimum recommendation for Argon2id: m = 19 MiB, t = 2, p = 1.
    /// Parameters weaker than this are rejected.
    pub const OWASP_MINIMUM: Self = Self {
        memory_kib: 19 * 1024,
        iterations: 2,
        parallelism: 1,
    };

    /// Upper bounds, so a tampered vault header cannot make unlock exhaust
    /// memory or hang the app. Generous enough for future increases.
    pub const MAX_MEMORY_KIB: u32 = 1024 * 1024;
    /// Upper bound on iterations.
    pub const MAX_ITERATIONS: u32 = 64;
    /// Upper bound on parallelism.
    pub const MAX_PARALLELISM: u32 = 16;

    /// Checks that the parameters are within the accepted range.
    pub fn validate(&self) -> Result<()> {
        let min = Self::OWASP_MINIMUM;
        if self.memory_kib < min.memory_kib || self.memory_kib > Self::MAX_MEMORY_KIB {
            return Err(CryptoError::InvalidParams("memory cost out of range"));
        }
        if self.iterations < min.iterations || self.iterations > Self::MAX_ITERATIONS {
            return Err(CryptoError::InvalidParams("iteration count out of range"));
        }
        if self.parallelism < min.parallelism || self.parallelism > Self::MAX_PARALLELISM {
            return Err(CryptoError::InvalidParams("parallelism out of range"));
        }
        Ok(())
    }
}

/// Derives a 256-bit key from a password with Argon2id (version 0x13).
///
/// Rejects parameters below [`Argon2Params::OWASP_MINIMUM`]. Use
/// [`derive_key_unchecked`] only for benchmarks.
pub fn derive_key(
    password: &[u8],
    salt: &[u8; SALT_LEN],
    params: Argon2Params,
) -> Result<SecretKey> {
    params.validate()?;
    derive_key_unchecked(password, salt, params)
}

/// Like [`derive_key`] but without the minimum-strength check, for
/// calibration benchmarks that measure weak settings. Never use it to protect
/// a vault.
pub fn derive_key_unchecked(
    password: &[u8],
    salt: &[u8; SALT_LEN],
    params: Argon2Params,
) -> Result<SecretKey> {
    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| CryptoError::InvalidParams("rejected by argon2"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password, salt, &mut *out)
        .map_err(|_| CryptoError::Internal)?;
    SecretKey::try_from_slice(&out[..])
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent known-answer vector from the reference C implementation
    // (phc-winner-argon2, `argon2` CLI):
    //   echo -n "correct horse battery staple" | \
    //     argon2 "atlys-test-salt!" -id -m 15 -t 2 -p 1 -l 32 -r
    // m = 2^15 KiB = 32 MiB. Synthetic password and salt.
    const KAT_PASSWORD: &[u8] = b"correct horse battery staple";
    const KAT_SALT: &[u8; SALT_LEN] = b"atlys-test-salt!";
    const KAT_PARAMS: Argon2Params = Argon2Params {
        memory_kib: 32 * 1024,
        iterations: 2,
        parallelism: 1,
    };
    const KAT_OUTPUT_HEX: &str = "eb32aa59adb26ef4d7c9fa62afc4f634b666f4c46f47cf72ca3cc9b706fc5649";

    #[test]
    fn matches_reference_implementation() {
        let key = derive_key(KAT_PASSWORD, KAT_SALT, KAT_PARAMS).unwrap();
        assert_eq!(hex::encode(key.expose_secret()), KAT_OUTPUT_HEX);
    }

    #[test]
    fn same_inputs_same_key_and_salt_matters() {
        let a = derive_key(b"pw", KAT_SALT, Argon2Params::OWASP_MINIMUM).unwrap();
        let b = derive_key(b"pw", KAT_SALT, Argon2Params::OWASP_MINIMUM).unwrap();
        let c = derive_key(b"pw", b"another-salt-16b", Argon2Params::OWASP_MINIMUM).unwrap();
        assert!(a.ct_eq(&b));
        assert!(!a.ct_eq(&c));
    }

    #[test]
    fn rejects_weak_and_absurd_params() {
        let weak = Argon2Params {
            memory_kib: 8 * 1024,
            ..Argon2Params::OWASP_MINIMUM
        };
        assert!(derive_key(b"pw", KAT_SALT, weak).is_err());
        let one_pass = Argon2Params {
            iterations: 1,
            ..Argon2Params::OWASP_MINIMUM
        };
        assert!(derive_key(b"pw", KAT_SALT, one_pass).is_err());
        let huge = Argon2Params {
            memory_kib: 4 * 1024 * 1024,
            ..Argon2Params::OWASP_MINIMUM
        };
        assert_eq!(
            huge.validate(),
            Err(CryptoError::InvalidParams("memory cost out of range"))
        );
        assert!(Argon2Params::V0_DEFAULT.validate().is_ok());
    }
}

//! The product vault over UniFFI: create, unlock, recover, change
//! credentials, lock, delete, plus password and recovery-phrase helpers.
//!
//! Passwords cross as bytes (`Data` in Swift), are wrapped in zeroizing
//! buffers on arrival, and are never logged. Every call that runs Argon2id
//! takes a noticeable fraction of a second: call it off the main thread.

use std::sync::{Arc, Mutex};

use atlys_crypto::kdf::Argon2Params;
use atlys_vault::envelope;
use atlys_vault::{RecoverySecret, UnlockedVault, Vault, VaultPaths, password, recovery};
use zeroize::Zeroizing;

use crate::AtlysError;

/// Argon2id parameters for new passwords (RFC-0002 proposal, pending
/// oldest-device measurements).
const CREATE_PARAMS: Argon2Params = Argon2Params::V1_CREATE;

/// Password strength for display while the user types.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PasswordStrength {
    /// zxcvbn score, 0 (weakest) to 4.
    pub score: u8,
    /// Whether the password meets the policy for a new password.
    pub acceptable: bool,
    /// The main problem, if any.
    pub warning: Option<String>,
    /// Suggestions for a stronger password.
    pub suggestions: Vec<String>,
}

/// Estimates the strength of a candidate password.
#[uniffi::export]
pub fn password_strength(password: String) -> PasswordStrength {
    let password = Zeroizing::new(password);
    let s = password::strength(&password, &[]);
    PasswordStrength {
        score: s.score,
        acceptable: s.acceptable,
        warning: s.warning,
        suggestions: s.suggestions,
    }
}

/// Minimum length of a new password, in characters.
#[uniffi::export]
pub fn password_min_length() -> u32 {
    password::MIN_CHARS as u32
}

/// Whether `word` is in the recovery-phrase wordlist.
#[uniffi::export]
pub fn is_recovery_word(word: String) -> bool {
    recovery::is_word(&word)
}

/// Whether a full phrase is valid (24 known words with a correct checksum).
#[uniffi::export]
pub fn is_valid_recovery_phrase(words: Vec<String>) -> bool {
    let words = Zeroizing::new(words);
    RecoverySecret::from_phrase(&words).is_ok()
}

/// Whether a vault exists in `vault_root`.
#[uniffi::export]
pub fn vault_exists(vault_root: String) -> bool {
    Vault::exists(&VaultPaths::new(vault_root))
}

/// The vault ID as lowercase hex, read from the unverified header. Used to
/// scope Keychain items while locked; not authenticated until unlock.
#[uniffi::export]
pub fn vault_id_unverified(vault_root: String) -> Result<String, AtlysError> {
    let header = Vault::read_header(&VaultPaths::new(vault_root))?;
    Ok(hex(header.unverified().vault_id.as_bytes()))
}

/// A newly created vault and its recovery phrase. Show the phrase once and
/// drop it.
#[derive(Debug, uniffi::Record)]
pub struct CreatedVault {
    /// The unlocked session.
    pub session: Arc<VaultSession>,
    /// The 24-word recovery phrase.
    pub recovery_phrase: Vec<String>,
}

/// Creates a vault in `vault_root`. Fails with `VaultExists` rather than
/// replacing an existing vault.
#[uniffi::export]
pub fn create_vault(vault_root: String, password: Vec<u8>) -> Result<CreatedVault, AtlysError> {
    let password = Zeroizing::new(password);
    let (vault, recovery) = Vault::create(&VaultPaths::new(vault_root), &password, CREATE_PARAMS)?;
    Ok(CreatedVault {
        session: VaultSession::new(vault),
        recovery_phrase: recovery.to_phrase(),
    })
}

/// Unlocks with the master password.
#[uniffi::export]
pub fn unlock_with_password(
    vault_root: String,
    password: Vec<u8>,
) -> Result<Arc<VaultSession>, AtlysError> {
    let password = Zeroizing::new(password);
    let vault = Vault::unlock_with_password(&VaultPaths::new(vault_root), &password)?;
    Ok(VaultSession::new(vault))
}

/// Unlocks with a device (Face ID) envelope and the Secure Enclave's ECDH
/// shared secret for the envelope's ephemeral key.
#[uniffi::export]
pub fn unlock_with_device(
    vault_root: String,
    device_envelope: Vec<u8>,
    shared_secret: Vec<u8>,
) -> Result<Arc<VaultSession>, AtlysError> {
    let shared_secret = Zeroizing::new(shared_secret);
    let vault = Vault::unlock_with_device(
        &VaultPaths::new(vault_root),
        &device_envelope,
        &shared_secret,
    )?;
    Ok(VaultSession::new(vault))
}

/// Unlocks with the recovery phrase and sets a new password in one step.
#[uniffi::export]
pub fn recover_vault(
    vault_root: String,
    recovery_phrase: Vec<String>,
    new_password: Vec<u8>,
) -> Result<Arc<VaultSession>, AtlysError> {
    let phrase = Zeroizing::new(recovery_phrase);
    let new_password = Zeroizing::new(new_password);
    let secret = RecoverySecret::from_phrase(&phrase)?;
    let vault = Vault::recover(
        &VaultPaths::new(vault_root),
        &secret,
        &new_password,
        CREATE_PARAMS,
    )?;
    Ok(VaultSession::new(vault))
}

/// The ephemeral public key in a device envelope, for the Secure Enclave ECDH.
#[uniffi::export]
pub fn device_envelope_ephemeral_key(device_envelope: Vec<u8>) -> Result<Vec<u8>, AtlysError> {
    Ok(envelope::device_ephemeral_key(&device_envelope)?.to_vec())
}

/// The Secure Enclave public key a device envelope was made for.
#[uniffi::export]
pub fn device_envelope_enclave_key(device_envelope: Vec<u8>) -> Result<Vec<u8>, AtlysError> {
    Ok(envelope::device_enclave_key(&device_envelope)?.to_vec())
}

/// Finishes a deletion whose header is already gone (a retry after a
/// partial deletion). Normal deletion goes through [`VaultSession::delete`].
#[uniffi::export]
pub fn delete_vault_leftovers(vault_root: String) -> Result<(), AtlysError> {
    let paths = VaultPaths::new(vault_root);
    if Vault::exists(&paths) {
        return Err(AtlysError::VaultExists);
    }
    Ok(atlys_vault::vault::delete_files(&paths)?)
}

/// An unlocked vault. Locking or deleting ends the session; later calls fail
/// with `Locked`. Dropping the last reference also locks.
#[derive(Debug, uniffi::Object)]
pub struct VaultSession {
    inner: Mutex<Option<UnlockedVault>>,
}

impl VaultSession {
    fn new(vault: UnlockedVault) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Some(vault)),
        })
    }

    fn with<T>(
        &self,
        f: impl FnOnce(&mut UnlockedVault) -> Result<T, AtlysError>,
    ) -> Result<T, AtlysError> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        f(guard.as_mut().ok_or(AtlysError::Locked)?)
    }

    fn take(&self) -> Option<UnlockedVault> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

#[uniffi::export]
impl VaultSession {
    /// The vault ID as lowercase hex (authenticated).
    pub fn vault_id(&self) -> Result<String, AtlysError> {
        self.with(|v| Ok(hex(v.vault_id().as_bytes())))
    }

    /// Whether the session has been locked.
    pub fn is_locked(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none()
    }

    /// Changes the master password. The current password is checked afresh.
    pub fn change_password(
        &self,
        current_password: Vec<u8>,
        new_password: Vec<u8>,
    ) -> Result<(), AtlysError> {
        let current = Zeroizing::new(current_password);
        let new = Zeroizing::new(new_password);
        self.with(|v| Ok(v.change_password(&current, &new, CREATE_PARAMS)?))
    }

    /// Checks the master password without changing anything, for actions
    /// that need fresh authentication (such as deleting the vault).
    pub fn verify_password(&self, password: Vec<u8>) -> Result<(), AtlysError> {
        let password = Zeroizing::new(password);
        self.with(|v| Ok(v.verify_password(&password)?))
    }

    /// Replaces the recovery phrase. The old phrase stops working.
    pub fn regenerate_recovery(&self) -> Result<Vec<String>, AtlysError> {
        self.with(|v| Ok(v.regenerate_recovery()?.to_phrase()))
    }

    /// Creates a device (Face ID) envelope. Swift stores it in the Keychain.
    pub fn device_envelope(
        &self,
        shared_secret: Vec<u8>,
        ephemeral_public_key: Vec<u8>,
        enclave_public_key: Vec<u8>,
    ) -> Result<Vec<u8>, AtlysError> {
        let shared_secret = Zeroizing::new(shared_secret);
        self.with(|v| {
            Ok(
                v.device_envelope(&shared_secret, &ephemeral_public_key, &enclave_public_key)?
                    .to_vec(),
            )
        })
    }

    /// Locks: closes the database and zeroizes the keys.
    pub fn lock(&self) -> Result<(), AtlysError> {
        match self.take() {
            Some(vault) => Ok(vault.lock()?),
            None => Ok(()),
        }
    }

    /// Deletes the vault and its inbox. Call only right after fresh
    /// authentication, with collectors stopped; Swift removes the Keychain
    /// items. Safe to retry on `Storage` errors via `delete_vault_leftovers`.
    pub fn delete(&self) -> Result<(), AtlysError> {
        let vault = self.take().ok_or(AtlysError::Locked)?;
        Ok(vault.delete()?)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IngestSealer, RecordDraft};

    const PASSWORD: &[u8] = b"synthetic correct horse battery";
    const NEW_PASSWORD: &[u8] = b"synthetic second passphrase";

    fn root() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("container").to_string_lossy().into_owned();
        (dir, root)
    }

    fn p256(byte: u8) -> Vec<u8> {
        let mut k = vec![byte; 65];
        k[0] = 4;
        k
    }

    // One test runs the whole flow, because each Argon2id call at the
    // creation parameters costs a noticeable fraction of a second.
    #[test]
    fn full_lifecycle_over_ffi() {
        let (_dir, root) = root();
        assert!(!vault_exists(root.clone()));
        assert!(matches!(
            create_vault(root.clone(), b"weak".to_vec()),
            Err(AtlysError::WeakPassword { .. })
        ));
        let created = create_vault(root.clone(), PASSWORD.to_vec()).unwrap();
        assert!(vault_exists(root.clone()));
        assert_eq!(created.recovery_phrase.len(), 24);
        assert!(is_valid_recovery_phrase(created.recovery_phrase.clone()));
        let id = created.session.vault_id().unwrap();
        assert_eq!(vault_id_unverified(root.clone()).unwrap(), id);

        let envelope = created
            .session
            .device_envelope(vec![7; 32], p256(1), p256(2))
            .unwrap();
        assert_eq!(
            device_envelope_ephemeral_key(envelope.clone()).unwrap(),
            p256(1)
        );
        assert_eq!(
            device_envelope_enclave_key(envelope.clone()).unwrap(),
            p256(2)
        );

        // Collectors write while locked.
        created.session.lock().unwrap();
        assert!(created.session.is_locked());
        assert!(matches!(
            created.session.vault_id(),
            Err(AtlysError::Locked)
        ));
        let sealer = IngestSealer::open(root.clone()).unwrap();
        let name = sealer
            .seal(
                "device".into(),
                vec![RecordDraft {
                    record_type: "device.battery".into(),
                    source_id: "synthetic-1".into(),
                    start_ms: 1,
                    end_ms: None,
                    data_json: r#"{"level":0.5,"state":"unplugged","low_power_mode":false}"#.into(),
                    tz: Some("Europe/Paris".into()),
                    ..Default::default()
                }],
            )
            .unwrap();
        assert!(name.ends_with(".sealed"));

        assert!(matches!(
            unlock_with_password(root.clone(), NEW_PASSWORD.to_vec()),
            Err(AtlysError::WrongCredentials)
        ));
        let session = unlock_with_device(root.clone(), envelope.clone(), vec![7; 32]).unwrap();
        assert!(matches!(
            session.verify_password(NEW_PASSWORD.to_vec()),
            Err(AtlysError::WrongCredentials)
        ));
        session.verify_password(PASSWORD.to_vec()).unwrap();
        session
            .change_password(PASSWORD.to_vec(), NEW_PASSWORD.to_vec())
            .unwrap();
        let phrase = session.regenerate_recovery().unwrap();
        session.lock().unwrap();

        assert!(matches!(
            recover_vault(
                root.clone(),
                created.recovery_phrase.clone(),
                PASSWORD.to_vec()
            ),
            Err(AtlysError::WrongCredentials)
        ));
        let mut bad = phrase.clone();
        bad.swap(0, 1);
        assert!(matches!(
            recover_vault(root.clone(), bad, PASSWORD.to_vec()),
            Err(AtlysError::InvalidRecoveryPhrase)
        ));
        let session = recover_vault(root.clone(), phrase, PASSWORD.to_vec()).unwrap();
        session.delete().unwrap();
        assert!(!vault_exists(root.clone()));
        assert!(matches!(
            unlock_with_password(root.clone(), PASSWORD.to_vec()),
            Err(AtlysError::NoVault)
        ));
        delete_vault_leftovers(root).unwrap();
    }

    #[test]
    fn strength_and_phrase_helpers() {
        let weak = password_strength("password1234".into());
        assert!(!weak.acceptable);
        assert!(password_strength("synthetic correct horse battery".into()).acceptable);
        assert_eq!(password_min_length(), 12);
        assert!(is_recovery_word("abandon".into()));
        assert!(!is_recovery_word("atlys".into()));
        assert!(!is_valid_recovery_phrase(vec!["abandon".into(); 24]));
    }
}

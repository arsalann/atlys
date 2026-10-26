//! **Throwaway M0 spike API.** Compiled only with the `spike` feature.
//!
//! These exports exist to measure and verify things on the simulator and on
//! devices (M0 tasks 0.3, 0.5, 0.6, 0.7, 0.8). They take shortcuts that are
//! not acceptable in the product, most importantly [`SpikeVault`], which
//! stores its private keys **unencrypted** on disk. Delete this module before
//! M1 feature work; the real `Vault` / `VaultSession` API replaces it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use atlys_crypto::hpke::{IngestKeyPair, IngestPrivateKey};
use atlys_crypto::kdf::{Argon2Params, derive_key_unchecked};
use atlys_crypto::{SecretKey, random_bytes};
use atlys_vault::m0::{envelope, inbox};
use atlys_vault::store::Store;
use zeroize::Zeroizing;

use crate::AtlysError;

// ---------------------------------------------------------------------------
// 0.3 SQLCipher self-test
// ---------------------------------------------------------------------------

/// Result of [`spike_sqlcipher_self_test`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct SqlcipherReport {
    /// SQLCipher version.
    pub cipher_version: String,
    /// Crypto provider (`commoncrypto` expected on iOS).
    pub provider: String,
    /// Provider version string.
    pub provider_version: String,
    /// SQLite version.
    pub sqlite_version: String,
    /// Data written with the key reads back after close and reopen.
    pub round_trip_ok: bool,
    /// Opening with a different key fails.
    pub wrong_key_rejected: bool,
    /// The file does not start with the plaintext `SQLite format 3` header.
    pub header_encrypted: bool,
    /// A known plaintext marker does not appear anywhere in the file.
    pub plaintext_absent: bool,
    /// Wall time for the whole test.
    pub elapsed_ms: f64,
}

/// Creates an encrypted database in `dir` with a random raw key, checks it,
/// then deletes it.
#[uniffi::export]
pub fn spike_sqlcipher_self_test(dir: String) -> Result<SqlcipherReport, AtlysError> {
    const MARKER: &[u8] = b"ATLYS-SPIKE-SYNTHETIC-MARKER";
    let start = Instant::now();
    let dir = PathBuf::from(dir);
    fs::create_dir_all(&dir).map_err(io)?;
    let path = dir.join("spike-selftest.db");
    remove_db(&path);

    let key = SecretKey::generate()?;
    let store = Store::create(&path, &key)?;
    let info = store.cipher_info()?;
    store.put_meta("probe", MARKER)?;
    store.close()?;

    let round_trip_ok = Store::open(&path, &key)?.get_meta("probe")?.as_deref() == Some(MARKER);
    let wrong_key_rejected = Store::open(&path, &SecretKey::generate()?).is_err();
    let bytes = fs::read(&path).map_err(io)?;
    let header_encrypted = !bytes.starts_with(b"SQLite format 3\0");
    let plaintext_absent = !bytes.windows(MARKER.len()).any(|w| w == MARKER);
    remove_db(&path);

    Ok(SqlcipherReport {
        cipher_version: info.cipher_version,
        provider: info.provider,
        provider_version: info.provider_version,
        sqlite_version: info.sqlite_version,
        round_trip_ok,
        wrong_key_rejected,
        header_encrypted,
        plaintext_absent,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
    })
}

fn remove_db(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

// ---------------------------------------------------------------------------
// 0.6 Argon2id calibration
// ---------------------------------------------------------------------------

/// Runs one Argon2id derivation with the given costs and returns its wall
/// time in milliseconds. Accepts weak settings, for calibration only.
#[uniffi::export]
pub fn spike_argon2_ms(
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<f64, AtlysError> {
    let salt = random_bytes::<16>()?;
    let params = Argon2Params {
        memory_kib,
        iterations,
        parallelism,
    };
    let start = Instant::now();
    let _key = derive_key_unchecked(b"synthetic benchmark password", &salt, params)?;
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

// ---------------------------------------------------------------------------
// 0.5 / 0.7 / 0.8 Spike vault: ingest keypair + a stand-in vault key
// ---------------------------------------------------------------------------

const KEYS_FILE: &str = "spike-keys.bin";
const PUBLIC_KEY_FILE: &str = "ingest.pub";

/// Summary of one opened inbox file. Contains no record contents.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BatchSummary {
    /// File name.
    pub file_name: String,
    /// Collector that wrote it.
    pub collector: String,
    /// When it was sealed.
    pub created_ms: i64,
    /// Number of records.
    pub record_count: u32,
    /// Distinct record types, sorted.
    pub record_types: Vec<String>,
}

/// Result of [`SpikeVault::open_inbox`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct InboxReport {
    /// Files that decrypted and validated.
    pub opened: u32,
    /// Files that failed authentication or validation.
    pub rejected: u32,
    /// Total records across opened files.
    pub records: u32,
    /// One entry per opened file, oldest first.
    pub batches: Vec<BatchSummary>,
}

/// A stand-in for the real vault: an ingest keypair and a random "vault key".
///
/// **Spike only:** both private keys are stored unencrypted in
/// `<dir>/spike-keys.bin`. In the product the ingest private key is wrapped by
/// the vault key inside the vault, and the vault key is never stored in the
/// clear. The public key is stored separately in `<dir>/ingest.pub`, as the
/// vault header will.
#[derive(uniffi::Object)]
pub struct SpikeVault {
    ingest: IngestKeyPair,
    vault_key: SecretKey,
}

impl std::fmt::Debug for SpikeVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpikeVault").finish_non_exhaustive()
    }
}

#[uniffi::export]
impl SpikeVault {
    /// Creates new keys in `dir`, replacing any existing spike vault there.
    #[uniffi::constructor]
    pub fn create(dir: String) -> Result<Arc<Self>, AtlysError> {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).map_err(io)?;
        let vault = Self {
            ingest: IngestKeyPair::generate(),
            vault_key: SecretKey::generate()?,
        };
        let mut secret = Zeroizing::new(Vec::with_capacity(64));
        secret.extend_from_slice(&vault.ingest.private.to_bytes()[..]);
        secret.extend_from_slice(vault.vault_key.expose_secret());
        fs::write(dir.join(KEYS_FILE), &secret[..]).map_err(io)?;
        fs::write(dir.join(PUBLIC_KEY_FILE), vault.ingest.public.to_bytes()).map_err(io)?;
        Ok(Arc::new(vault))
    }

    /// Loads an existing spike vault from `dir`.
    #[uniffi::constructor]
    pub fn open(dir: String) -> Result<Arc<Self>, AtlysError> {
        let secret = Zeroizing::new(fs::read(Path::new(&dir).join(KEYS_FILE)).map_err(io)?);
        if secret.len() != 64 {
            return Err(AtlysError::InvalidInput {
                message: "corrupt spike key file".into(),
            });
        }
        let private = IngestPrivateKey::from_bytes(&secret[..32])?;
        let public = private.public_key();
        Ok(Arc::new(Self {
            ingest: IngestKeyPair { private, public },
            vault_key: SecretKey::try_from_slice(&secret[32..])?,
        }))
    }

    /// The ingest public key (32 bytes, X25519).
    pub fn ingest_public_key(&self) -> Vec<u8> {
        self.ingest.public.to_bytes().to_vec()
    }

    /// Opens every pending file in `inbox_dir`. Opened files are deleted when
    /// `delete_opened` is set; rejected files are always left in place.
    pub fn open_inbox(
        &self,
        inbox_dir: String,
        delete_opened: bool,
    ) -> Result<InboxReport, AtlysError> {
        let mut report = InboxReport {
            opened: 0,
            rejected: 0,
            records: 0,
            batches: Vec::new(),
        };
        for path in inbox::pending(Path::new(&inbox_dir))? {
            match inbox::read(&self.ingest.private, &path) {
                Ok(batch) => {
                    let mut types: Vec<String> = batch
                        .records
                        .iter()
                        .map(|r| r.record_type.clone())
                        .collect();
                    types.sort();
                    types.dedup();
                    report.opened += 1;
                    report.records += batch.records.len() as u32;
                    report.batches.push(BatchSummary {
                        file_name: file_name(&path),
                        collector: batch.collector,
                        created_ms: batch.created_ms,
                        record_count: batch.records.len() as u32,
                        record_types: types,
                    });
                    if delete_opened {
                        fs::remove_file(&path).map_err(io)?;
                    }
                }
                Err(_) => report.rejected += 1,
            }
        }
        Ok(report)
    }

    /// Wraps this vault's key into a Secure Enclave envelope. `shared_secret`
    /// is ECDH(ephemeral private key, Secure Enclave public key) from CryptoKit.
    pub fn secure_enclave_envelope(
        &self,
        shared_secret: Vec<u8>,
        ephemeral_public_key: Vec<u8>,
        enclave_public_key: Vec<u8>,
    ) -> Result<Vec<u8>, AtlysError> {
        let shared_secret = Zeroizing::new(shared_secret);
        Ok(envelope::seal_secure_enclave(
            &self.vault_key,
            &shared_secret,
            &ephemeral_public_key,
            &enclave_public_key,
        )?)
    }

    /// Unwraps a Secure Enclave envelope and reports whether it holds this
    /// vault's key. The key itself is never returned. `shared_secret` is
    /// ECDH(Secure Enclave private key, envelope ephemeral public key).
    pub fn unlock_with_secure_enclave(
        &self,
        envelope: Vec<u8>,
        shared_secret: Vec<u8>,
    ) -> Result<bool, AtlysError> {
        let shared_secret = Zeroizing::new(shared_secret);
        let key = envelope::open_secure_enclave(&envelope, &shared_secret)?;
        Ok(key.ct_eq(&self.vault_key))
    }
}

/// The ephemeral public key stored in a Secure Enclave envelope.
#[uniffi::export]
pub fn spike_secure_enclave_ephemeral_key(envelope: Vec<u8>) -> Result<Vec<u8>, AtlysError> {
    Ok(envelope::secure_enclave_ephemeral_key(&envelope)?.to_vec())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn io(err: std::io::Error) -> AtlysError {
    AtlysError::Storage {
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IngestSealer, RecordDraft};

    #[test]
    fn sqlcipher_self_test_passes() {
        let dir = tempfile::tempdir().unwrap();
        let r = spike_sqlcipher_self_test(dir.path().to_string_lossy().into()).unwrap();
        assert!(
            r.round_trip_ok && r.wrong_key_rejected && r.header_encrypted && r.plaintext_absent
        );
        #[cfg(target_vendor = "apple")]
        assert_eq!(r.provider, "commoncrypto");
        assert!(!dir.path().join("spike-selftest.db").exists());
    }

    #[test]
    fn vault_persists_and_opens_its_inbox() {
        let dir = tempfile::tempdir().unwrap();
        let vault_dir = dir.path().join("vault").to_string_lossy().into_owned();
        let inbox_dir = dir.path().join("inbox").to_string_lossy().into_owned();
        let created = SpikeVault::create(vault_dir.clone()).unwrap();
        let sealer = IngestSealer::new(inbox_dir.clone(), created.ingest_public_key()).unwrap();
        sealer
            .seal(
                "health".into(),
                vec![RecordDraft {
                    record_type: "health.steps".into(),
                    source_id: "synthetic".into(),
                    start_ms: 0,
                    end_ms: Some(60_000),
                    data_json: r#"{"count":10}"#.into(),
                }],
            )
            .unwrap();
        fs::write(
            Path::new(&inbox_dir).join("01TAMPERED.sealed"),
            b"ATLYSINB garbage",
        )
        .unwrap();

        let reopened = SpikeVault::open(vault_dir).unwrap();
        assert_eq!(reopened.ingest_public_key(), created.ingest_public_key());
        let report = reopened.open_inbox(inbox_dir.clone(), true).unwrap();
        assert_eq!((report.opened, report.rejected, report.records), (1, 1, 1));
        assert_eq!(report.batches[0].record_types, vec!["health.steps"]);
        assert_eq!(
            inbox::pending(Path::new(&inbox_dir)).unwrap().len(),
            1,
            "tampered file kept"
        );
    }

    #[test]
    fn secure_enclave_envelope_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let vault = SpikeVault::create(dir.path().to_string_lossy().into()).unwrap();
        let mut eph = vec![1u8; 65];
        eph[0] = 4;
        let mut se = vec![2u8; 65];
        se[0] = 4;
        let env = vault
            .secure_enclave_envelope(vec![9; 32], eph.clone(), se)
            .unwrap();
        assert_eq!(
            spike_secure_enclave_ephemeral_key(env.clone()).unwrap(),
            eph
        );
        assert!(
            vault
                .unlock_with_secure_enclave(env.clone(), vec![9; 32])
                .unwrap()
        );
        assert!(matches!(
            vault.unlock_with_secure_enclave(env, vec![8; 32]),
            Err(AtlysError::WrongKeyOrTampered)
        ));
    }

    #[test]
    fn argon2_timing_is_positive() {
        assert!(spike_argon2_ms(19 * 1024, 2, 1).unwrap() > 0.0);
    }
}

//! Vault lifecycle (RFC-0002 "Database and durable lifecycle").
//!
//! Layout under the container directory:
//!
//! ```text
//! vault/header.json     authenticated header (public metadata + envelopes)
//! vault/vault.db        SQLCipher database (+ WAL/SHM while open)
//! inbox/                sealed batches from collectors
//! quarantine/           rejected inbox files
//! .vault-create-*/      interrupted creations, never treated as vaults
//! ```
//!
//! Locked code may read the header's public metadata with
//! [`Vault::read_header`]; everything else goes through an [`UnlockedVault`],
//! which exists only after an envelope opened **and** the header MAC
//! verified.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use atlys_crypto::hkdf::{derive_subkey, labels};
use atlys_crypto::hpke::{IngestKeyPair, IngestPrivateKey, IngestPublicKey};
use atlys_crypto::kdf::{self, Argon2Params};
use atlys_crypto::{SecretKey, random_bytes};

use crate::envelope::{self, DEVICE_ENVELOPE_LEN};
use crate::fsutil::{random_suffix, read_regular_bounded, sync_dir, write_atomic};
use crate::header::{self, Header, KdfParams, UnverifiedHeader, VaultId};
use crate::inbox::InboxWriter;
use crate::recovery::RecoverySecret;
use crate::store::Store;
use crate::{Result, VaultError, password};

const CREATE_PREFIX: &str = ".vault-create-";

/// Only one Argon2id derivation runs at a time (RFC-0002), bounding memory.
static KDF_LOCK: Mutex<()> = Mutex::new(());

/// Paths of a vault inside its container directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultPaths {
    root: PathBuf,
}

impl VaultPaths {
    /// A vault rooted at `root`, normally the app's own container.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The container directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `vault/`: header and database.
    pub fn vault_dir(&self) -> PathBuf {
        self.root.join("vault")
    }

    /// `vault/header.json`.
    pub fn header(&self) -> PathBuf {
        self.vault_dir().join("header.json")
    }

    /// `vault/vault.db`.
    pub fn database(&self) -> PathBuf {
        self.vault_dir().join("vault.db")
    }

    /// `inbox/`.
    pub fn inbox(&self) -> PathBuf {
        self.root.join("inbox")
    }

    /// `quarantine/`.
    pub fn quarantine(&self) -> PathBuf {
        self.root.join("quarantine")
    }
}

/// Entry points that do not need an unlocked vault.
#[derive(Debug)]
pub struct Vault;

impl Vault {
    /// Whether a vault header exists at `paths`.
    pub fn exists(paths: &VaultPaths) -> bool {
        paths.header().symlink_metadata().is_ok()
    }

    /// Creates a vault. Returns the unlocked vault and the recovery secret,
    /// which the user must save. Fails with [`VaultError::VaultExists`]
    /// rather than replacing an existing vault.
    pub fn create(
        paths: &VaultPaths,
        password: &[u8],
        params: Argon2Params,
    ) -> Result<(UnlockedVault, RecoverySecret)> {
        password::check_new(password)?;
        params.validate()?;
        if paths.vault_dir().symlink_metadata().is_ok() {
            return Err(VaultError::VaultExists);
        }
        let vault_id = VaultId::generate()?;
        let vault_key = SecretKey::generate()?;
        let recovery = RecoverySecret::generate()?;
        let ingest = IngestKeyPair::generate();
        let kdf = KdfParams {
            argon2: params,
            salt: random_bytes()?,
        };
        let header = Header {
            vault_id,
            kdf,
            password_envelope: password_envelope(&vault_id, &vault_key, password, &kdf)?,
            recovery_envelope: envelope::wrap(
                &recovery_kek(&recovery),
                vault_key.expose_secret(),
                &envelope::recovery_aad(&vault_id),
            )?,
            ingest_public_key: ingest.public.clone(),
            ingest_private_envelope: envelope::wrap(
                &derive_subkey(&vault_key, labels::INGEST_WRAP),
                &ingest.private.to_bytes(),
                &envelope::ingest_aad(&vault_id),
            )?,
        };

        fs::create_dir_all(paths.root())?;
        remove_interrupted_creations(paths)?;
        let tmp = paths
            .root()
            .join(format!("{CREATE_PREFIX}{}", random_suffix()?));
        let result = (|| -> Result<()> {
            fs::create_dir(&tmp)?;
            Store::create(&tmp.join("vault.db"), &sqlcipher_key(&vault_key))?.close()?;
            write_atomic(
                &tmp,
                ".header.tmp",
                &tmp.join("header.json"),
                &header.encode(&vault_key),
            )?;
            // Publish without replacing: a non-empty target makes rename fail,
            // and the existence check covers an empty one.
            if paths.vault_dir().symlink_metadata().is_ok() {
                return Err(VaultError::VaultExists);
            }
            fs::rename(&tmp, paths.vault_dir())?;
            sync_dir(paths.root())?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }

        let store = Store::open(&paths.database(), &sqlcipher_key(&vault_key))?;
        Ok((
            UnlockedVault {
                paths: paths.clone(),
                header,
                vault_key,
                ingest_private: ingest.private,
                store: Some(store),
            },
            recovery,
        ))
    }

    /// Reads and parses the header without authenticating it. Locked code
    /// uses this only for the ingest public key (see RFC-0002 on the limits
    /// of pre-unlock integrity).
    pub fn read_header(paths: &VaultPaths) -> Result<UnverifiedHeader> {
        let bytes = match read_regular_bounded(&paths.header(), header::MAX_FILE_LEN) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Err(VaultError::InvalidHeader("header file too large")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(VaultError::NoVault),
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                return Err(VaultError::InvalidHeader("header is not a regular file"));
            }
            Err(e) => return Err(e.into()),
        };
        UnverifiedHeader::parse(&bytes)
    }

    /// An inbox writer for background collectors, built from the unverified
    /// header. Needs no secrets.
    pub fn inbox_writer(paths: &VaultPaths) -> Result<InboxWriter> {
        let header = Self::read_header(paths)?;
        let header = header.unverified();
        InboxWriter::new(
            paths.inbox(),
            paths.quarantine(),
            header.vault_id,
            header.ingest_public_key.clone(),
        )
    }

    /// Unlocks with the master password.
    pub fn unlock_with_password(paths: &VaultPaths, password: &[u8]) -> Result<UnlockedVault> {
        let unverified = Self::read_header(paths)?;
        let h = unverified.unverified();
        let kek = derive_password_kek(password, &h.kdf)?;
        let vault_key = envelope::unwrap(
            &kek,
            &h.password_envelope,
            &envelope::password_aad(&h.vault_id, &h.kdf.argon2, &h.kdf.salt),
        )?;
        finish_unlock(paths, unverified, vault_key)
    }

    /// Unlocks with the device (Face ID) envelope. `shared_secret` is the
    /// Secure Enclave ECDH result for the envelope's ephemeral key.
    pub fn unlock_with_device(
        paths: &VaultPaths,
        device_envelope: &[u8],
        shared_secret: &[u8],
    ) -> Result<UnlockedVault> {
        let unverified = Self::read_header(paths)?;
        let vault_key = envelope::open_device(
            &unverified.unverified().vault_id,
            device_envelope,
            shared_secret,
        )?;
        finish_unlock(paths, unverified, vault_key)
    }

    /// Unlocks with the recovery secret and sets a new password in the same
    /// step, so recovery never yields a vault whose password is unknown.
    pub fn recover(
        paths: &VaultPaths,
        recovery: &RecoverySecret,
        new_password: &[u8],
        params: Argon2Params,
    ) -> Result<UnlockedVault> {
        password::check_new(new_password)?;
        params.validate()?;
        let unverified = Self::read_header(paths)?;
        let h = unverified.unverified();
        let vault_key = envelope::unwrap(
            &recovery_kek(recovery),
            &h.recovery_envelope,
            &envelope::recovery_aad(&h.vault_id),
        )?;
        let mut vault = finish_unlock(paths, unverified, vault_key)?;
        vault.set_password(new_password, params)?;
        Ok(vault)
    }
}

/// An unlocked vault. Holds the vault key, the ingest private key, and the
/// open database. Dropping it (or [`lock`](Self::lock)) closes the database
/// and zeroizes the keys.
#[derive(Debug)]
pub struct UnlockedVault {
    paths: VaultPaths,
    header: Header,
    vault_key: SecretKey,
    ingest_private: IngestPrivateKey,
    store: Option<Store>,
}

impl UnlockedVault {
    /// The vault's identifier.
    pub fn vault_id(&self) -> VaultId {
        self.header.vault_id
    }

    /// The authenticated header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The vault's paths.
    pub fn paths(&self) -> &VaultPaths {
        &self.paths
    }

    /// The ingest public key collectors seal to.
    pub fn ingest_public_key(&self) -> &IngestPublicKey {
        &self.header.ingest_public_key
    }

    /// The ingest private key, for merging the inbox.
    pub fn ingest_private_key(&self) -> &IngestPrivateKey {
        &self.ingest_private
    }

    /// The open database.
    pub fn store(&self) -> &Store {
        self.store.as_ref().expect("store is open until lock")
    }

    /// Changes the master password. Requires the current password, checked
    /// afresh, and generates a new salt. Only the password envelope, KDF
    /// fields, and MAC change.
    pub fn change_password(
        &mut self,
        current: &[u8],
        new_password: &[u8],
        params: Argon2Params,
    ) -> Result<()> {
        password::check_new(new_password)?;
        params.validate()?;
        self.verify_password(current)?;
        self.set_password(new_password, params)
    }

    /// Checks the master password afresh (Argon2id), for actions that need
    /// recent authentication such as deleting the vault.
    pub fn verify_password(&self, password: &[u8]) -> Result<()> {
        let h = &self.header;
        let kek = derive_password_kek(password, &h.kdf)?;
        let key = envelope::unwrap(
            &kek,
            &h.password_envelope,
            &envelope::password_aad(&h.vault_id, &h.kdf.argon2, &h.kdf.salt),
        )?;
        if !key.ct_eq(&self.vault_key) {
            return Err(VaultError::WrongCredentials);
        }
        Ok(())
    }

    /// Replaces the recovery envelope. The old phrase stops working.
    pub fn regenerate_recovery(&mut self) -> Result<RecoverySecret> {
        let recovery = RecoverySecret::generate()?;
        let mut header = self.header.clone();
        header.recovery_envelope = envelope::wrap(
            &recovery_kek(&recovery),
            self.vault_key.expose_secret(),
            &envelope::recovery_aad(&header.vault_id),
        )?;
        self.write_header(header)?;
        Ok(recovery)
    }

    /// Creates a device (Face ID) envelope for this vault. The caller stores
    /// it in the device-local Keychain; it never enters the header.
    pub fn device_envelope(
        &self,
        shared_secret: &[u8],
        ephemeral_public_key: &[u8],
        enclave_public_key: &[u8],
    ) -> Result<[u8; DEVICE_ENVELOPE_LEN]> {
        envelope::seal_device(
            &self.header.vault_id,
            &self.vault_key,
            shared_secret,
            ephemeral_public_key,
            enclave_public_key,
        )
    }

    /// Locks the vault: closes the database (releasing it even if the
    /// checkpoint fails) and zeroizes the keys.
    pub fn lock(mut self) -> Result<()> {
        match self.store.take() {
            Some(store) => store.close(),
            None => Ok(()),
        }
    }

    /// Deletes the vault, its inbox and quarantine, and interrupted
    /// creations. The caller must obtain this `UnlockedVault` from a fresh
    /// password or Face ID unlock, stop collectors first, and remove the
    /// device Keychain items. Missing files are not errors, so a
    /// [`VaultError::PartialDeletion`] can be retried with a new unlock, or
    /// with [`delete_files`] if the header is already gone.
    pub fn delete(mut self) -> Result<()> {
        if let Some(store) = self.store.take() {
            let _ = store.close();
        }
        delete_files(&self.paths)
    }

    fn set_password(&mut self, password: &[u8], params: Argon2Params) -> Result<()> {
        let kdf = KdfParams {
            argon2: params,
            salt: random_bytes()?,
        };
        let mut header = self.header.clone();
        header.password_envelope =
            password_envelope(&header.vault_id, &self.vault_key, password, &kdf)?;
        header.kdf = kdf;
        self.write_header(header)
    }

    /// Durably replaces the header: exclusive temp file, fsync, rename,
    /// directory fsync. A crash leaves the old or the new complete header.
    fn write_header(&mut self, header: Header) -> Result<()> {
        let tmp = format!(".header-{}.tmp", random_suffix()?);
        write_atomic(
            &self.paths.vault_dir(),
            &tmp,
            &self.paths.header(),
            &header.encode(&self.vault_key),
        )?;
        self.header = header;
        Ok(())
    }
}

/// Removes all vault files under `paths`. Used by [`UnlockedVault::delete`];
/// call directly only to finish a deletion whose header is already gone.
pub fn delete_files(paths: &VaultPaths) -> Result<()> {
    let mut ok = true;
    for dir in [paths.vault_dir(), paths.inbox(), paths.quarantine()] {
        ok &= remove_dir_if_present(&dir);
    }
    ok &= remove_interrupted_creations(paths).is_ok();
    if paths.root().exists() {
        ok &= sync_dir(paths.root()).is_ok();
    }
    if ok {
        Ok(())
    } else {
        Err(VaultError::PartialDeletion)
    }
}

fn finish_unlock(
    paths: &VaultPaths,
    unverified: UnverifiedHeader,
    vault_key: SecretKey,
) -> Result<UnlockedVault> {
    let header = unverified.verify(&vault_key)?;
    let ingest_key = envelope::unwrap(
        &derive_subkey(&vault_key, labels::INGEST_WRAP),
        &header.ingest_private_envelope,
        &envelope::ingest_aad(&header.vault_id),
    )
    .map_err(|_| VaultError::IntegrityFailure)?;
    let ingest_private = IngestPrivateKey::from_bytes(ingest_key.expose_secret())
        .map_err(|_| VaultError::IntegrityFailure)?;
    if ingest_private.public_key() != header.ingest_public_key {
        return Err(VaultError::IntegrityFailure);
    }
    let store = Store::open(&paths.database(), &sqlcipher_key(&vault_key))?;
    Ok(UnlockedVault {
        paths: paths.clone(),
        header,
        vault_key,
        ingest_private,
        store: Some(store),
    })
}

fn derive_password_kek(password: &[u8], kdf: &KdfParams) -> Result<SecretKey> {
    // Parameters were range-checked when the header was parsed; derive_key
    // checks again before allocating.
    let _guard = KDF_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Ok(kdf::derive_key(password, &kdf.salt, kdf.argon2)?)
}

fn password_envelope(
    vault_id: &VaultId,
    vault_key: &SecretKey,
    password: &[u8],
    kdf: &KdfParams,
) -> Result<[u8; header::ENVELOPE_LEN]> {
    let kek = derive_password_kek(password, kdf)?;
    envelope::wrap(
        &kek,
        vault_key.expose_secret(),
        &envelope::password_aad(vault_id, &kdf.argon2, &kdf.salt),
    )
}

fn recovery_kek(recovery: &RecoverySecret) -> SecretKey {
    derive_subkey(recovery.key(), labels::RECOVERY)
}

fn sqlcipher_key(vault_key: &SecretKey) -> SecretKey {
    derive_subkey(vault_key, labels::SQLCIPHER)
}

fn remove_interrupted_creations(paths: &VaultPaths) -> io::Result<()> {
    let entries = match fs::read_dir(paths.root()) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(CREATE_PREFIX)
        {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

fn remove_dir_if_present(dir: &Path) -> bool {
    match fs::remove_dir_all(dir) {
        Ok(()) => true,
        Err(e) => e.kind() == io::ErrorKind::NotFound,
    }
}

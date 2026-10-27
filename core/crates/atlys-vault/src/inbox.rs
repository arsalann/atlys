//! Sealed inbox version 2 (RFC-0002 "Sealed inbox version 2").
//!
//! Collectors seal batches to the vault's ingest **public** key and drop them
//! in `inbox/` as `<ulid>.sealed`. Only the unlocked vault holds the private
//! key, so background code can add data but never read it.
//!
//! ```text
//! offset  size  field
//! 0       8     magic "ATLYSINB"
//! 8       1     format version (2)
//! 9       1     suite (1 = X25519 / HKDF-SHA256 / ChaCha20-Poly1305)
//! 10      16    recipient vault ID
//! 26      32    HPKE encapsulated key
//! 58      ..    ciphertext || 16-byte tag
//! ```
//!
//! HPKE `info` is `"atlys/v2/inbox" || 0x00 || vault_id` and the AAD is bytes
//! `[0..26)`. The plaintext is `BE32(json_length) || batch_json || zeros`,
//! padded to the smallest bucket that fits.
//!
//! Readers sort failures into three kinds ([`InboxError`]): permanently bad
//! files go to quarantine, files from a newer version stay pending until the
//! app is updated, and I/O errors are transient.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use atlys_adm::{Batch, ErrorKind};
use atlys_crypto::hpke::{
    self, ENCAPPED_KEY_LEN, IngestPrivateKey, IngestPublicKey, SealedMessage, TAG_LEN,
};
use zeroize::Zeroizing;

use crate::fsutil::{random_suffix, read_regular_bounded, sync_dir, write_atomic};
use crate::header::{VAULT_ID_LEN, VaultId};
use crate::{Result, VaultError, ulid};

/// File magic.
pub const MAGIC: &[u8; 8] = b"ATLYSINB";
/// Inbox format version written and read by this code.
pub const FORMAT_VERSION: u8 = 2;
/// HPKE suite 1: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20-Poly1305.
pub const SUITE_X25519_SHA256_CHACHA20POLY1305: u8 = 1;
/// Prefix of the HPKE `info`, followed by 0x00 and the vault ID.
pub const HPKE_INFO_PREFIX: &[u8] = b"atlys/v2/inbox";
/// File extension of sealed batches.
pub const EXTENSION: &str = "sealed";
/// File extension of quarantined files.
pub const QUARANTINE_EXTENSION: &str = "rejected";
/// Largest complete inbox file.
pub const MAX_FILE_LEN: usize = 64 * MIB;
/// Combined size budget of the inbox and quarantine directories.
pub const STORAGE_BUDGET: u64 = 256 * MIB as u64;

const HEADER_LEN: usize = MAGIC.len() + 2 + VAULT_ID_LEN;
const PREFIX_LEN: usize = HEADER_LEN + ENCAPPED_KEY_LEN;
const LENGTH_FIELD: usize = 4;
const MIB: usize = 1024 * 1024;
const BUCKETS: [usize; 6] = [1024, 4 * 1024, 16 * 1024, 64 * 1024, 256 * 1024, MIB];

/// Why a file was rejected. The codes are generic and contain no record data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RejectReason {
    /// Larger than [`MAX_FILE_LEN`].
    TooLarge,
    /// Too short to hold a header, key, and tag.
    TooShort,
    /// Not an inbox file.
    BadMagic,
    /// An older format (the M0 version 1) that product readers never open.
    ObsoleteVersion,
    /// Sealed for a different vault.
    WrongVault,
    /// Failed HPKE authentication.
    Unauthentic,
    /// Bad length prefix, nonzero padding, or a size that is not a bucket.
    BadPadding,
    /// The batch failed validation.
    InvalidBatch,
    /// A symlink or other non-regular file.
    NotRegularFile,
}

impl RejectReason {
    /// A stable code for status display and quarantine file names.
    pub fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "too-large",
            Self::TooShort => "too-short",
            Self::BadMagic => "bad-magic",
            Self::ObsoleteVersion => "obsolete-version",
            Self::WrongVault => "wrong-vault",
            Self::Unauthentic => "unauthentic",
            Self::BadPadding => "bad-padding",
            Self::InvalidBatch => "invalid-batch",
            Self::NotRegularFile => "not-regular-file",
        }
    }
}

/// Failure to read an inbox file.
#[derive(Debug, thiserror::Error)]
pub enum InboxError {
    /// Permanently malformed or unauthentic: move it to quarantine.
    #[error("inbox file rejected: {}", .0.code())]
    Rejected(RejectReason),
    /// Written by a newer version or suite: leave it pending and report that
    /// an update is needed.
    #[error("inbox file needs a newer version of Atlys")]
    NeedsUpdate,
    /// A transient I/O failure (including a locked device): leave it pending.
    #[error("inbox i/o error: {0}")]
    Io(#[from] io::Error),
}

fn header(vault_id: &VaultId) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[..8].copy_from_slice(MAGIC);
    h[8] = FORMAT_VERSION;
    h[9] = SUITE_X25519_SHA256_CHACHA20POLY1305;
    h[10..].copy_from_slice(vault_id.as_bytes());
    h
}

fn hpke_info(vault_id: &VaultId) -> Vec<u8> {
    let mut info = HPKE_INFO_PREFIX.to_vec();
    info.push(0);
    info.extend_from_slice(vault_id.as_bytes());
    info
}

/// Size the padded plaintext occupies for a batch of `json_len` bytes.
pub fn padded_len(json_len: usize) -> usize {
    let needed = json_len + LENGTH_FIELD;
    BUCKETS
        .iter()
        .copied()
        .find(|b| *b >= needed)
        .unwrap_or_else(|| needed.div_ceil(MIB) * MIB)
}

fn is_bucket(len: usize) -> bool {
    BUCKETS.contains(&len) || (len > MIB && len.is_multiple_of(MIB))
}

/// Encodes a batch into the bytes of an inbox file.
pub fn seal_batch(
    vault_id: &VaultId,
    public_key: &IngestPublicKey,
    batch: &Batch,
) -> Result<Vec<u8>> {
    let json = Zeroizing::new(batch.to_json());
    let padded_size = padded_len(json.len());
    // Enforce the file limit before allocating the padding.
    if PREFIX_LEN + padded_size + TAG_LEN > MAX_FILE_LEN {
        return Err(VaultError::InvalidInboxFile("batch too large"));
    }
    let len = u32::try_from(json.len()).expect("bounded by MAX_FILE_LEN");
    let mut padded = Zeroizing::new(vec![0u8; padded_size]);
    padded[..LENGTH_FIELD].copy_from_slice(&len.to_be_bytes());
    padded[LENGTH_FIELD..LENGTH_FIELD + json.len()].copy_from_slice(&json);

    let header = header(vault_id);
    let sealed = hpke::seal(public_key, &hpke_info(vault_id), &header, &padded)?;
    let mut out = Vec::with_capacity(PREFIX_LEN + sealed.ciphertext.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&sealed.encapped_key);
    out.extend_from_slice(&sealed.ciphertext);
    Ok(out)
}

/// Decodes and authenticates the bytes of an inbox file for `vault_id`.
/// Header fields are checked before any decryption.
pub fn open_batch(
    vault_id: &VaultId,
    private_key: &IngestPrivateKey,
    file: &[u8],
) -> core::result::Result<Batch, InboxError> {
    use RejectReason::*;
    let reject = |r| Err(InboxError::Rejected(r));
    if file.len() > MAX_FILE_LEN {
        return reject(TooLarge);
    }
    if file.len() < 10 {
        return reject(TooShort);
    }
    if &file[..8] != MAGIC {
        return reject(BadMagic);
    }
    match file[8] {
        v if v < FORMAT_VERSION => return reject(ObsoleteVersion),
        v if v > FORMAT_VERSION => return Err(InboxError::NeedsUpdate),
        _ => {}
    }
    if file[9] != SUITE_X25519_SHA256_CHACHA20POLY1305 {
        // Suites are only added by newer versions; keep the data.
        return Err(InboxError::NeedsUpdate);
    }
    if file.len() < PREFIX_LEN + TAG_LEN + LENGTH_FIELD {
        return reject(TooShort);
    }
    if &file[10..HEADER_LEN] != vault_id.as_bytes() {
        return reject(WrongVault);
    }
    let (header, rest) = file.split_at(HEADER_LEN);
    let (enc, ciphertext) = rest.split_at(ENCAPPED_KEY_LEN);
    let message = SealedMessage {
        encapped_key: enc.try_into().expect("split at ENCAPPED_KEY_LEN"),
        ciphertext: ciphertext.to_vec(),
    };
    let padded = hpke::open(private_key, &hpke_info(vault_id), header, &message)
        .map_err(|_| InboxError::Rejected(Unauthentic))?;
    if !is_bucket(padded.len()) {
        return reject(BadPadding);
    }
    let len = u32::from_be_bytes(padded[..LENGTH_FIELD].try_into().expect("4 bytes")) as usize;
    let Some(json) = padded.get(LENGTH_FIELD..LENGTH_FIELD + len) else {
        return reject(BadPadding);
    };
    if padded_len(len) != padded.len() || padded[LENGTH_FIELD + len..].iter().any(|b| *b != 0) {
        return reject(BadPadding);
    }
    Batch::from_json(json).map_err(|e| match e.kind {
        ErrorKind::UnsupportedVersion(_) => InboxError::NeedsUpdate,
        _ => InboxError::Rejected(InvalidBatch),
    })
}

/// Writes sealed batches into the inbox. Needs only the public key, so it is
/// safe to use from background code while the vault is locked.
#[derive(Debug)]
pub struct InboxWriter {
    inbox_dir: PathBuf,
    quarantine_dir: PathBuf,
    vault_id: VaultId,
    public_key: IngestPublicKey,
}

impl InboxWriter {
    /// Creates a writer, creating `inbox_dir` if needed. On iOS the app sets
    /// the directory's file protection class, which new files inherit.
    pub fn new(
        inbox_dir: impl Into<PathBuf>,
        quarantine_dir: impl Into<PathBuf>,
        vault_id: VaultId,
        public_key: IngestPublicKey,
    ) -> Result<Self> {
        let inbox_dir = inbox_dir.into();
        fs::create_dir_all(&inbox_dir)?;
        Ok(Self {
            inbox_dir,
            quarantine_dir: quarantine_dir.into(),
            vault_id,
            public_key,
        })
    }

    /// Seals `batch` and writes it durably: exclusive temporary file, fsync,
    /// rename, fsync of the directory. Fails with [`VaultError::InboxFull`]
    /// when the inbox and quarantine together exceed [`STORAGE_BUDGET`].
    pub fn write(&self, batch: &Batch) -> Result<PathBuf> {
        let bytes = seal_batch(&self.vault_id, &self.public_key, batch)?;
        let used = dir_size(&self.inbox_dir)? + dir_size(&self.quarantine_dir)?;
        if used + bytes.len() as u64 > STORAGE_BUDGET {
            return Err(VaultError::InboxFull);
        }
        let name = ulid::new()?;
        let path = self.inbox_dir.join(format!("{name}.{EXTENSION}"));
        let tmp = format!(".{name}-{}.tmp", random_suffix()?);
        write_atomic(&self.inbox_dir, &tmp, &path, &bytes)?;
        Ok(path)
    }
}

/// Lists sealed batches in `dir`, oldest first. Temporary files, symlinks,
/// and other entries are ignored.
pub fn pending(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let named_sealed = path.extension().is_some_and(|e| e == EXTENSION)
            && !entry.file_name().to_string_lossy().starts_with('.');
        if named_sealed {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Reads and opens one inbox file without following symlinks or trusting
/// the metadata size.
pub fn read(
    vault_id: &VaultId,
    private_key: &IngestPrivateKey,
    path: &Path,
) -> core::result::Result<Batch, InboxError> {
    let bytes = match read_regular_bounded(path, MAX_FILE_LEN) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Err(InboxError::Rejected(RejectReason::TooLarge)),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => {
            return Err(InboxError::Rejected(RejectReason::NotRegularFile));
        }
        Err(e) => return Err(e.into()),
    };
    open_batch(vault_id, private_key, &bytes)
}

/// Moves a rejected file into `quarantine_dir` as `<name>.<code>.rejected`.
/// The ciphertext is kept; it is never retried automatically or exported.
pub fn quarantine(path: &Path, quarantine_dir: &Path, reason: RejectReason) -> io::Result<PathBuf> {
    fs::create_dir_all(quarantine_dir)?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dest = quarantine_dir.join(format!("{stem}.{}.{QUARANTINE_EXTENSION}", reason.code()));
    fs::rename(path, &dest)?;
    sync_dir(quarantine_dir)?;
    if let Some(parent) = path.parent() {
        sync_dir(parent)?;
    }
    Ok(dest)
}

/// Counts quarantined files by reason code.
pub fn quarantine_counts(quarantine_dir: &Path) -> io::Result<BTreeMap<String, u32>> {
    let mut counts = BTreeMap::new();
    let entries = match fs::read_dir(quarantine_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(counts),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_suffix(&format!(".{QUARANTINE_EXTENSION}"))
            && let Some((_, code)) = rest.rsplit_once('.')
        {
            *counts.entry(code.to_owned()).or_insert(0) += 1;
        }
    }
    Ok(counts)
}

fn dir_size(dir: &Path) -> io::Result<u64> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut total = 0;
    for entry in entries {
        let meta = entry?.metadata()?;
        if meta.is_file() {
            total += meta.len();
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use atlys_adm::RecordDraft;
    use atlys_crypto::hpke::IngestKeyPair;

    use super::*;

    const ID: VaultId = VaultId::from_bytes([0x11; 16]);

    fn batch(n: usize) -> Batch {
        let records = (0..n)
            .map(|i| {
                RecordDraft::from_value(&serde_json::json!({
                    "type": "location.visit",
                    "source_id": format!("synthetic-{i}"),
                    "start_ms": 1_700_000_000_000_i64 + i as i64,
                    "data": { "latitude": 0.0, "longitude": 0.0, "horizontal_accuracy_m": 50.0 },
                }))
                .unwrap()
            })
            .collect();
        Batch::new("location".into(), 1_700_000_000_000, records, vec![]).unwrap()
    }

    fn rejected(r: core::result::Result<Batch, InboxError>) -> RejectReason {
        match r {
            Err(InboxError::Rejected(reason)) => reason,
            other => panic!("expected rejection, got {other:?}"),
        }
    }

    /// Seals arbitrary plaintext with a valid header, for padding tests.
    fn seal_raw(kp: &IngestKeyPair, plaintext: &[u8]) -> Vec<u8> {
        let header = header(&ID);
        let sealed = hpke::seal(&kp.public, &hpke_info(&ID), &header, plaintext).unwrap();
        [&header[..], &sealed.encapped_key, &sealed.ciphertext].concat()
    }

    #[test]
    fn writer_and_reader_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let kp = IngestKeyPair::generate();
        let inbox = dir.path().join("inbox");
        let writer =
            InboxWriter::new(&inbox, dir.path().join("quarantine"), ID, kp.public.clone()).unwrap();
        let first = writer.write(&batch(1)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = writer.write(&batch(3)).unwrap();
        let files = pending(&inbox).unwrap();
        assert_eq!(files, vec![first, second]);
        assert_eq!(read(&ID, &kp.private, &files[0]).unwrap(), batch(1));
        assert_eq!(read(&ID, &kp.private, &files[1]).unwrap().records.len(), 3);
    }

    #[test]
    fn header_layout_and_info_match_rfc_0002_vectors() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../spec/vault/v1/vectors.json")).unwrap();
        let id = VaultId::from_bytes(core::array::from_fn(|i| i as u8));
        let hex = |v: &serde_json::Value| crate::hex::decode_vec(v.as_str().unwrap()).unwrap();
        assert_eq!(header(&id).to_vec(), hex(&fixture["inbox_header_hex"]));
        assert_eq!(hpke_info(&id), hex(&fixture["inbox_info_hex"]));
    }

    #[test]
    fn other_vaults_and_wrong_keys_are_rejected() {
        let kp = IngestKeyPair::generate();
        let file = seal_batch(&ID, &kp.public, &batch(1)).unwrap();
        let other = VaultId::from_bytes([0x22; 16]);
        assert_eq!(
            rejected(open_batch(&other, &kp.private, &file)),
            RejectReason::WrongVault
        );
        // Rewriting the vault ID in the header breaks authentication.
        let mut moved = file.clone();
        moved[10..26].copy_from_slice(other.as_bytes());
        assert_eq!(
            rejected(open_batch(&other, &kp.private, &moved)),
            RejectReason::Unauthentic
        );
        let stranger = IngestKeyPair::generate();
        assert_eq!(
            rejected(open_batch(&ID, &stranger.private, &file)),
            RejectReason::Unauthentic
        );
    }

    #[test]
    fn every_tampered_byte_is_rejected() {
        let kp = IngestKeyPair::generate();
        let file = seal_batch(&ID, &kp.public, &batch(1)).unwrap();
        for pos in (0..file.len())
            .step_by(37)
            .chain([0, 8, 9, 10, 25, 26, 57, 58, file.len() - 1])
        {
            let mut tampered = file.clone();
            tampered[pos] ^= 0x01;
            assert!(
                open_batch(&ID, &kp.private, &tampered).is_err(),
                "byte {pos}"
            );
        }
        assert!(open_batch(&ID, &kp.private, &file[..file.len() - 1]).is_err());
        assert_eq!(
            rejected(open_batch(&ID, &kp.private, &[])),
            RejectReason::TooShort
        );
    }

    #[test]
    fn versions_and_suites_are_classified() {
        let kp = IngestKeyPair::generate();
        let file = seal_batch(&ID, &kp.public, &batch(1)).unwrap();
        let with = |pos: usize, value: u8| {
            let mut f = file.clone();
            f[pos] = value;
            open_batch(&ID, &kp.private, &f)
        };
        assert_eq!(rejected(with(8, 1)), RejectReason::ObsoleteVersion);
        assert!(matches!(with(8, 3), Err(InboxError::NeedsUpdate)));
        assert!(matches!(with(9, 2), Err(InboxError::NeedsUpdate)));
        assert_eq!(rejected(with(0, b'X')), RejectReason::BadMagic);
    }

    #[test]
    fn m0_version_1_files_are_never_opened() {
        let kp = IngestKeyPair::generate();
        let draft = atlys_adm::draft::Batch::new("location".into(), 1, vec![]).unwrap();
        let v1 = crate::m0::inbox::seal_batch(&kp.public, &draft).unwrap();
        assert_eq!(
            rejected(open_batch(&ID, &kp.private, &v1)),
            RejectReason::ObsoleteVersion
        );
    }

    #[test]
    fn padding_is_validated() {
        let kp = IngestKeyPair::generate();
        let json = batch(1).to_json();
        let mut good = vec![0u8; 1024];
        good[..4].copy_from_slice(&(json.len() as u32).to_be_bytes());
        good[4..4 + json.len()].copy_from_slice(&json);
        assert!(open_batch(&ID, &kp.private, &seal_raw(&kp, &good)).is_ok());

        let mut nonzero = good.clone();
        nonzero[1023] = 1;
        let mut not_bucket = good.clone();
        not_bucket.push(0);
        let mut overlong = good.clone();
        overlong[..4].copy_from_slice(&2000u32.to_be_bytes());
        let mut wrong_bucket = vec![0u8; 4096];
        wrong_bucket[..4 + json.len()].copy_from_slice(&good[..4 + json.len()]);
        for (case, what) in [
            (nonzero, "nonzero padding"),
            (not_bucket, "non-bucket size"),
            (overlong, "length past end"),
            (wrong_bucket, "larger bucket than needed"),
        ] {
            assert_eq!(
                rejected(open_batch(&ID, &kp.private, &seal_raw(&kp, &case))),
                RejectReason::BadPadding,
                "{what}"
            );
        }
        let mut bad_json = vec![0u8; 1024];
        bad_json[..4].copy_from_slice(&3u32.to_be_bytes());
        bad_json[4..7].copy_from_slice(b"{x}");
        assert_eq!(
            rejected(open_batch(&ID, &kp.private, &seal_raw(&kp, &bad_json))),
            RejectReason::InvalidBatch
        );
    }

    #[test]
    fn newer_adm_version_waits_for_an_update() {
        let kp = IngestKeyPair::generate();
        let json = br#"{"adm_version":"9.0","collector":"health","created_ms":0,"records":[]}"#;
        let mut plain = vec![0u8; 1024];
        plain[..4].copy_from_slice(&(json.len() as u32).to_be_bytes());
        plain[4..4 + json.len()].copy_from_slice(json);
        assert!(matches!(
            open_batch(&ID, &kp.private, &seal_raw(&kp, &plain)),
            Err(InboxError::NeedsUpdate)
        ));
    }

    #[test]
    fn sizes_are_bucketed() {
        assert_eq!(padded_len(0), 1024);
        assert_eq!(padded_len(1020), 1024);
        assert_eq!(padded_len(1021), 4096);
        assert_eq!(padded_len(MIB), 2 * MIB);
        let kp = IngestKeyPair::generate();
        let small = seal_batch(&ID, &kp.public, &batch(1)).unwrap();
        let bigger = seal_batch(&ID, &kp.public, &batch(3)).unwrap();
        assert_eq!(small.len(), bigger.len(), "same bucket, same file size");
        assert_eq!(small.len(), PREFIX_LEN + 1024 + TAG_LEN);
    }

    #[test]
    fn pending_ignores_temp_files_and_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".01ABC-00.tmp"), b"partial").unwrap();
        fs::write(dir.path().join(".hidden.sealed"), b"x").unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        assert!(pending(dir.path()).unwrap().is_empty());
        assert!(pending(&dir.path().join("nope")).unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let kp = IngestKeyPair::generate();
        let target = dir.path().join("elsewhere");
        fs::write(&target, seal_batch(&ID, &kp.public, &batch(1)).unwrap()).unwrap();
        let link = dir.path().join("01LINK.sealed");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            rejected(read(&ID, &kp.private, &link)),
            RejectReason::NotRegularFile
        );
    }

    #[test]
    fn quarantine_moves_files_and_counts_reasons() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        let q = dir.path().join("quarantine");
        fs::create_dir_all(&inbox).unwrap();
        for (name, reason) in [
            ("01A.sealed", RejectReason::Unauthentic),
            ("01B.sealed", RejectReason::Unauthentic),
            ("01C.sealed", RejectReason::WrongVault),
        ] {
            fs::write(inbox.join(name), b"ciphertext").unwrap();
            quarantine(&inbox.join(name), &q, reason).unwrap();
        }
        assert!(pending(&inbox).unwrap().is_empty());
        let counts = quarantine_counts(&q).unwrap();
        assert_eq!(counts.get("unauthentic"), Some(&2));
        assert_eq!(counts.get("wrong-vault"), Some(&1));
        assert!(q.join("01A.unauthentic.rejected").exists());
    }

    #[test]
    fn writer_respects_storage_budget() {
        let dir = tempfile::tempdir().unwrap();
        let kp = IngestKeyPair::generate();
        let q = dir.path().join("quarantine");
        fs::create_dir_all(&q).unwrap();
        // A sparse file counts at its full length.
        let filler = fs::File::create(q.join("big.x.rejected")).unwrap();
        filler.set_len(STORAGE_BUDGET).unwrap();
        let writer = InboxWriter::new(dir.path().join("inbox"), &q, ID, kp.public.clone()).unwrap();
        assert!(matches!(
            writer.write(&batch(1)),
            Err(VaultError::InboxFull)
        ));
        assert!(pending(&dir.path().join("inbox")).unwrap().is_empty());
    }
}

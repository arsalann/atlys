//! The write-only ingest inbox.
//!
//! Collectors seal batches to the vault's ingest **public** key and drop them
//! in `inbox/` as `<ulid>.sealed`. Only the unlocked vault holds the private
//! key, so background code can add data but never read it. See
//! `plan/v0/03-ios-app.md#the-write-only-ingest-inbox`.
//!
//! M0 file format (version 1, superseded by RFC-0002 inbox version 2):
//!
//! ```text
//! offset  size  field
//! 0       8     magic "ATLYSINB"
//! 8       1     format version (1)
//! 9       1     HPKE suite (1 = X25519 / HKDF-SHA256 / ChaCha20-Poly1305)
//! 10      32    HPKE encapsulated key
//! 42      ..    HPKE ciphertext || 16-byte tag
//! ```
//!
//! HPKE `info` is `"atlys/v1/inbox"` and the AAD is the 10-byte header. The
//! plaintext is `u32 big-endian length || batch JSON || zero padding`, padded
//! up to a fixed size bucket so file sizes reveal little about their contents.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use atlys_adm::draft::Batch;
use atlys_crypto::hpke::{
    self, ENCAPPED_KEY_LEN, IngestPrivateKey, IngestPublicKey, SealedMessage, TAG_LEN,
};
use zeroize::Zeroizing;

use crate::{Result, VaultError, ulid};

/// File magic.
pub const MAGIC: &[u8; 8] = b"ATLYSINB";
/// Current inbox format version.
pub const FORMAT_VERSION: u8 = 1;
/// HPKE suite identifier for X25519 / HKDF-SHA256 / ChaCha20-Poly1305.
pub const SUITE_X25519_SHA256_CHACHA20POLY1305: u8 = 1;
/// HPKE `info` (domain separation) for inbox files.
pub const HPKE_INFO: &[u8] = b"atlys/v1/inbox";
/// File extension of sealed batches.
pub const EXTENSION: &str = "sealed";

const HEADER_LEN: usize = MAGIC.len() + 2;
const PREFIX_LEN: usize = HEADER_LEN + ENCAPPED_KEY_LEN;
const LENGTH_FIELD: usize = 4;
const MIB: usize = 1024 * 1024;
/// Padding buckets for the plaintext. Larger batches round up to whole MiB.
const BUCKETS: [usize; 6] = [1024, 4 * 1024, 16 * 1024, 64 * 1024, 256 * 1024, MIB];
/// Largest inbox file accepted when reading, to bound memory use.
pub const MAX_FILE_LEN: usize = 64 * MIB;

fn header() -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[..MAGIC.len()].copy_from_slice(MAGIC);
    h[MAGIC.len()] = FORMAT_VERSION;
    h[MAGIC.len() + 1] = SUITE_X25519_SHA256_CHACHA20POLY1305;
    h
}

/// Size the padded plaintext occupies for a payload of `len` bytes.
pub fn padded_len(len: usize) -> usize {
    let needed = len + LENGTH_FIELD;
    BUCKETS
        .iter()
        .copied()
        .find(|b| *b >= needed)
        .unwrap_or_else(|| needed.div_ceil(MIB) * MIB)
}

/// Encodes a batch into the bytes of an inbox file.
pub fn seal_batch(public_key: &IngestPublicKey, batch: &Batch) -> Result<Vec<u8>> {
    let json = Zeroizing::new(batch.to_json()?);
    let len =
        u32::try_from(json.len()).map_err(|_| VaultError::InvalidInboxFile("batch too large"))?;
    let mut padded = Zeroizing::new(vec![0u8; padded_len(json.len())]);
    padded[..LENGTH_FIELD].copy_from_slice(&len.to_be_bytes());
    padded[LENGTH_FIELD..LENGTH_FIELD + json.len()].copy_from_slice(&json);

    let header = header();
    let sealed = hpke::seal(public_key, HPKE_INFO, &header, &padded)?;
    let mut out = Vec::with_capacity(PREFIX_LEN + sealed.ciphertext.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&sealed.encapped_key);
    out.extend_from_slice(&sealed.ciphertext);
    Ok(out)
}

/// Decodes and authenticates the bytes of an inbox file.
pub fn open_batch(private_key: &IngestPrivateKey, file: &[u8]) -> Result<Batch> {
    if file.len() > MAX_FILE_LEN {
        return Err(VaultError::InvalidInboxFile("file too large"));
    }
    if file.len() < PREFIX_LEN + TAG_LEN + LENGTH_FIELD {
        return Err(VaultError::InvalidInboxFile("file too short"));
    }
    let (header, rest) = file.split_at(HEADER_LEN);
    if &header[..MAGIC.len()] != MAGIC {
        return Err(VaultError::InvalidInboxFile("bad magic"));
    }
    if header[MAGIC.len()] != FORMAT_VERSION {
        return Err(VaultError::InvalidInboxFile("unsupported format version"));
    }
    if header[MAGIC.len() + 1] != SUITE_X25519_SHA256_CHACHA20POLY1305 {
        return Err(VaultError::InvalidInboxFile("unsupported HPKE suite"));
    }
    let (enc, ciphertext) = rest.split_at(ENCAPPED_KEY_LEN);
    let message = SealedMessage {
        encapped_key: enc.try_into().expect("split at ENCAPPED_KEY_LEN"),
        ciphertext: ciphertext.to_vec(),
    };
    let padded = hpke::open(private_key, HPKE_INFO, header, &message)
        .map_err(|_| VaultError::InvalidInboxFile("authentication failed"))?;
    let len = u32::from_be_bytes(padded[..LENGTH_FIELD].try_into().expect("4 bytes")) as usize;
    let json = padded
        .get(LENGTH_FIELD..LENGTH_FIELD + len)
        .ok_or(VaultError::InvalidInboxFile("bad length"))?;
    Ok(Batch::from_json(json)?)
}

/// Writes sealed batches into an inbox directory. Needs only the public key,
/// so it is safe to use from background code while the vault is locked.
#[derive(Debug)]
pub struct InboxWriter {
    dir: PathBuf,
    public_key: IngestPublicKey,
}

impl InboxWriter {
    /// Creates a writer, creating `dir` if needed. On iOS the app sets the
    /// directory's file protection class, which new files inherit.
    pub fn new(dir: impl Into<PathBuf>, public_key: IngestPublicKey) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir, public_key })
    }

    /// Seals `batch` and writes it atomically (write to a temporary file,
    /// fsync, rename). Returns the path of the new `.sealed` file.
    pub fn write(&self, batch: &Batch) -> Result<PathBuf> {
        let bytes = seal_batch(&self.public_key, batch)?;
        let name = ulid::new()?;
        let tmp = self.dir.join(format!(".{name}.tmp"));
        let path = self.dir.join(format!("{name}.{EXTENSION}"));
        let mut file = File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &path)?;
        Ok(path)
    }
}

/// Lists sealed batches in `dir`, oldest first. Temporary files from
/// interrupted writes are ignored.
pub fn pending(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|ext| ext == EXTENSION))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    files.sort();
    Ok(files)
}

/// Reads and opens one inbox file.
pub fn read(private_key: &IngestPrivateKey, path: &Path) -> Result<Batch> {
    let len = fs::metadata(path)?.len();
    if len > MAX_FILE_LEN as u64 {
        return Err(VaultError::InvalidInboxFile("file too large"));
    }
    open_batch(private_key, &fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use atlys_adm::draft::RecordDraft;
    use atlys_crypto::hpke::IngestKeyPair;

    use super::*;

    fn batch(n: usize) -> Batch {
        let records = (0..n)
            .map(|i| RecordDraft {
                record_type: "location.visit".into(),
                source_id: format!("synthetic-{i}"),
                start_ms: 1_700_000_000_000 + i as i64,
                end_ms: None,
                data: serde_json::json!({ "lat": 0.0, "lon": 0.0, "accuracy_m": 50.0 }),
            })
            .collect();
        Batch::new("location".into(), 1_700_000_000_000, records).unwrap()
    }

    #[test]
    fn writer_and_reader_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let kp = IngestKeyPair::generate();
        let writer = InboxWriter::new(dir.path().join("inbox"), kp.public.clone()).unwrap();
        let first = writer.write(&batch(1)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = writer.write(&batch(3)).unwrap();

        let files = pending(&dir.path().join("inbox")).unwrap();
        assert_eq!(files, vec![first, second]);
        assert_eq!(read(&kp.private, &files[0]).unwrap(), batch(1));
        assert_eq!(read(&kp.private, &files[1]).unwrap().records.len(), 3);
    }

    #[test]
    fn writer_cannot_read_and_other_vaults_cannot_open() {
        let kp = IngestKeyPair::generate();
        let other = IngestKeyPair::generate();
        let file = seal_batch(&kp.public, &batch(1)).unwrap();
        assert!(matches!(
            open_batch(&other.private, &file),
            Err(VaultError::InvalidInboxFile("authentication failed"))
        ));
    }

    #[test]
    fn every_tampered_byte_is_rejected() {
        let kp = IngestKeyPair::generate();
        let file = seal_batch(&kp.public, &batch(1)).unwrap();
        // Flip one bit at a spread of positions across header, enc, and ciphertext.
        for pos in (0..file.len())
            .step_by(37)
            .chain([0, 8, 9, 10, 41, 42, file.len() - 1])
        {
            let mut tampered = file.clone();
            tampered[pos] ^= 0x01;
            assert!(
                open_batch(&kp.private, &tampered).is_err(),
                "byte {pos} not authenticated"
            );
        }
        assert!(open_batch(&kp.private, &file[..file.len() - 1]).is_err());
        assert!(open_batch(&kp.private, &[]).is_err());
    }

    #[test]
    fn sizes_are_bucketed() {
        assert_eq!(padded_len(0), 1024);
        assert_eq!(padded_len(1020), 1024);
        assert_eq!(padded_len(1021), 4096);
        assert_eq!(padded_len(MIB), 2 * MIB);
        let kp = IngestKeyPair::generate();
        let small = seal_batch(&kp.public, &batch(1)).unwrap();
        let bigger = seal_batch(&kp.public, &batch(3)).unwrap();
        assert_eq!(small.len(), bigger.len(), "same bucket, same file size");
        assert_eq!(small.len(), PREFIX_LEN + 1024 + TAG_LEN);
    }

    #[test]
    fn pending_ignores_temp_files_and_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".01ABC.tmp"), b"partial").unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        assert!(pending(dir.path()).unwrap().is_empty());
        assert!(pending(&dir.path().join("nope")).unwrap().is_empty());
    }
}

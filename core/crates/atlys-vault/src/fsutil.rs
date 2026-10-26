//! Durable file operations used by the vault and the inbox.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use atlys_crypto::random_bytes;

use crate::{Result, hex};

/// Syncs a directory so a rename or create inside it is durable.
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    match File::open(dir)?.sync_all() {
        // Some file systems refuse fsync on directories (EINVAL).
        Err(e) if e.kind() == io::ErrorKind::InvalidInput => Ok(()),
        other => other,
    }
}

/// A random suffix for temporary names.
pub(crate) fn random_suffix() -> Result<String> {
    Ok(hex::encode(&random_bytes::<8>()?))
}

/// Writes `bytes` to a new, exclusively created temporary file in `dir`,
/// syncs it, renames it to `dest` (replacing any existing file), and syncs
/// `dir`. A crash leaves either the old or the new complete file, plus at
/// most a stray temporary file that readers ignore.
pub(crate) fn write_atomic(dir: &Path, tmp_name: &str, dest: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = dir.join(tmp_name);
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, dest)?;
        sync_dir(dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Reads a regular file of at most `limit` bytes without following a final
/// symlink and without trusting the metadata size. Returns `Ok(None)` if the
/// file is larger than `limit`.
pub(crate) fn read_regular_bounded(path: &Path, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a regular file",
        ));
    }
    let file = File::open(path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0).min(limit));
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Ok(None);
    }
    Ok(Some(bytes))
}

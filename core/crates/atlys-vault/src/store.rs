//! The SQLCipher-encrypted vault database.
//!
//! SQLCipher is always opened in **raw-key mode**: the 256-bit key comes from
//! the Atlys key hierarchy (HKDF from the vault key), never from a password,
//! so SQLCipher's own PBKDF2 stretching is skipped.

use std::path::Path;

use atlys_crypto::SecretKey;
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use zeroize::Zeroizing;

use crate::{Result, VaultError};

/// Schema version written to `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 1;

/// Details of the linked SQLCipher build, for diagnostics and the spike report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CipherInfo {
    /// SQLCipher version, for example `4.19.0 community`.
    pub cipher_version: String,
    /// Crypto provider: `commoncrypto` on Apple platforms, `openssl` elsewhere.
    pub provider: String,
    /// Version string reported by the provider.
    pub provider_version: String,
    /// The underlying SQLite version.
    pub sqlite_version: String,
}

/// An open, unlocked vault database.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (or creates) the database at `path` with a raw 256-bit key.
    ///
    /// Fails with [`VaultError::WrongKeyOrCorrupt`] if the key does not match
    /// or the file is not a SQLCipher database.
    pub fn open(path: &Path, key: &SecretKey) -> Result<Self> {
        let conn = Connection::open(path).map_err(storage)?;
        apply_key(&conn, key)?;
        // The first read decrypts page 1 and fails if the key is wrong.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(map_open_error)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA secure_delete = ON;",
        )
        .map_err(storage)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Reports the SQLCipher build and crypto provider.
    pub fn cipher_info(&self) -> Result<CipherInfo> {
        let pragma = |name: &str| -> Result<String> {
            self.conn
                .query_row(&format!("PRAGMA {name}"), [], |r| r.get::<_, String>(0))
                .map_err(storage)
        };
        Ok(CipherInfo {
            cipher_version: pragma("cipher_version")?,
            provider: pragma("cipher_provider")?,
            provider_version: pragma("cipher_provider_version")?,
            sqlite_version: self
                .conn
                .query_row("SELECT sqlite_version()", [], |r| r.get(0))
                .map_err(storage)?,
        })
    }

    /// Stores a small metadata value.
    pub fn put_meta(&self, key: &str, value: &[u8]) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// Reads a metadata value.
    pub fn get_meta(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(storage)
    }

    /// Flushes the write-ahead log into the main file and closes the database.
    pub fn close(self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(storage)?;
        self.conn.close().map_err(|(_, e)| storage(e))
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(storage)?;
        if version < 1 {
            // Placeholder schema. The record tables arrive with RFC-0001/0002.
            self.conn
                .execute_batch(&format!(
                    "BEGIN;
                     CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT;
                     PRAGMA user_version = {SCHEMA_VERSION};
                     COMMIT;"
                ))
                .map_err(storage)?;
        }
        Ok(())
    }
}

/// Sets the raw key. The PRAGMA text holds the key in hex, so it lives in a
/// zeroizing buffer. SQLCipher keeps its own copy in locked memory.
fn apply_key(conn: &Connection, key: &SecretKey) -> Result<()> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut pragma = Zeroizing::new(String::with_capacity(80));
    pragma.push_str("PRAGMA key = \"x'");
    for b in key.expose_secret() {
        pragma.push(HEX[usize::from(b >> 4)] as char);
        pragma.push(HEX[usize::from(b & 0x0F)] as char);
    }
    pragma.push_str("'\";");
    conn.execute_batch(&pragma).map_err(storage)
}

fn map_open_error(err: rusqlite::Error) -> VaultError {
    match err.sqlite_error_code() {
        Some(ErrorCode::NotADatabase) => VaultError::WrongKeyOrCorrupt,
        _ => storage(err),
    }
}

fn storage(err: rusqlite::Error) -> VaultError {
    VaultError::Storage(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const MARKER: &[u8] = b"SYNTHETIC-PLAINTEXT-MARKER-0123456789";

    fn key(byte: u8) -> SecretKey {
        SecretKey::from_bytes([byte; 32])
    }

    #[test]
    fn write_close_reopen_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.db");
        let store = Store::open(&path, &key(1)).unwrap();
        store.put_meta("probe", MARKER).unwrap();
        store.close().unwrap();

        let store = Store::open(&path, &key(1)).unwrap();
        assert_eq!(store.get_meta("probe").unwrap().as_deref(), Some(MARKER));
        assert_eq!(store.get_meta("missing").unwrap(), None);
    }

    #[test]
    fn wrong_key_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.db");
        Store::open(&path, &key(1)).unwrap().close().unwrap();
        let err = Store::open(&path, &key(2)).unwrap_err();
        assert!(matches!(err, VaultError::WrongKeyOrCorrupt), "{err:?}");
    }

    #[test]
    fn plain_sqlite_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.db");
        fs::write(&path, b"SQLite format 3\0this is not an encrypted vault").unwrap();
        assert!(matches!(
            Store::open(&path, &key(1)).unwrap_err(),
            VaultError::WrongKeyOrCorrupt
        ));
    }

    #[test]
    fn file_is_unreadable_without_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.db");
        let store = Store::open(&path, &key(1)).unwrap();
        store.put_meta("probe", MARKER).unwrap();
        // Inspect the WAL before checkpointing, then the main file after.
        let wal = fs::read(dir.path().join("vault.db-wal")).unwrap_or_default();
        assert!(!contains(&wal, MARKER), "plaintext found in WAL");
        store.close().unwrap();

        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() >= 4096);
        assert!(
            !bytes.starts_with(b"SQLite format 3\0"),
            "plaintext SQLite header"
        );
        assert!(
            !contains(&bytes, MARKER),
            "plaintext found in database file"
        );
        assert!(
            !contains(&bytes, b"CREATE TABLE"),
            "schema visible in database file"
        );
    }

    #[test]
    fn reports_cipher_provider() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("vault.db"), &key(1)).unwrap();
        let info = store.cipher_info().unwrap();
        assert!(info.cipher_version.starts_with('4'), "{info:?}");
        #[cfg(target_vendor = "apple")]
        assert_eq!(info.provider, "commoncrypto");
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }
}

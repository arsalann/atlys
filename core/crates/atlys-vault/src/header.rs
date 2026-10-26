//! The vault header, `vault/header.json` (RFC-0002 "Header container").
//!
//! The file holds exactly two strings: `payload_hex`, the exact UTF-8 bytes
//! of the inner JSON object, and `mac_hex`, an HMAC-SHA256 tag over those
//! bytes. Authenticating the stored bytes avoids any need for JSON
//! canonicalization. Both layers reject unknown and duplicate fields.
//!
//! Parsing yields an [`UnverifiedHeader`]. Its public metadata (the ingest
//! public key and KDF parameters) may be read before unlock, but nothing else
//! is trusted until [`UnverifiedHeader::verify`] succeeds with the vault key.

use core::fmt;

use atlys_crypto::hkdf::{derive_subkey, labels};
use atlys_crypto::hpke::IngestPublicKey;
use atlys_crypto::kdf::{Argon2Params, SALT_LEN};
use atlys_crypto::{SecretKey, aead, mac, random_bytes};
use serde::{Deserialize, Serialize};

use crate::{Result, VaultError, hex};

/// The header format version this code reads and writes.
pub const FORMAT_VERSION: u32 = 1;
/// Largest accepted header file.
pub const MAX_FILE_LEN: usize = 64 * 1024;
/// Deepest accepted JSON nesting, checked before parsing.
pub const MAX_NESTING: usize = 8;
/// Length of a vault ID.
pub const VAULT_ID_LEN: usize = 16;
/// Length of a password, recovery, or ingest envelope:
/// `nonce[24] || ciphertext[32] || tag[16]`.
pub const ENVELOPE_LEN: usize = aead::OVERHEAD + atlys_crypto::KEY_LEN;

const KDF_ALGORITHM: &str = "argon2id";
const ARGON2_VERSION: u32 = 0x13;

/// A random 16-byte vault identifier. Not secret.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VaultId([u8; VAULT_ID_LEN]);

impl VaultId {
    /// Generates a new random ID.
    pub fn generate() -> Result<Self> {
        Ok(Self(random_bytes()?))
    }

    /// Wraps existing ID bytes.
    pub const fn from_bytes(bytes: [u8; VAULT_ID_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw ID bytes.
    pub fn as_bytes(&self) -> &[u8; VAULT_ID_LEN] {
        &self.0
    }
}

impl fmt::Debug for VaultId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VaultId({})", hex::encode(&self.0))
    }
}

/// Argon2id cost parameters and salt for the password envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Cost parameters, always within the RFC-0002 accepted range.
    pub argon2: Argon2Params,
    /// Random salt, fresh on creation and on each password change.
    pub salt: [u8; SALT_LEN],
}

/// The authenticated contents of a vault header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// The vault's identifier.
    pub vault_id: VaultId,
    /// Password KDF parameters.
    pub kdf: KdfParams,
    /// Vault key wrapped under the Argon2id output.
    pub password_envelope: [u8; ENVELOPE_LEN],
    /// Vault key wrapped under the recovery KEK.
    pub recovery_envelope: [u8; ENVELOPE_LEN],
    /// Public key collectors seal inbox files to.
    pub ingest_public_key: IngestPublicKey,
    /// Ingest private key wrapped under the ingest-wrap subkey.
    pub ingest_private_envelope: [u8; ENVELOPE_LEN],
}

impl Header {
    /// Serializes the header and authenticates it under `vault_key`.
    /// Returns the complete file contents.
    pub fn encode(&self, vault_key: &SecretKey) -> Vec<u8> {
        let raw = RawHeader {
            format_version: FORMAT_VERSION,
            vault_id: hex::encode(self.vault_id.as_bytes()),
            kdf: RawKdf {
                algorithm: KDF_ALGORITHM.into(),
                version: ARGON2_VERSION,
                memory_kib: self.kdf.argon2.memory_kib,
                iterations: self.kdf.argon2.iterations,
                parallelism: self.kdf.argon2.parallelism,
                salt: hex::encode(&self.kdf.salt),
            },
            password_envelope: hex::encode(&self.password_envelope),
            recovery_envelope: hex::encode(&self.recovery_envelope),
            ingest_public_key: hex::encode(&self.ingest_public_key.to_bytes()),
            ingest_private_envelope: hex::encode(&self.ingest_private_envelope),
        };
        let payload = serde_json::to_vec(&raw).expect("header serializes");
        let tag = mac::header_tag(&mac_key(vault_key), &payload);
        let outer = RawContainer {
            payload_hex: hex::encode(&payload),
            mac_hex: hex::encode(&tag),
        };
        let mut file = serde_json::to_vec(&outer).expect("container serializes");
        file.push(b'\n');
        file
    }
}

/// A parsed header whose MAC has not been checked yet.
#[derive(Debug, Clone)]
pub struct UnverifiedHeader {
    header: Header,
    payload: Vec<u8>,
    tag: [u8; mac::TAG_LEN],
}

impl UnverifiedHeader {
    /// Parses header file contents strictly, without authenticating them.
    pub fn parse(file: &[u8]) -> Result<Self> {
        if file.len() > MAX_FILE_LEN {
            return Err(VaultError::InvalidHeader("header file too large"));
        }
        check_nesting(file)?;
        let outer: RawContainer =
            serde_json::from_slice(file).map_err(|_| VaultError::InvalidHeader("bad container"))?;
        let payload = hex::decode_vec(&outer.payload_hex)
            .ok_or(VaultError::InvalidHeader("bad payload encoding"))?;
        let tag = hex::decode_array(&outer.mac_hex)
            .ok_or(VaultError::InvalidHeader("bad MAC encoding"))?;
        check_nesting(&payload)?;
        check_version(&payload)?;
        let raw: RawHeader = serde_json::from_slice(&payload)
            .map_err(|_| VaultError::InvalidHeader("bad payload"))?;
        Ok(Self {
            header: raw.into_header()?,
            payload,
            tag,
        })
    }

    /// The header contents, **not yet authenticated**. Use only for what
    /// RFC-0002 permits before unlock: deriving the password KEK from the KDF
    /// fields, opening an envelope, and sealing inbox files to the ingest key.
    pub fn unverified(&self) -> &Header {
        &self.header
    }

    /// Checks the MAC with a candidate vault key in constant time.
    pub fn verify(self, vault_key: &SecretKey) -> Result<Header> {
        mac::verify_header_tag(&mac_key(vault_key), &self.payload, &self.tag)
            .map_err(|_| VaultError::IntegrityFailure)?;
        Ok(self.header)
    }
}

fn mac_key(vault_key: &SecretKey) -> SecretKey {
    derive_subkey(vault_key, labels::HEADER_MAC)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContainer {
    payload_hex: String,
    mac_hex: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeader {
    format_version: u32,
    vault_id: String,
    kdf: RawKdf,
    password_envelope: String,
    recovery_envelope: String,
    ingest_public_key: String,
    ingest_private_envelope: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKdf {
    algorithm: String,
    version: u32,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    salt: String,
}

impl RawHeader {
    fn into_header(self) -> Result<Header> {
        if self.format_version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion);
        }
        if self.kdf.algorithm != KDF_ALGORITHM || self.kdf.version != ARGON2_VERSION {
            return Err(VaultError::InvalidHeader("unsupported KDF"));
        }
        let argon2 = Argon2Params {
            memory_kib: self.kdf.memory_kib,
            iterations: self.kdf.iterations,
            parallelism: self.kdf.parallelism,
        };
        argon2
            .validate()
            .map_err(|_| VaultError::InvalidHeader("KDF parameters out of range"))?;
        let field = |text: &str, name: &'static str| {
            hex::decode_array::<ENVELOPE_LEN>(text).ok_or(VaultError::InvalidHeader(name))
        };
        let ingest_public_key = hex::decode_array::<32>(&self.ingest_public_key)
            .and_then(|b| IngestPublicKey::from_bytes(&b).ok())
            .ok_or(VaultError::InvalidHeader("bad ingest public key"))?;
        Ok(Header {
            vault_id: VaultId(
                hex::decode_array(&self.vault_id)
                    .ok_or(VaultError::InvalidHeader("bad vault ID"))?,
            ),
            kdf: KdfParams {
                argon2,
                salt: hex::decode_array(&self.kdf.salt)
                    .ok_or(VaultError::InvalidHeader("bad KDF salt"))?,
            },
            password_envelope: field(&self.password_envelope, "bad password envelope")?,
            recovery_envelope: field(&self.recovery_envelope, "bad recovery envelope")?,
            ingest_public_key,
            ingest_private_envelope: field(
                &self.ingest_private_envelope,
                "bad ingest private envelope",
            )?,
        })
    }
}

/// Reports a newer format as [`VaultError::UnsupportedVersion`] rather than a
/// parse error, since a newer header may have different fields.
fn check_version(payload: &[u8]) -> Result<()> {
    #[derive(Deserialize)]
    struct VersionOnly {
        format_version: serde_json::Value,
    }
    if let Ok(v) = serde_json::from_slice::<VersionOnly>(payload)
        && let Some(n) = v.format_version.as_u64()
        && n > u64::from(FORMAT_VERSION)
    {
        return Err(VaultError::UnsupportedVersion);
    }
    Ok(())
}

/// Rejects JSON nested deeper than [`MAX_NESTING`] with a linear scan.
fn check_nesting(json: &[u8]) -> Result<()> {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for &c in json {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_NESTING {
                    return Err(VaultError::InvalidHeader("JSON nested too deeply"));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use atlys_crypto::hpke::IngestKeyPair;

    use super::*;

    fn sample() -> (Header, SecretKey) {
        let header = Header {
            vault_id: VaultId::from_bytes([0xab; 16]),
            kdf: KdfParams {
                argon2: Argon2Params::OWASP_MINIMUM,
                salt: [2; 16],
            },
            password_envelope: [3; ENVELOPE_LEN],
            recovery_envelope: [4; ENVELOPE_LEN],
            ingest_public_key: IngestKeyPair::generate().public,
            ingest_private_envelope: [5; ENVELOPE_LEN],
        };
        (header, SecretKey::from_bytes([9; 32]))
    }

    fn payload_of(file: &[u8]) -> String {
        let outer: serde_json::Value = serde_json::from_slice(file).unwrap();
        String::from_utf8(hex::decode_vec(outer["payload_hex"].as_str().unwrap()).unwrap()).unwrap()
    }

    /// Re-wraps an arbitrary payload in a correctly authenticated container,
    /// so tests reach the inner parser rather than failing on the MAC.
    fn container(payload: &str, key: &SecretKey) -> Vec<u8> {
        let tag = mac::header_tag(&mac_key(key), payload.as_bytes());
        format!(
            r#"{{"payload_hex":"{}","mac_hex":"{}"}}"#,
            hex::encode(payload.as_bytes()),
            hex::encode(&tag)
        )
        .into_bytes()
    }

    #[test]
    fn encode_parse_verify_round_trip() {
        let (header, key) = sample();
        let file = header.encode(&key);
        let parsed = UnverifiedHeader::parse(&file).unwrap();
        assert_eq!(parsed.unverified(), &header);
        assert_eq!(parsed.verify(&key).unwrap(), header);
    }

    #[test]
    fn wrong_key_fails_integrity() {
        let (header, key) = sample();
        let parsed = UnverifiedHeader::parse(&header.encode(&key)).unwrap();
        assert!(matches!(
            parsed.verify(&SecretKey::from_bytes([8; 32])),
            Err(VaultError::IntegrityFailure)
        ));
    }

    #[test]
    fn outer_whitespace_and_order_are_immaterial() {
        let (header, key) = sample();
        let file = header.encode(&key);
        let outer: serde_json::Value = serde_json::from_slice(&file).unwrap();
        let reordered = format!(
            "{{\n  \"mac_hex\" : \"{}\",\n  \"payload_hex\": \"{}\"\n}}",
            outer["mac_hex"].as_str().unwrap(),
            outer["payload_hex"].as_str().unwrap()
        );
        let parsed = UnverifiedHeader::parse(reordered.as_bytes()).unwrap();
        assert_eq!(parsed.verify(&key).unwrap(), header);
    }

    #[test]
    fn inner_whitespace_is_authenticated() {
        let (header, key) = sample();
        let payload = payload_of(&header.encode(&key));
        let spaced = payload.replacen(':', ": ", 1);
        let outer = header.encode(&key);
        let outer: serde_json::Value = serde_json::from_slice(&outer).unwrap();
        let forged = format!(
            r#"{{"payload_hex":"{}","mac_hex":"{}"}}"#,
            hex::encode(spaced.as_bytes()),
            outer["mac_hex"].as_str().unwrap()
        );
        let parsed = UnverifiedHeader::parse(forged.as_bytes()).unwrap();
        assert!(matches!(
            parsed.verify(&key),
            Err(VaultError::IntegrityFailure)
        ));
    }

    #[test]
    fn rejects_malformed_inner_payloads() {
        let (header, key) = sample();
        let good = payload_of(&header.encode(&key));
        let vid = hex::encode(&[0xab; 16]);
        let cases: Vec<(String, &str)> = vec![
            (good.replacen("{", r#"{"extra":1,"#, 1), "unknown field"),
            (
                good.replacen("{", &format!(r#"{{"vault_id":"{vid}","#), 1),
                "duplicate field",
            ),
            (good.replace(&vid, &vid.to_uppercase()), "uppercase hex"),
            (good.replace(&vid, &vid[..30]), "short vault ID"),
            (
                good.replace("\"format_version\":1", "\"format_version\":1.0"),
                "float version",
            ),
            (
                good.replace("\"format_version\":1", "\"format_version\":0"),
                "version 0",
            ),
            (
                good.replace("\"format_version\":1", "\"format_version\":null"),
                "null version",
            ),
            (
                good.replace("\"memory_kib\":19456", "\"memory_kib\":-1"),
                "negative",
            ),
            (
                good.replace("\"memory_kib\":19456", "\"memory_kib\":19455"),
                "memory below range",
            ),
            (
                good.replace("\"memory_kib\":19456", "\"memory_kib\":4194304"),
                "memory above range",
            ),
            (
                good.replace("\"parallelism\":1", "\"parallelism\":4"),
                "lanes",
            ),
            (good.replace("argon2id", "argon2i"), "algorithm"),
            (
                good.replace("\"version\":19", "\"version\":16"),
                "argon2 version",
            ),
            (
                good.replace("\"kdf\":{", "\"kdf\":{\"x\":0,"),
                "unknown kdf field",
            ),
            (
                good.replace(&hex::encode(&[3; 72]), &hex::encode(&[3; 71])),
                "envelope length",
            ),
            (good.replacen("}", "", 1), "truncated"),
            ("[]".into(), "array"),
        ];
        for (payload, what) in cases {
            assert_ne!(payload, good, "{what}: mutation did not apply");
            let file = container(&payload, &key);
            assert!(UnverifiedHeader::parse(&file).is_err(), "{what} accepted");
        }
    }

    #[test]
    fn newer_version_is_reported_as_unsupported() {
        let (_, key) = sample();
        let file = container(r#"{"format_version":2,"future":true}"#, &key);
        assert!(matches!(
            UnverifiedHeader::parse(&file),
            Err(VaultError::UnsupportedVersion)
        ));
    }

    #[test]
    fn rejects_malformed_containers() {
        let (header, key) = sample();
        let file = String::from_utf8(header.encode(&key)).unwrap();
        let cases = [
            file.replacen("{", r#"{"extra":"x","#, 1),
            file.replacen("{", r#"{"mac_hex":"00","#, 1),
            file.replace("mac_hex", "tag_hex"),
            file.replacen("\"payload_hex\":\"", "\"payload_hex\":\"0", 1),
        ];
        for case in cases {
            assert!(UnverifiedHeader::parse(case.as_bytes()).is_err(), "{case}");
        }
        let short_mac = format!(
            r#"{{"payload_hex":"{}","mac_hex":"{}"}}"#,
            hex::encode(b"{}"),
            "00".repeat(16)
        );
        assert!(UnverifiedHeader::parse(short_mac.as_bytes()).is_err());
    }

    #[test]
    fn enforces_size_and_nesting_limits() {
        let big = vec![b' '; MAX_FILE_LEN + 1];
        assert!(matches!(
            UnverifiedHeader::parse(&big),
            Err(VaultError::InvalidHeader("header file too large"))
        ));
        let deep = format!("{}{}", "[".repeat(9), "]".repeat(9));
        assert!(matches!(
            UnverifiedHeader::parse(deep.as_bytes()),
            Err(VaultError::InvalidHeader("JSON nested too deeply"))
        ));
        // Brackets inside strings do not count.
        assert!(check_nesting(br#"{"a":"[[[[[[[[[[[[\"[[["}"#).is_ok());
    }
}

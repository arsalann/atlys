//! End-to-end vault lifecycle tests (RFC-0002 implementation gates).
//!
//! Argon2id runs with the weakest accepted parameters to keep tests fast.

use std::fs;

use atlys_adm::{Batch, RecordDraft};
use atlys_crypto::kdf::Argon2Params;
use atlys_vault::envelope::P256_PUBLIC_KEY_LEN;
use atlys_vault::header::UnverifiedHeader;
use atlys_vault::vault::delete_files;
use atlys_vault::{RecoverySecret, UnlockedVault, Vault, VaultError, VaultPaths, inbox};

const PARAMS: Argon2Params = Argon2Params::OWASP_MINIMUM;
const PASSWORD: &[u8] = b"synthetic correct horse battery";
const NEW_PASSWORD: &[u8] = b"synthetic second passphrase";

fn setup() -> (tempfile::TempDir, VaultPaths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = VaultPaths::new(dir.path().join("container"));
    (dir, paths)
}

fn create(paths: &VaultPaths) -> (UnlockedVault, RecoverySecret) {
    Vault::create(paths, PASSWORD, PARAMS).unwrap()
}

fn unlock(paths: &VaultPaths, password: &[u8]) -> Result<UnlockedVault, VaultError> {
    Vault::unlock_with_password(paths, password)
}

fn p256(byte: u8) -> Vec<u8> {
    let mut k = vec![byte; P256_PUBLIC_KEY_LEN];
    k[0] = 0x04;
    k
}

/// Rewrites one inner header field while keeping the stored MAC, as a
/// filesystem attacker could.
fn tamper_payload(paths: &VaultPaths, edit: impl Fn(&str) -> String) {
    let file: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.header()).unwrap()).unwrap();
    let payload =
        String::from_utf8(hex::decode(file["payload_hex"].as_str().unwrap()).unwrap()).unwrap();
    let edited = edit(&payload);
    assert_ne!(edited, payload, "edit did not change the payload");
    let out = serde_json::json!({
        "payload_hex": hex::encode(edited.as_bytes()),
        "mac_hex": file["mac_hex"],
    });
    fs::write(paths.header(), serde_json::to_vec(&out).unwrap()).unwrap();
}

fn payload_field(paths: &VaultPaths, path: &[&str]) -> serde_json::Value {
    let file: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.header()).unwrap()).unwrap();
    let payload: serde_json::Value =
        serde_json::from_slice(&hex::decode(file["payload_hex"].as_str().unwrap()).unwrap())
            .unwrap();
    path.iter().fold(payload, |v, k| v[*k].clone())
}

fn flip_hex_char(text: &str, needle: &str) -> String {
    let flipped: String = needle
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                if c == '0' { '1' } else { '0' }
            } else {
                c
            }
        })
        .collect();
    text.replacen(needle, &flipped, 1)
}

#[test]
fn create_lock_unlock_keeps_data() {
    let (_dir, paths) = setup();
    assert!(!Vault::exists(&paths));
    let (vault, _) = create(&paths);
    assert!(Vault::exists(&paths));
    vault.store().put_meta("probe", b"synthetic").unwrap();
    let id = vault.vault_id();
    vault.lock().unwrap();

    let vault = unlock(&paths, PASSWORD).unwrap();
    assert_eq!(vault.vault_id(), id);
    assert_eq!(
        vault.store().get_meta("probe").unwrap().as_deref(),
        Some(&b"synthetic"[..])
    );
}

#[test]
fn wrong_password_is_rejected() {
    let (_dir, paths) = setup();
    create(&paths).0.lock().unwrap();
    assert!(matches!(
        unlock(&paths, b"synthetic wrong password"),
        Err(VaultError::WrongCredentials)
    ));
    // Exact bytes: no trimming or normalization.
    let mut padded = PASSWORD.to_vec();
    padded.push(b' ');
    assert!(matches!(
        unlock(&paths, &padded),
        Err(VaultError::WrongCredentials)
    ));
}

#[test]
fn weak_passwords_are_rejected_for_new_passwords_only() {
    let (_dir, paths) = setup();
    assert!(matches!(
        Vault::create(&paths, b"short", PARAMS),
        Err(VaultError::WeakPassword(_))
    ));
    assert!(matches!(
        Vault::create(&paths, &[0xff; 20], PARAMS),
        Err(VaultError::WeakPassword(_))
    ));
    assert!(matches!(
        Vault::create(&paths, b"password1234", PARAMS),
        Err(VaultError::WeakPassword(_))
    ));
    // Non-ASCII passphrases are fine.
    let unicode = "café über naïve ångström".as_bytes();
    let (mut vault, recovery) = Vault::create(&paths, unicode, PARAMS).unwrap();
    for weak in [&b"too short"[..], b"password1234"] {
        assert!(matches!(
            vault.change_password(unicode, weak, PARAMS),
            Err(VaultError::WeakPassword(_))
        ));
    }
    vault.lock().unwrap();
    assert!(matches!(
        Vault::recover(&paths, &recovery, b"short", PARAMS),
        Err(VaultError::WeakPassword(_))
    ));
}

#[test]
fn create_never_replaces_an_existing_vault() {
    let (_dir, paths) = setup();
    let (vault, _) = create(&paths);
    let id = vault.vault_id();
    vault.lock().unwrap();
    assert!(matches!(
        Vault::create(&paths, PASSWORD, PARAMS),
        Err(VaultError::VaultExists)
    ));
    assert_eq!(unlock(&paths, PASSWORD).unwrap().vault_id(), id);
}

#[test]
fn interrupted_creation_is_not_a_vault_and_is_cleaned_up() {
    let (_dir, paths) = setup();
    let stray = paths.root().join(".vault-create-0011223344556677");
    fs::create_dir_all(&stray).unwrap();
    fs::write(stray.join("header.json"), b"partial").unwrap();
    assert!(!Vault::exists(&paths));
    assert!(matches!(unlock(&paths, PASSWORD), Err(VaultError::NoVault)));
    create(&paths).0.lock().unwrap();
    assert!(!stray.exists());
}

#[test]
fn stray_header_temp_files_are_ignored() {
    let (_dir, paths) = setup();
    create(&paths).0.lock().unwrap();
    fs::write(paths.vault_dir().join(".header-deadbeef.tmp"), b"{half").unwrap();
    unlock(&paths, PASSWORD).unwrap();
}

#[test]
fn missing_vault_and_missing_database() {
    let (_dir, paths) = setup();
    assert!(matches!(unlock(&paths, PASSWORD), Err(VaultError::NoVault)));
    create(&paths).0.lock().unwrap();
    fs::remove_file(paths.database()).unwrap();
    assert!(matches!(
        unlock(&paths, PASSWORD),
        Err(VaultError::MissingDatabase)
    ));
    assert!(
        !paths.database().exists(),
        "unlock must not create an empty database"
    );
}

#[test]
fn change_password() {
    let (_dir, paths) = setup();
    let (mut vault, recovery) = create(&paths);
    let old_salt = vault.header().kdf.salt;
    assert!(matches!(
        vault.change_password(b"synthetic wrong password", NEW_PASSWORD, PARAMS),
        Err(VaultError::WrongCredentials)
    ));
    vault
        .change_password(PASSWORD, NEW_PASSWORD, PARAMS)
        .unwrap();
    assert_ne!(vault.header().kdf.salt, old_salt, "fresh salt");
    vault.lock().unwrap();
    assert!(matches!(
        unlock(&paths, PASSWORD),
        Err(VaultError::WrongCredentials)
    ));
    unlock(&paths, NEW_PASSWORD).unwrap().lock().unwrap();
    // The recovery envelope is untouched.
    Vault::recover(&paths, &recovery, PASSWORD, PARAMS).unwrap();
}

#[test]
fn recover_sets_a_new_password() {
    let (_dir, paths) = setup();
    let (vault, recovery) = create(&paths);
    vault.store().put_meta("probe", b"kept").unwrap();
    vault.lock().unwrap();
    let wrong = RecoverySecret::from_bytes([0x42; 32]);
    assert!(matches!(
        Vault::recover(&paths, &wrong, NEW_PASSWORD, PARAMS),
        Err(VaultError::WrongCredentials)
    ));
    let vault = Vault::recover(&paths, &recovery, NEW_PASSWORD, PARAMS).unwrap();
    assert_eq!(
        vault.store().get_meta("probe").unwrap().as_deref(),
        Some(&b"kept"[..])
    );
    vault.lock().unwrap();
    assert!(matches!(
        unlock(&paths, PASSWORD),
        Err(VaultError::WrongCredentials)
    ));
    unlock(&paths, NEW_PASSWORD).unwrap();
}

#[test]
fn regenerate_recovery_revokes_the_old_secret() {
    let (_dir, paths) = setup();
    let (mut vault, old) = create(&paths);
    let new = vault.regenerate_recovery().unwrap();
    assert_ne!(old.expose_secret(), new.expose_secret());
    vault.lock().unwrap();
    assert!(matches!(
        Vault::recover(&paths, &old, NEW_PASSWORD, PARAMS),
        Err(VaultError::WrongCredentials)
    ));
    Vault::recover(&paths, &new, NEW_PASSWORD, PARAMS).unwrap();
}

#[test]
fn device_envelope_unlocks_only_its_vault() {
    let (_dir, paths) = setup();
    let (vault, _) = create(&paths);
    let envelope = vault.device_envelope(&[7; 32], &p256(1), &p256(2)).unwrap();
    let id = vault.vault_id();
    vault.lock().unwrap();
    assert_eq!(
        Vault::unlock_with_device(&paths, &envelope, &[7; 32])
            .unwrap()
            .vault_id(),
        id
    );
    assert!(matches!(
        Vault::unlock_with_device(&paths, &envelope, &[8; 32]),
        Err(VaultError::WrongCredentials)
    ));

    let (_other_dir, other) = setup();
    create(&other).0.lock().unwrap();
    assert!(matches!(
        Vault::unlock_with_device(&other, &envelope, &[7; 32]),
        Err(VaultError::WrongCredentials)
    ));
}

#[test]
fn every_header_field_mutation_is_detected() {
    let (_dir, paths) = setup();
    create(&paths).0.lock().unwrap();
    let original = fs::read(paths.header()).unwrap();
    let hex_fields: [&[&str]; 7] = [
        &["vault_id"],
        &["kdf", "salt"],
        &["password_envelope"],
        &["recovery_envelope"],
        &["ingest_public_key"],
        &["ingest_private_envelope"],
        &["format_version"],
    ];
    for field in hex_fields {
        fs::write(paths.header(), &original).unwrap();
        let value = payload_field(&paths, field);
        match value.as_str() {
            Some(hex) => tamper_payload(&paths, |p| flip_hex_char(p, hex)),
            None => tamper_payload(&paths, |p| {
                p.replace("\"format_version\":1", "\"format_version\":7")
            }),
        }
        let result = unlock(&paths, PASSWORD);
        assert!(result.is_err(), "{field:?} mutation accepted");
    }
    for (from, to) in [
        ("\"iterations\":2", "\"iterations\":3"),
        ("\"memory_kib\":19456", "\"memory_kib\":19457"),
    ] {
        fs::write(paths.header(), &original).unwrap();
        tamper_payload(&paths, |p| p.replace(from, to));
        assert!(unlock(&paths, PASSWORD).is_err(), "{from} -> {to} accepted");
    }

    // Fields the password envelope does not bind are caught by the MAC.
    fs::write(paths.header(), &original).unwrap();
    let key = payload_field(&paths, &["ingest_public_key"]);
    let replacement = hex::encode(
        atlys_crypto::hpke::IngestKeyPair::generate()
            .public
            .to_bytes(),
    );
    tamper_payload(&paths, |p| p.replace(key.as_str().unwrap(), &replacement));
    assert!(matches!(
        unlock(&paths, PASSWORD),
        Err(VaultError::IntegrityFailure)
    ));

    // A changed MAC is an integrity failure.
    fs::write(paths.header(), &original).unwrap();
    let file: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let mac = file["mac_hex"].as_str().unwrap();
    let text = String::from_utf8(original.clone()).unwrap();
    fs::write(paths.header(), flip_hex_char(&text, mac)).unwrap();
    assert!(matches!(
        unlock(&paths, PASSWORD),
        Err(VaultError::IntegrityFailure)
    ));

    fs::write(paths.header(), &original).unwrap();
    unlock(&paths, PASSWORD).unwrap();
}

#[test]
fn kdf_bounds_are_enforced_before_running_argon2() {
    let (_dir, paths) = setup();
    create(&paths).0.lock().unwrap();
    tamper_payload(&paths, |p| {
        p.replace("\"memory_kib\":19456", "\"memory_kib\":4194304")
    });
    let start = std::time::Instant::now();
    assert!(matches!(
        unlock(&paths, PASSWORD),
        Err(VaultError::InvalidHeader(_))
    ));
    assert!(start.elapsed() < std::time::Duration::from_millis(200));
}

#[test]
fn envelopes_do_not_move_between_vaults() {
    let (_a_dir, a) = setup();
    let (_b_dir, b) = setup();
    create(&a).0.lock().unwrap();
    create(&b).0.lock().unwrap();
    // Same password on both vaults. Copy A's password envelope and KDF
    // fields into B: the AAD binds B's vault ID, so it does not open.
    let a_env = payload_field(&a, &["password_envelope"]);
    let a_salt = payload_field(&a, &["kdf", "salt"]);
    let b_env = payload_field(&b, &["password_envelope"]);
    let b_salt = payload_field(&b, &["kdf", "salt"]);
    tamper_payload(&b, |p| {
        p.replace(b_env.as_str().unwrap(), a_env.as_str().unwrap())
            .replace(b_salt.as_str().unwrap(), a_salt.as_str().unwrap())
    });
    assert!(matches!(
        unlock(&b, PASSWORD),
        Err(VaultError::WrongCredentials)
    ));
}

#[test]
fn header_rollback_is_a_documented_limitation() {
    // RFC-0002: v1 has no monotonic counter, so restoring a complete old
    // header brings back the old password. This test pins that behavior so a
    // future fix is a deliberate format change.
    let (_dir, paths) = setup();
    let (mut vault, _) = create(&paths);
    let old_header = fs::read(paths.header()).unwrap();
    vault
        .change_password(PASSWORD, NEW_PASSWORD, PARAMS)
        .unwrap();
    vault.lock().unwrap();
    fs::write(paths.header(), &old_header).unwrap();
    unlock(&paths, PASSWORD).unwrap();
}

#[test]
fn inbox_writer_needs_no_secrets_and_unlocked_vault_reads_it() {
    let (_dir, paths) = setup();
    let (vault, _) = create(&paths);
    let id = vault.vault_id();
    vault.lock().unwrap();

    let writer = Vault::inbox_writer(&paths).unwrap();
    let batch = Batch::new(
        "device".into(),
        1_700_000_000_000,
        vec![RecordDraft {
            record_type: "device.battery".into(),
            source_id: "synthetic-1".into(),
            start_ms: 1_700_000_000_000,
            end_ms: None,
            data: serde_json::json!({ "level": 0.5 }),
        }],
    )
    .unwrap();
    let file = writer.write(&batch).unwrap();

    let vault = unlock(&paths, PASSWORD).unwrap();
    assert_eq!(inbox::pending(&paths.inbox()).unwrap(), vec![file.clone()]);
    assert_eq!(
        inbox::read(&id, vault.ingest_private_key(), &file).unwrap(),
        batch
    );
}

#[test]
fn unverified_header_exposes_only_public_metadata_before_unlock() {
    let (_dir, paths) = setup();
    let (vault, _) = create(&paths);
    let public = vault.ingest_public_key().clone();
    vault.lock().unwrap();
    let header: UnverifiedHeader = Vault::read_header(&paths).unwrap();
    assert_eq!(header.unverified().ingest_public_key, public);
}

#[test]
fn delete_removes_everything_and_is_retryable() {
    let (_dir, paths) = setup();
    let (vault, _) = create(&paths);
    Vault::inbox_writer(&paths).unwrap();
    fs::create_dir_all(paths.quarantine()).unwrap();
    fs::write(paths.quarantine().join("01A.unauthentic.rejected"), b"x").unwrap();
    fs::create_dir_all(paths.root().join(".vault-create-aa")).unwrap();
    vault.lock().unwrap();

    unlock(&paths, PASSWORD).unwrap().delete().unwrap();
    for gone in [
        paths.vault_dir(),
        paths.inbox(),
        paths.quarantine(),
        paths.root().join(".vault-create-aa"),
    ] {
        assert!(!gone.exists(), "{gone:?} survived deletion");
    }
    assert!(matches!(unlock(&paths, PASSWORD), Err(VaultError::NoVault)));
    delete_files(&paths).unwrap();
    // A new vault can be created afterwards.
    create(&paths);
}

#[test]
fn unsupported_header_version_asks_for_an_update() {
    let (_dir, paths) = setup();
    create(&paths).0.lock().unwrap();
    tamper_payload(&paths, |p| {
        p.replace("\"format_version\":1", "\"format_version\":2")
    });
    assert!(matches!(
        unlock(&paths, PASSWORD),
        Err(VaultError::UnsupportedVersion)
    ));
}

#[test]
fn recovery_phrase_round_trips_through_the_vault() {
    let (_dir, paths) = setup();
    let (vault, recovery) = create(&paths);
    let words = recovery.to_phrase();
    assert_eq!(words.len(), 24);
    vault.lock().unwrap();
    let typed = RecoverySecret::from_phrase(&words).unwrap();
    Vault::recover(&paths, &typed, NEW_PASSWORD, PARAMS).unwrap();
}

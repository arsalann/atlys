//! Public synthetic interoperability fixtures from RFC-0002.
use atlys_crypto::SecretKey;
use atlys_crypto::hkdf::{derive_from_shared_secret, derive_subkey, labels};
use atlys_crypto::{aead, mac};

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../../spec/vault/v1/vectors.json")).unwrap()
}

fn bytes(value: &serde_json::Value) -> Vec<u8> {
    hex::decode(value.as_str().unwrap()).unwrap()
}

#[test]
fn rfc0002_hkdf_vectors_match_independent_reference() {
    let fixture = fixture();
    let root = SecretKey::try_from_slice(&bytes(&fixture["root_hex"])).unwrap();
    let all = [
        labels::SQLCIPHER,
        labels::BLOB,
        labels::HEADER_MAC,
        labels::INGEST_WRAP,
        labels::RECOVERY,
    ];
    assert_eq!(fixture["subkeys"].as_object().unwrap().len(), all.len());
    for label in all {
        let name = std::str::from_utf8(label.as_bytes()).unwrap();
        let expected = bytes(&fixture["subkeys"][name]);
        assert_eq!(
            derive_subkey(&root, label).expose_secret().as_slice(),
            expected
        );
    }
    let enclave = &fixture["secure_enclave"];
    let key = derive_from_shared_secret(
        &bytes(&enclave["shared_secret_hex"]),
        &bytes(&enclave["salt_hex"]),
        labels::SECURE_ENCLAVE_ENVELOPE,
    )
    .unwrap();
    assert_eq!(key.expose_secret().as_slice(), bytes(&enclave["kek_hex"]));
}

#[test]
fn rfc0002_header_mac_vector_matches_independent_reference() {
    let fixture = fixture();
    let root = SecretKey::try_from_slice(&bytes(&fixture["root_hex"])).unwrap();
    let key = derive_subkey(&root, labels::HEADER_MAC);
    let vector = &fixture["header_mac"];
    let payload = vector["payload_utf8"].as_str().unwrap().as_bytes();
    let tag = bytes(&vector["tag_hex"]);
    let mut framed = mac::HEADER_CONTEXT.to_vec();
    framed.push(0);
    framed.extend_from_slice(payload);
    assert_eq!(framed, bytes(&vector["input_hex"]));
    assert_eq!(mac::header_tag(&key, payload).as_slice(), tag);
    mac::verify_header_tag(&key, payload, &tag).unwrap();
}

#[test]
fn rfc0002_envelopes_open_with_independent_xchacha20poly1305() {
    let fixture = fixture();
    let vault_key = bytes(&fixture["root_hex"]);
    let root = SecretKey::try_from_slice(&vault_key).unwrap();
    let envelopes = &fixture["envelopes"];
    let recovery = &envelopes["recovery"];
    let entropy = SecretKey::try_from_slice(&bytes(&recovery["entropy_hex"])).unwrap();
    // KEKs come from the key hierarchy, except the password KEK, which stands
    // in for an Argon2id output.
    let derived_keks = [
        ("password", None),
        ("recovery", Some(derive_subkey(&entropy, labels::RECOVERY))),
        ("ingest", Some(derive_subkey(&root, labels::INGEST_WRAP))),
    ];
    let aad_fields = [
        ("password", "password_aad_hex"),
        ("recovery", "recovery_aad_hex"),
        ("ingest", "ingest_wrap_aad_hex"),
    ];
    for ((name, derived), (_, aad_field)) in derived_keks.into_iter().zip(aad_fields) {
        let vector = &envelopes[name];
        let kek = SecretKey::try_from_slice(&bytes(&vector["kek_hex"])).unwrap();
        if let Some(derived) = derived {
            assert!(derived.ct_eq(&kek), "{name} KEK derivation");
        }
        let aad = bytes(&vector["aad_hex"]);
        assert_eq!(aad, bytes(&fixture[aad_field]), "{name} AAD");
        let envelope = bytes(&vector["envelope_hex"]);
        assert_eq!(envelope.len(), aead::OVERHEAD + atlys_crypto::KEY_LEN);
        assert_eq!(envelope[..aead::NONCE_LEN], bytes(&vector["nonce_hex"])[..]);
        let opened = aead::unwrap_key(&kek, &envelope, &aad).unwrap();
        assert_eq!(
            opened.expose_secret().as_slice(),
            bytes(&vector["plaintext_hex"]),
            "{name} plaintext"
        );
        for i in 0..envelope.len() {
            let mut tampered = envelope.clone();
            tampered[i] ^= 1;
            assert!(
                aead::unwrap_key(&kek, &tampered, &aad).is_err(),
                "{name} byte {i}"
            );
        }
        let mut other_aad = aad.clone();
        *other_aad.last_mut().unwrap() ^= 1;
        assert!(aead::unwrap_key(&kek, &envelope, &other_aad).is_err());
    }
    assert_eq!(bytes(&envelopes["password"]["plaintext_hex"]), vault_key);
}

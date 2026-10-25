//! Public synthetic interoperability fixtures from RFC-0002.
use atlys_crypto::SecretKey;
use atlys_crypto::hkdf::{derive_from_shared_secret, derive_subkey, labels};

#[test]
fn rfc0002_hkdf_vectors_match_independent_reference() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../spec/vault/v1/vectors.json")).unwrap();
    let bytes = |value: &serde_json::Value| hex::decode(value.as_str().unwrap()).unwrap();
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

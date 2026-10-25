//! Public synthetic interoperability fixtures from RFC-0002.
use atlys_crypto::SecretKey;
use atlys_crypto::hkdf::{derive_from_shared_secret, derive_subkey, labels};
use atlys_crypto::mac;

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

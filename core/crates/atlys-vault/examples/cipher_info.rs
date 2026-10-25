//! Prints the linked SQLCipher build and crypto provider.
//!
//!   cargo run -p atlys-vault --example cipher_info

use atlys_crypto::SecretKey;
use atlys_vault::store::Store;

fn main() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = SecretKey::generate().expect("rng");
    let store = Store::open(&dir.path().join("probe.db"), &key).expect("open");
    println!("{:#?}", store.cipher_info().expect("cipher info"));
}

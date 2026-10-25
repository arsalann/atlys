//! Thin wrapper so the bindgen version always matches the `uniffi` runtime
//! version pinned in `Cargo.lock`.

fn main() {
    uniffi::uniffi_bindgen_main()
}

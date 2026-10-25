//! UniFFI surface of the Atlys core.
//!
//! Everything exported here becomes part of the Swift `AtlysCore` module. Keep
//! the surface small: Swift owns platform concerns, Rust owns data and keys.

uniffi::setup_scaffolding!();

/// Static information about the linked core library.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CoreInfo {
    /// Version of the Rust core (the workspace version).
    pub version: String,
    /// Version of the Atlys Data Model the core understands.
    pub adm_version: String,
}

/// Returns version information about the core.
#[uniffi::export]
pub fn core_info() -> CoreInfo {
    CoreInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        adm_version: atlys_adm::ADM_VERSION.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_info_reports_versions() {
        let info = core_info();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.adm_version, atlys_adm::ADM_VERSION);
    }
}

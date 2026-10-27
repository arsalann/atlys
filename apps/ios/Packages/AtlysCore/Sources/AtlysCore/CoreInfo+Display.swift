// Hand-written additions to the generated AtlysCore bindings.

extension CoreInfo {
    /// Short version string for About screens, for example "core 0.0.1 · ADM 0.1".
    public var displayVersion: String {
        "core \(version) · ADM \(admVersion)"
    }
}

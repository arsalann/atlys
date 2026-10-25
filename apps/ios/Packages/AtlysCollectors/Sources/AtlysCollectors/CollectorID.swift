/// The v0 collectors. See plan/v0/02-data-sources.md.
public enum CollectorID: String, CaseIterable, Sendable {
    case health
    case location
    case motion
    case device
}

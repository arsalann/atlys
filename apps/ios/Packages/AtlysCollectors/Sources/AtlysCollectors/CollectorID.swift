/// The v0 collectors. See plan/v0/02-data-sources.md.
public enum CollectorID: String, CaseIterable, Sendable, Codable {
    case health
    case location
    case motion
    case device

    public var displayName: String {
        switch self {
        case .health: "Apple Health"
        case .location: "Location"
        case .motion: "Motion & Fitness"
        case .device: "Device"
        }
    }

    /// SF Symbol for lists.
    public var symbolName: String {
        switch self {
        case .health: "heart.fill"
        case .location: "location.fill"
        case .motion: "figure.walk"
        case .device: "iphone"
        }
    }
}

import Foundation
import UIKit

/// One background or foreground wake. Holds no record contents, coordinates,
/// or health values: only when it happened, why, and what was written.
struct WakeEvent: Codable, Sendable, Identifiable {
    var id: Date { date }
    let date: Date
    let trigger: String
    let appState: String
    let protectedDataAvailable: Bool
    let outcome: String
}

/// Append-only JSON Lines log of wakes, readable in the app and exportable for
/// the spike report (M0 task 0.8).
actor WakeLog {
    static let shared = WakeLog()

    func append(_ event: WakeEvent) {
        guard let url = try? SpikePaths.wakeLog,
              var line = try? JSONEncoder.spike.encode(event) else { return }
        line.append(0x0A)
        if let handle = try? FileHandle(forWritingTo: url) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: line)
        } else {
            try? line.write(to: url, options: [.completeFileProtectionUntilFirstUserAuthentication])
        }
    }

    func entries() -> [WakeEvent] {
        guard let url = try? SpikePaths.wakeLog, let data = try? Data(contentsOf: url) else { return [] }
        return data.split(separator: 0x0A).compactMap { try? JSONDecoder.spike.decode(WakeEvent.self, from: $0) }
    }

    func clear() {
        if let url = try? SpikePaths.wakeLog { try? FileManager.default.removeItem(at: url) }
    }
}

extension WakeLog {
    /// Records a wake, reading app state on the main actor.
    @MainActor
    static func record(_ trigger: String, outcome: String) {
        let app = UIApplication.shared
        let state = switch app.applicationState {
        case .active: "active"
        case .inactive: "inactive"
        case .background: "background"
        @unknown default: "unknown"
        }
        let event = WakeEvent(date: .now, trigger: trigger, appState: state,
                              protectedDataAvailable: app.isProtectedDataAvailable, outcome: outcome)
        Task { await shared.append(event) }
    }
}

extension JSONEncoder {
    static let spike: JSONEncoder = {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .iso8601
        return e
    }()
}

extension JSONDecoder {
    static let spike: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()
}

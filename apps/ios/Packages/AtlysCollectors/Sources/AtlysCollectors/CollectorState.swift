import Foundation

/// What a collector remembers between runs: query anchors, cursors, and
/// status. Stored as one small file per collector next to the inbox, with
/// *Complete Until First User Authentication* protection so background runs
/// can read it. It reveals when collection happened, never what was collected.
public struct CollectorState: Codable, Sendable, Equatable {
    /// Last run that reached the collector, Unix epoch milliseconds.
    public var lastRunMs: Int64?
    /// Last run that finished without an error.
    public var lastSuccessMs: Int64?
    /// Short, content-free description of the last failure.
    public var lastError: String?
    /// Records sealed into the inbox since the vault was created.
    public var recordsSealed: Int64 = 0
    /// Collector-specific values (anchors, cursors, flags).
    public var values: [String: Data] = [:]

    public init() {}

    public func int(_ key: String) -> Int64? {
        values[key].flatMap { Int64(String(decoding: $0, as: UTF8.self)) }
    }

    public mutating func setInt(_ key: String, _ value: Int64?) {
        values[key] = value.map { Data(String($0).utf8) }
    }
}

/// Loads and saves ``CollectorState`` files under a directory.
public struct CollectorStateStore: Sendable {
    public let directory: URL

    public init(directory: URL) {
        self.directory = directory
    }

    /// `<vault root>/collectors`.
    public init(vaultRoot: URL) {
        self.directory = vaultRoot.appending(path: "collectors", directoryHint: .isDirectory)
    }

    private func url(_ id: CollectorID) -> URL {
        directory.appending(path: "\(id.rawValue).json")
    }

    public func load(_ id: CollectorID) -> CollectorState {
        guard let data = try? Data(contentsOf: url(id)),
              let state = try? JSONDecoder().decode(CollectorState.self, from: data)
        else { return CollectorState() }
        return state
    }

    public func save(_ state: CollectorState, for id: CollectorID) throws {
        try FileManager.default.createDirectory(
            at: directory, withIntermediateDirectories: true,
            attributes: [.protectionKey: InboxLocation.protection]
        )
        let data = try JSONEncoder().encode(state)
        try data.write(to: url(id), options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }

    /// Applies `change` to the stored state and saves it.
    @discardableResult
    public func update(_ id: CollectorID, _ change: (inout CollectorState) -> Void) throws -> CollectorState {
        var state = load(id)
        change(&state)
        try save(state, for: id)
        return state
    }

    /// Removes every state file (when the vault is deleted).
    public func removeAll() {
        try? FileManager.default.removeItem(at: directory)
    }
}

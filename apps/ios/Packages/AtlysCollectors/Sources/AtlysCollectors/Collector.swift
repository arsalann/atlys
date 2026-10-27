import AtlysCore
import Foundation

/// What started a collection run.
public enum CollectionTrigger: String, Sendable {
    /// The app came to the foreground.
    case foreground
    /// The user tapped "Collect now".
    case manual
    /// `BGAppRefreshTask`.
    case backgroundRefresh
    /// `BGProcessingTask` (long backfills while charging).
    case backgroundProcessing
    /// A HealthKit background delivery.
    case healthDelivery
    /// A location event relaunched or woke the app.
    case locationEvent
}

/// Whether a collector can run.
public enum CollectorPermission: Sendable, Equatable {
    /// No permission is needed.
    case notRequired
    /// The user has not been asked yet.
    case notDetermined
    /// Granted.
    case granted
    /// Granted only while the app is in use (location).
    case limited
    /// Denied or restricted in iOS Settings.
    case denied
    /// The framework is not available on this device.
    case unavailable
}

/// The result of one run of one collector.
public struct CollectionOutcome: Sendable, Equatable {
    public var recordsSealed = 0
    public var deletionsSealed = 0
    /// More data is waiting (the time budget ran out mid-backfill).
    public var moreAvailable = false
    /// Data could not be read now (for example HealthKit while the phone is
    /// locked) and will be read on a later run.
    public var deferred = false

    public init() {}

    public static func + (a: Self, b: Self) -> Self {
        var out = a
        out.recordsSealed += b.recordsSealed
        out.deletionsSealed += b.deletionsSealed
        out.moreAvailable = a.moreAvailable || b.moreAvailable
        out.deferred = a.deferred || b.deferred
        return out
    }
}

/// Everything a collector needs for one run.
public struct CollectionContext: Sendable {
    public let trigger: CollectionTrigger
    /// When the run must stop. Collectors check it between pages.
    public let deadline: Date
    let sealer: any RecordSealing
    let states: CollectorStateStore

    public init(trigger: CollectionTrigger, deadline: Date, sealer: any RecordSealing, states: CollectorStateStore) {
        self.trigger = trigger
        self.deadline = deadline
        self.sealer = sealer
        self.states = states
    }

    public var timeIsUp: Bool { Date() >= deadline }

    /// Seals records and deletions in batches of bounded size.
    public func seal(_ collector: CollectorID, records: [RecordDraft], deletions: [RecordDeletion] = []) throws -> CollectionOutcome {
        var outcome = CollectionOutcome()
        guard !records.isEmpty || !deletions.isEmpty else { return outcome }
        let chunks = stride(from: 0, to: max(records.count, 1), by: Self.batchSize).map {
            Array(records[$0..<min($0 + Self.batchSize, records.count)])
        }
        for (i, chunk) in chunks.enumerated() {
            let dels = i == chunks.count - 1 ? deletions : []
            guard !chunk.isEmpty || !dels.isEmpty else { continue }
            try sealer.seal(collector: collector, records: chunk, deletions: dels)
            outcome.recordsSealed += chunk.count
            outcome.deletionsSealed += dels.count
        }
        return outcome
    }

    static let batchSize = 2000

    public func state(_ id: CollectorID) -> CollectorState { states.load(id) }

    public func saveState(_ state: CollectorState, for id: CollectorID) throws {
        try states.save(state, for: id)
    }
}

/// Seals batches into the inbox. ``IngestSealer`` in the app; a recorder in tests.
public protocol RecordSealing: Sendable {
    func seal(collector: CollectorID, records: [RecordDraft], deletions: [RecordDeletion]) throws
}

extension IngestSealer: RecordSealing {
    public func seal(collector: CollectorID, records: [RecordDraft], deletions: [RecordDeletion]) throws {
        _ = try sealWithDeletions(collector: collector.rawValue, records: records, deletions: deletions)
    }
}

/// One source of on-device data.
public protocol Collector: Sendable {
    var id: CollectorID { get }
    /// Current permission state. Cheap; never prompts.
    func permission() async -> CollectorPermission
    /// Asks the user for access. May show system prompts.
    func requestPermission() async
    /// Reads new data since the last run and seals it, until `context.deadline`.
    func collect(_ context: CollectionContext) async throws -> CollectionOutcome
}

/// Builds a record draft from typed fields.
public func makeRecord(
    type: String,
    sourceID: String,
    start: Date,
    end: Date? = nil,
    data: JSONValue,
    timeZone: String? = nil,
    sourceName: String? = nil,
    sourceBundleID: String? = nil,
    sourceDevice: String? = nil
) -> RecordDraft {
    RecordDraft(
        recordType: type, sourceId: sourceID, startMs: start.millis, endMs: end.map(\.millis),
        dataJson: data.encoded(), tz: timeZone, sourceName: sourceName.flatMap(nonEmpty),
        sourceBundleId: sourceBundleID.flatMap(nonEmpty), sourceDevice: sourceDevice.flatMap(nonEmpty)
    )
}

private func nonEmpty(_ s: String) -> String? {
    let trimmed = String(s.prefix(256))
    return trimmed.isEmpty ? nil : trimmed
}

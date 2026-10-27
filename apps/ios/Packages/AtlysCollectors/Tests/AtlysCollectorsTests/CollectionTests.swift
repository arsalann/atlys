@testable import AtlysCollectors
import AtlysCore
import CoreLocation
import Foundation
import Testing

/// Records what a collector run sealed instead of writing inbox files.
final class RecordingSealer: RecordSealing, @unchecked Sendable {
    private let lock = NSLock()
    private var stored: [(CollectorID, [RecordDraft], [RecordDeletion])] = []

    var batches: [(CollectorID, [RecordDraft], [RecordDeletion])] { lock.withLock { stored } }

    func seal(collector: CollectorID, records: [RecordDraft], deletions: [RecordDeletion]) throws {
        lock.withLock { stored.append((collector, records, deletions)) }
    }
}

struct FakeCollector: Collector {
    let id: CollectorID
    var permissionState: CollectorPermission = .granted
    var records = 1
    var fails = false

    func permission() async -> CollectorPermission { permissionState }
    func requestPermission() async {}

    func collect(_ context: CollectionContext) async throws -> CollectionOutcome {
        if fails { throw CocoaError(.fileReadNoPermission) }
        let drafts = (0..<records).map { i in
            makeRecord(type: "device.battery", sourceID: "\(id.rawValue)-\(context.trigger.rawValue)-\(i)",
                       start: Date(millis: 1_700_000_000_000),
                       data: .object(["state": .string("full"), "low_power_mode": .bool(false)]))
        }
        return try context.seal(id, records: drafts)
    }
}

@Suite("Collection")
struct CollectionTests {
    let states = CollectorStateStore(directory: FileManager.default.temporaryDirectory
        .appending(path: "atlys-states-\(UUID().uuidString)", directoryHint: .isDirectory))

    @Test("Fake collectors run under every trigger and record their state",
          arguments: [CollectionTrigger.foreground, .manual, .backgroundRefresh, .backgroundProcessing,
                      .healthDelivery, .locationEvent])
    func everyTrigger(trigger: CollectionTrigger) async throws {
        let sealer = RecordingSealer()
        let coordinator = CollectionCoordinator(
            collectors: [FakeCollector(id: .device), FakeCollector(id: .motion, records: 3)],
            states: states, makeSealer: { sealer }
        )
        let outcomes = await coordinator.run(trigger, budget: 10)
        #expect(outcomes[.device]?.recordsSealed == 1)
        #expect(outcomes[.motion]?.recordsSealed == 3)
        #expect(sealer.batches.map(\.0) == [.device, .motion])
        let motion = states.load(.motion)
        #expect(motion.recordsSealed == 3)
        #expect(motion.lastSuccessMs != nil && motion.lastError == nil)
    }

    @Test("Collectors without permission are skipped; failures are recorded without content")
    func skipsAndFailures() async {
        let sealer = RecordingSealer()
        let coordinator = CollectionCoordinator(
            collectors: [
                FakeCollector(id: .health, permissionState: .notDetermined),
                FakeCollector(id: .location, permissionState: .denied),
                FakeCollector(id: .device, fails: true),
                FakeCollector(id: .motion),
            ],
            states: states, makeSealer: { sealer }
        )
        let outcomes = await coordinator.run(.foreground, budget: 10)
        #expect(Set(outcomes.keys) == [.motion])
        #expect(states.load(.device).lastError == "NSCocoaErrorDomain 257")
        #expect(states.load(.health).lastRunMs == nil)
        let statuses = await coordinator.statuses()
        #expect(statuses.map(\.permission) == [.notDetermined, .denied, .granted, .granted])
    }

    @Test("Runs never overlap")
    func serialized() async {
        let sealer = RecordingSealer()
        let coordinator = CollectionCoordinator(
            collectors: [FakeCollector(id: .device)], states: states, makeSealer: { sealer }
        )
        async let a = coordinator.run(.foreground, budget: 10)
        async let b = coordinator.run(.manual, budget: 10)
        _ = await (a, b)
        #expect(sealer.batches.count == 2)
        #expect(states.load(.device).recordsSealed == 2)
    }

    @Test("Without a vault there is no sealer and nothing runs")
    func noVault() async {
        let coordinator = CollectionCoordinator(
            collectors: [FakeCollector(id: .device)], states: states,
            makeSealer: { throw AtlysError.NoVault }
        )
        #expect(await coordinator.run(.foreground, budget: 10).isEmpty)
    }

    @Test("Large outputs are sealed in bounded batches; deletions go in the last one")
    func chunking() throws {
        let sealer = RecordingSealer()
        let context = CollectionContext(trigger: .manual, deadline: .distantFuture, sealer: sealer, states: states)
        let records = (0..<4500).map { i in
            makeRecord(type: "device.battery", sourceID: "b-\(i)", start: Date(),
                       data: .object(["state": .string("full"), "low_power_mode": .bool(true)]))
        }
        let deletion = RecordDeletion(recordType: "health.quantity", sourceId: "gone")
        let outcome = try context.seal(.health, records: records, deletions: [deletion])
        #expect(sealer.batches.map { $0.1.count } == [2000, 2000, 500])
        #expect(sealer.batches.map { $0.2.count } == [0, 0, 1])
        #expect(outcome.recordsSealed == 4500 && outcome.deletionsSealed == 1)
        _ = try context.seal(.health, records: [], deletions: [deletion])
        #expect(sealer.batches.last.map { $0.1.isEmpty && $0.2.count == 1 } == true)
    }

    @Test("An invalid record is skipped without blocking the rest of its batch")
    func invalidRecordSkipped() throws {
        let sealer = RejectingSealer()
        let context = CollectionContext(trigger: .manual, deadline: .distantFuture, sealer: sealer, states: states)
        let good = makeRecord(type: "device.battery", sourceID: "ok", start: Date(),
                              data: .object(["state": .string("full"), "low_power_mode": .bool(true)]))
        let bad = makeRecord(type: "device.battery", sourceID: "bad", start: Date(), data: .object([:]))
        let outcome = try context.seal(.device, records: [good, bad, good])
        #expect(outcome.recordsSealed == 2 && outcome.recordsSkipped == 1)
        #expect(sealer.sealed.map(\.sourceId) == ["ok", "ok"])
    }

    @Test("State round-trips through its file")
    func state() throws {
        var state = CollectorState()
        state.setInt("cursor", 42)
        state.values["anchor"] = Data([1, 2, 3])
        try states.save(state, for: .health)
        let loaded = states.load(.health)
        #expect(loaded == state)
        #expect(loaded.int("cursor") == 42)
        #expect(states.load(.motion) == CollectorState())
    }
}

@Suite("Records")
struct RecordBuilderTests {
    @Test("JSON leaves out nil and non-finite numbers and sorts keys")
    func json() {
        let value = JSONValue.object([
            "b": .number(.nan), "a": .int(1), "c": nil, "d": .array([.bool(true), .string("x/y")]),
        ])
        #expect(value.encoded() == #"{"a":1,"d":[true,"x/y"]}"#)
        #expect(JSONValue.from(any: Date(millis: 5)) == .int(5))
        #expect(JSONValue.from(any: ["k": 1.5]) == .object(["k": .double(1.5)]))
    }

    @Test("Visits are keyed by arrival so the departure update replaces them")
    func visits() throws {
        let point = LocationService.pointRecord(
            CLLocation(coordinate: .init(latitude: 1.5, longitude: -2), altitude: 10, horizontalAccuracy: 5,
                       verticalAccuracy: -1, timestamp: Date(millis: 1_700_000_000_000)),
            trigger: "foreground"
        )
        #expect(point.recordType == "location.point")
        #expect(point.sourceId == "point-1700000000000")
        #expect(!point.dataJson.contains("altitude_m"), "invalid vertical accuracy drops altitude")
        #expect(point.dataJson.contains(#""trigger":"foreground""#))
    }

    @Test("Pedometer buckets start on the hour")
    func hours() {
        let date = Date(millis: 1_700_000_123_456)
        #expect(MotionCollector.hourStart(date).millis % 3_600_000 == 0)
        #expect(MotionCollector.hourStart(date) <= date)
    }
}

/// Rejects any batch that contains a record with empty data, like the core would.
final class RejectingSealer: RecordSealing, @unchecked Sendable {
    private let lock = NSLock()
    private var stored: [RecordDraft] = []
    var sealed: [RecordDraft] { lock.withLock { stored } }

    func seal(collector: CollectorID, records: [RecordDraft], deletions: [RecordDeletion]) throws {
        if records.contains(where: { $0.dataJson == "{}" }) {
            throw AtlysError.InvalidInput(message: "records[0].data.state: missing required field")
        }
        lock.withLock { stored.append(contentsOf: records) }
    }
}

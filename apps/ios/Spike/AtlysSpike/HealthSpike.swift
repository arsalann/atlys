import AtlysCore
import Foundation
import HealthKit

/// HealthKit observer + background delivery for step count (M0 task 0.8).
///
/// On each wake it runs an anchored query from the saved anchor and seals new
/// samples. While the phone is locked HealthKit refuses reads
/// (`errorDatabaseInaccessible`); the wake is logged as deferred and the next
/// wake catches up from the same anchor.
final class HealthSpike: Sendable {
    static let shared = HealthSpike()

    let store = HKHealthStore()
    private let stepType = HKQuantityType(.stepCount)

    var isAvailable: Bool { HKHealthStore.isHealthDataAvailable() }

    func requestAuthorization() async throws {
        // Share access is only requested in the simulator, to seed synthetic samples.
        #if targetEnvironment(simulator)
        let share: Set<HKSampleType> = [stepType]
        #else
        let share: Set<HKSampleType> = []
        #endif
        try await store.requestAuthorization(toShare: share, read: [stepType])
        registerObserver()
    }

    /// Must run early in every launch so background deliveries are handled.
    func registerObserver() {
        guard isAvailable else { return }
        let query = HKObserverQuery(sampleType: stepType, predicate: nil) { _, completion, error in
            let done = UncheckedSendable(completion)
            if let error {
                Task { @MainActor in WakeLog.record("health-observer", outcome: "error: \(error)") }
                done.value()
                return
            }
            Task {
                await HealthSpike.shared.collect(trigger: "health-observer")
                done.value()
            }
        }
        store.execute(query)
        Task {
            do {
                try await store.enableBackgroundDelivery(for: stepType, frequency: .immediate)
            } catch {
                await MainActor.run { WakeLog.record("health-bg-delivery", outcome: "enable failed: \(error)") }
            }
        }
    }

    /// Reads new step samples since the saved anchor and seals them.
    func collect(trigger: String) async {
        let anchor = loadAnchor()
        let descriptor = HKAnchoredObjectQueryDescriptor(
            predicates: [.quantitySample(type: stepType)], anchor: anchor, limit: 500
        )
        do {
            let result = try await descriptor.result(for: store)
            let records = result.addedSamples.map(Self.record(for:))
            var outcome = "no new samples"
            if !records.isEmpty {
                let name = try SpikeVaultStore.sealer().seal(collector: "health", records: records)
                outcome = "sealed \(records.count) record(s) -> \(name)"
            }
            if !result.deletedObjects.isEmpty { outcome += ", \(result.deletedObjects.count) deletion(s)" }
            saveAnchor(result.newAnchor)
            await MainActor.run { WakeLog.record(trigger, outcome: outcome) }
        } catch let error as HKError where error.code == .errorDatabaseInaccessible {
            await MainActor.run { WakeLog.record(trigger, outcome: "deferred: health data locked") }
        } catch {
            await MainActor.run { WakeLog.record(trigger, outcome: "failed: \(error)") }
        }
    }

    #if targetEnvironment(simulator)
    /// Simulator only: writes one synthetic step sample to trigger the observer.
    func addSyntheticSteps() async throws {
        let end = Date.now
        let sample = HKQuantitySample(
            type: stepType, quantity: HKQuantity(unit: .count(), doubleValue: Double(Int.random(in: 50...500))),
            start: end.addingTimeInterval(-600), end: end, metadata: ["AtlysSyntheticTestData": true]
        )
        try await store.save(sample)
    }
    #endif

    private static func record(for sample: HKQuantitySample) -> RecordDraft {
        RecordDraft(
            recordType: "health.steps",
            sourceId: sample.uuid.uuidString,
            startMs: sample.startDate.millis,
            endMs: sample.endDate.millis,
            dataJson: json([
                "count": sample.quantity.doubleValue(for: .count()),
                "source_bundle": sample.sourceRevision.source.bundleIdentifier,
            ])
        )
    }

    private func loadAnchor() -> HKQueryAnchor? {
        guard let url = try? SpikePaths.healthAnchor, let data = try? Data(contentsOf: url) else { return nil }
        return try? NSKeyedUnarchiver.unarchivedObject(ofClass: HKQueryAnchor.self, from: data)
    }

    private func saveAnchor(_ anchor: HKQueryAnchor) {
        guard let url = try? SpikePaths.healthAnchor,
              let data = try? NSKeyedArchiver.archivedData(withRootObject: anchor, requiringSecureCoding: true)
        else { return }
        try? data.write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }
}

/// Carries a non-Sendable value (HealthKit's completion handler) across an
/// isolation boundary. HealthKit documents the handler as callable from any thread.
struct UncheckedSendable<Value>: @unchecked Sendable {
    let value: Value
    init(_ value: Value) { self.value = value }
}

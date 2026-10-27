import AtlysCore
import CoreLocation
import Foundation
import HealthKit
import UIKit

/// Reads every HealthKit type the app can read (RFC-0001 `health.*`
/// families) with anchored queries, including deletions, and seals the
/// results. The first runs page through all history; later runs read only
/// what changed. HealthKit refuses reads while the phone is locked, so such
/// runs are deferred to the next wake.
public final class HealthCollector: Collector, @unchecked Sendable {
    public let id = CollectorID.health
    let store = HKHealthStore()
    /// Clinical records need the Health Records entitlement. The app sets
    /// `AtlysHealthRecords` in its Info.plist when it has it.
    let clinical: Bool
    static let pageSize = 2000

    public init(clinical: Bool = Bundle.main.object(forInfoDictionaryKey: "AtlysHealthRecords") as? Bool ?? false) {
        self.clinical = clinical
    }

    public var isAvailable: Bool { HKHealthStore.isHealthDataAvailable() }

    public func permission() async -> CollectorPermission {
        guard isAvailable else { return .unavailable }
        // Apps can't see whether read access was granted; "asked" is the
        // most HealthKit reveals.
        let status = try? await store.statusForAuthorizationRequest(
            toShare: [], read: HealthTypes.readTypes(clinical: false)
        )
        return status == .unnecessary ? .granted : .notDetermined
    }

    public func requestPermission() async {
        guard isAvailable else { return }
        try? await store.requestAuthorization(toShare: [], read: HealthTypes.readTypes(clinical: false))
        if clinical {
            try? await store.requestAuthorization(
                toShare: [], read: Set(HealthTypes.clinicalTypes as [HKObjectType])
            )
        }
        await enableBackgroundDelivery()
    }

    // MARK: Background delivery

    /// Registers one observer query for every sample type. Call during every
    /// launch, before it finishes, so background deliveries reach the app.
    public func startObserving(onUpdate: @escaping @Sendable () async -> Void) {
        guard isAvailable else { return }
        let descriptors = HealthTypes.sampleTypes(clinical: false).map { HKQueryDescriptor(sampleType: $0, predicate: nil) }
        let query = HKObserverQuery(queryDescriptors: descriptors) { _, _, completion, error in
            let done = CompletionBox(completion)
            guard error == nil else {
                done.call()
                return
            }
            Task {
                await onUpdate()
                done.call()
            }
        }
        store.execute(query)
        Task { await enableBackgroundDelivery() }
    }

    private func enableBackgroundDelivery() async {
        for type in HealthTypes.sampleTypes(clinical: false) {
            try? await store.enableBackgroundDelivery(for: type, frequency: .immediate)
        }
    }

    // MARK: Collection

    public func collect(_ context: CollectionContext) async throws -> CollectionOutcome {
        guard isAvailable else { return CollectionOutcome() }
        guard await UIApplication.shared.isProtectedDataAvailable else {
            var deferred = CollectionOutcome()
            deferred.deferred = true
            return deferred
        }
        var state = context.state(id)
        var outcome = CollectionOutcome()
        var units = UnitCache(store: store)

        if state.int("characteristicsMs").map({ Date().timeIntervalSince(Date(millis: $0)) > 86_400 }) ?? true {
            outcome = outcome + (try context.seal(id, records: characteristics()))
            state.setInt("characteristicsMs", Date().millis)
            try context.saveState(state, for: id)
        }

        let types = HealthTypes.sampleTypes(clinical: clinical)
        var complete = 0
        for type in types {
            if context.timeIsUp {
                outcome.moreAvailable = true
                break
            }
            let key = "anchor.\(type.identifier)"
            do {
                var done = false
                while !done, !context.timeIsUp {
                    let anchor = state.values[key].flatMap(Self.decodeAnchor)
                    let descriptor = HKAnchoredObjectQueryDescriptor(
                        predicates: [.sample(type: type)], anchor: anchor, limit: Self.pageSize
                    )
                    let result = try await descriptor.result(for: store)
                    var records: [RecordDraft] = []
                    for sample in result.addedSamples {
                        records.append(contentsOf: await self.records(for: sample, units: &units))
                    }
                    let deletions = result.deletedObjects.map {
                        RecordDeletion(recordType: Self.recordType(for: type), sourceId: $0.uuid.uuidString)
                    }
                    outcome = outcome + (try context.seal(id, records: records, deletions: deletions))
                    state.values[key] = Self.encodeAnchor(result.newAnchor)
                    try context.saveState(state, for: id)
                    done = result.addedSamples.count + result.deletedObjects.count < Self.pageSize
                }
                if done {
                    complete += 1
                } else {
                    outcome.moreAvailable = true
                }
            } catch let error as HKError where error.code == .errorDatabaseInaccessible {
                outcome.deferred = true
                break
            } catch let error as HKError where error.code == .errorAuthorizationDenied
                || error.code == .errorAuthorizationNotDetermined {
                complete += 1
            } catch let error as HKError where error.code == .errorInvalidArgument {
                // A type this device can't query; nothing to read.
                complete += 1
            }
        }
        state.setInt("typesTotal", Int64(types.count))
        if complete == types.count { state.setInt("backfillCompleteMs", state.int("backfillCompleteMs") ?? Date().millis) }
        state.setInt("typesComplete", Int64(complete))
        try context.saveState(state, for: id)
        return outcome
    }

    static func recordType(for type: HKSampleType) -> String {
        switch type {
        case is HKQuantityType: "health.quantity"
        case is HKCategoryType: "health.category"
        case is HKWorkoutType: "health.workout"
        case is HKElectrocardiogramType: "health.electrocardiogram"
        case is HKAudiogramSampleType: "health.audiogram"
        case is HKClinicalType: "health.clinical_record"
        case is HKStateOfMindType: "health.state_of_mind"
        case HKSeriesType.heartbeat(): "health.heartbeat_series"
        case HKSeriesType.workoutRoute(): "health.workout_route"
        default: "health.sample"
        }
    }

    // MARK: Samples to records

    private func records(for sample: HKSample, units: inout UnitCache) async -> [RecordDraft] {
        let common = Common(sample)
        switch sample {
        case let s as HKQuantitySample:
            guard let unit = await units.unit(for: s.quantityType) else { return [] }
            return [common.record("health.quantity", [
                "identifier": .string(s.quantityType.identifier),
                "value": .number(s.quantity.doubleValue(for: unit)),
                "unit": .string(unit.unitString),
                "metadata": common.metadata,
            ])]
        case let s as HKCategorySample:
            let identifier = s.categoryType.identifier
            return [common.record("health.category", [
                "identifier": .string(identifier),
                "value": .int(s.value),
                "value_name": HealthTypes.valueName(identifier: identifier, value: s.value).map { .string($0) },
                "metadata": common.metadata,
            ])]
        case let w as HKWorkout:
            var out = [common.record("health.workout", await workoutData(w, units: &units))]
            out.append(contentsOf: await routes(for: w))
            return out
        case let ecg as HKElectrocardiogram:
            return [common.record("health.electrocardiogram", [
                "classification": .int(ecg.classification.rawValue),
                "symptoms_status": .int(ecg.symptomsStatus.rawValue),
                "average_heart_rate_bpm": ecg.averageHeartRate.flatMap {
                    .number($0.doubleValue(for: .count().unitDivided(by: .minute())))
                },
                "sampling_frequency_hz": ecg.samplingFrequency.flatMap { .number($0.doubleValue(for: .hertz())) },
                "voltages": .array(await voltages(ecg)),
                "metadata": common.metadata,
            ])]
        case let series as HKHeartbeatSeriesSample:
            return [common.record("health.heartbeat_series", [
                "beats": .array(await beats(series)),
                "metadata": common.metadata,
            ])]
        case let audiogram as HKAudiogramSample:
            return [common.record("health.audiogram", [
                "sensitivity_points": .array(audiogram.sensitivityPoints.map(Self.sensitivityPoint)),
                "metadata": common.metadata,
            ])]
        case let mind as HKStateOfMind:
            return [common.record("health.state_of_mind", [
                "kind": .int(mind.kind.rawValue),
                "valence": .number(min(1, max(-1, mind.valence))),
                "labels": .array(mind.labels.map { .int($0.rawValue) }),
                "associations": .array(mind.associations.map { .int($0.rawValue) }),
                "metadata": common.metadata,
            ])]
        case let record as HKClinicalRecord:
            let resource = record.fhirResource
            return [common.record("health.clinical_record", [
                "identifier": .string(record.clinicalType.identifier),
                "display_name": .string(String(record.displayName.prefix(256)).isEmpty ? "Record" : String(record.displayName.prefix(256))),
                "fhir_version": resource.map { .string($0.fhirVersion.stringRepresentation) },
                "fhir_resource_type": resource.map { .string($0.resourceType.rawValue) },
                "fhir_resource": resource.flatMap {
                    (try? JSONSerialization.jsonObject(with: $0.data)).flatMap(JSONValue.from(any:))
                },
            ])]
        case let gad as HKGAD7Assessment:
            return [common.record("health.sample", [
                "identifier": .string(sample.sampleType.identifier),
                "details": .object([
                    "answers": .array(gad.answers.map { .int($0.rawValue) }),
                    "risk": .int(gad.risk.rawValue),
                ]),
                "metadata": common.metadata,
            ])]
        case let phq as HKPHQ9Assessment:
            return [common.record("health.sample", [
                "identifier": .string(sample.sampleType.identifier),
                "details": .object([
                    "answers": .array(phq.answers.map { .int($0.rawValue) }),
                    "risk": .int(phq.risk.rawValue),
                ]),
                "metadata": common.metadata,
            ])]
        default:
            return [common.record("health.sample", [
                "identifier": .string(sample.sampleType.identifier),
                "metadata": common.metadata,
            ])]
        }
    }

    private func workoutData(_ workout: HKWorkout, units: inout UnitCache) async -> [String: JSONValue?] {
        var statistics: [String: JSONValue] = [:]
        for (type, stats) in workout.allStatistics {
            guard let unit = await units.unit(for: type) else { continue }
            statistics[type.identifier] = .object([
                "sum": stats.sumQuantity().flatMap { .number($0.doubleValue(for: unit)) },
                "average": stats.averageQuantity().flatMap { .number($0.doubleValue(for: unit)) },
                "minimum": stats.minimumQuantity().flatMap { .number($0.doubleValue(for: unit)) },
                "maximum": stats.maximumQuantity().flatMap { .number($0.doubleValue(for: unit)) },
                "unit": .string(unit.unitString),
            ])
        }
        let events: [JSONValue] = (workout.workoutEvents ?? []).map { event in
            .object([
                "type": .int(event.type.rawValue),
                "start_ms": .int(event.dateInterval.start.millis),
                "end_ms": .int(event.dateInterval.end.millis),
                "metadata": event.metadata.flatMap { JSONValue.from(any: $0) },
            ])
        }
        let activities: [JSONValue] = workout.workoutActivities.map { activity in
            .object([
                "uuid": .string(activity.uuid.uuidString),
                "activity_type": .int(Int(activity.workoutConfiguration.activityType.rawValue)),
                "start_ms": .int(activity.startDate.millis),
                "end_ms": activity.endDate.map { .int($0.millis) },
                "duration_s": .number(activity.duration),
            ])
        }
        let raw = workout.workoutActivityType.rawValue
        return [
            "activity_type": .int(Int(raw)),
            "activity_name": HealthTypes.workoutNames[raw].map { .string($0) },
            "duration_s": .number(max(0, workout.duration)),
            "statistics": .object(statistics),
            "events": events.isEmpty ? nil : .array(events),
            "activities": activities.isEmpty ? nil : .array(activities),
            "metadata": workout.metadata.flatMap { JSONValue.from(any: $0) },
        ]
    }

    /// The workout's GPS routes, one record per route.
    private func routes(for workout: HKWorkout) async -> [RecordDraft] {
        let routes: [HKWorkoutRoute] = await withCheckedContinuation { continuation in
            let query = HKSampleQuery(
                sampleType: HKSeriesType.workoutRoute(),
                predicate: HKQuery.predicateForObjects(from: workout),
                limit: HKObjectQueryNoLimit, sortDescriptors: nil
            ) { _, samples, _ in
                continuation.resume(returning: (samples as? [HKWorkoutRoute]) ?? [])
            }
            store.execute(query)
        }
        var out: [RecordDraft] = []
        for route in routes {
            let points = await locations(route).map { location -> JSONValue in
                var fields = LocationService.pointFields(location)
                fields["t_ms"] = .int(location.timestamp.millis)
                return .object(fields)
            }
            out.append(Common(route).record("health.workout_route", [
                "workout_id": .string(workout.uuid.uuidString),
                "points": .array(points),
                "metadata": route.metadata.flatMap { JSONValue.from(any: $0) },
            ]))
        }
        return out
    }

    private func locations(_ route: HKWorkoutRoute) async -> [CLLocation] {
        let collected = LockedBox<[CLLocation]>([])
        return await withCheckedContinuation { continuation in
            let query = HKWorkoutRouteQuery(route: route) { _, batch, done, error in
                if let batch { collected.modify { $0.append(contentsOf: batch) } }
                if done || error != nil { continuation.resume(returning: collected.value) }
            }
            store.execute(query)
        }
    }

    private func voltages(_ ecg: HKElectrocardiogram) async -> [JSONValue] {
        let collected = LockedBox<[JSONValue]>([])
        return await withCheckedContinuation { continuation in
            let query = HKElectrocardiogramQuery(ecg) { _, result in
                switch result {
                case .measurement(let m):
                    if let v = m.quantity(for: .appleWatchSimilarToLeadI)?.doubleValue(for: .voltUnit(with: .micro)),
                       let point = JSONValue.number(v), let t = JSONValue.number(m.timeSinceSampleStart) {
                        collected.modify { $0.append(.array([t, point])) }
                    }
                case .done, .error:
                    continuation.resume(returning: collected.value)
                @unknown default:
                    continuation.resume(returning: collected.value)
                }
            }
            store.execute(query)
        }
    }

    private func beats(_ series: HKHeartbeatSeriesSample) async -> [JSONValue] {
        let collected = LockedBox<[JSONValue]>([])
        return await withCheckedContinuation { continuation in
            let query = HKHeartbeatSeriesQuery(heartbeatSeries: series) { _, time, gap, done, error in
                if error == nil, let t = JSONValue.number(time) {
                    collected.modify { $0.append(.array([t, .bool(gap)])) }
                }
                if done || error != nil { continuation.resume(returning: collected.value) }
            }
            store.execute(query)
        }
    }

    static func sensitivityPoint(_ point: HKAudiogramSensitivityPoint) -> JSONValue {
        let hz = point.frequency.doubleValue(for: .hertz())
        guard #available(iOS 18.1, *) else { return .object(["frequency_hz": .number(hz)]) }
        let tests: [JSONValue] = point.tests.map { test in
            .object([
                "sensitivity_db_hl": .number(test.sensitivity.doubleValue(for: .decibelHearingLevel())),
                "type": .int(test.type.rawValue),
                "masked": .bool(test.masked),
                "side": .int(test.side.rawValue),
            ])
        }
        return .object(["frequency_hz": .number(hz), "tests": .array(tests)])
    }

    // MARK: Characteristics

    private func characteristics() -> [RecordDraft] {
        let now = Date()
        func record(_ identifier: HKCharacteristicTypeIdentifier, _ value: JSONValue) -> RecordDraft {
            makeRecord(type: "health.characteristic", sourceID: identifier.rawValue, start: now,
                       data: .object(["identifier": .string(identifier.rawValue), "value": value]))
        }
        var out: [RecordDraft] = []
        if let v = try? store.biologicalSex().biologicalSex.rawValue { out.append(record(.biologicalSex, .int(v))) }
        if let v = try? store.bloodType().bloodType.rawValue { out.append(record(.bloodType, .int(v))) }
        if let v = try? store.fitzpatrickSkinType().skinType.rawValue {
            out.append(record(.fitzpatrickSkinType, .int(v)))
        }
        if let v = try? store.wheelchairUse().wheelchairUse.rawValue { out.append(record(.wheelchairUse, .int(v))) }
        if let v = try? store.activityMoveMode().activityMoveMode.rawValue {
            out.append(record(.activityMoveMode, .int(v)))
        }
        if let c = try? store.dateOfBirthComponents(), let year = c.year {
            out.append(record(.dateOfBirth, .object([
                "year": .int(year), "month": c.month.map { .int($0) }, "day": c.day.map { .int($0) },
            ])))
        }
        return out
    }

    // MARK: Anchors

    static func encodeAnchor(_ anchor: HKQueryAnchor) -> Data? {
        try? NSKeyedArchiver.archivedData(withRootObject: anchor, requiringSecureCoding: true)
    }

    static func decodeAnchor(_ data: Data) -> HKQueryAnchor? {
        try? NSKeyedUnarchiver.unarchivedObject(ofClass: HKQueryAnchor.self, from: data)
    }
}

/// Fields every HealthKit record shares: UUID, times, time zone, and origin.
private struct Common {
    let sample: HKSample

    init(_ sample: HKSample) {
        self.sample = sample
    }

    var metadata: JSONValue? {
        guard let metadata = sample.metadata, !metadata.isEmpty else { return nil }
        return .object(metadata.compactMapValues { value in
            if let quantity = value as? HKQuantity {
                // Metadata quantities carry no unit of their own; keep HealthKit's text form.
                return .string(quantity.description)
            }
            return JSONValue.from(any: value)
        })
    }

    func record(_ type: String, _ data: [String: JSONValue?]) -> RecordDraft {
        let source = sample.sourceRevision
        let timeZone = (sample.metadata?[HKMetadataKeyTimeZone] as? String)
            .flatMap { TimeZone(identifier: $0)?.identifier }
        let start = sample.startDate
        let end: Date? = switch type {
        case "health.characteristic": nil
        default: max(sample.endDate, start)
        }
        return makeRecord(
            type: type, sourceID: sample.uuid.uuidString, start: start, end: end,
            data: .object(data), timeZone: timeZone,
            sourceName: source.source.name, sourceBundleID: source.source.bundleIdentifier,
            sourceDevice: sample.device?.name ?? sample.device?.model
        )
    }
}

/// Caches each quantity type's unit for one run: the unit Apple Health shows
/// the user, or the first compatible fallback.
struct UnitCache {
    let store: HKHealthStore
    private var units: [HKQuantityType: HKUnit?] = [:]

    init(store: HKHealthStore) {
        self.store = store
    }

    mutating func unit(for type: HKQuantityType) async -> HKUnit? {
        if let cached = units[type] { return cached }
        var unit = try? await store.preferredUnits(for: [type])[type]
        if unit == nil {
            unit = HealthTypes.fallbackUnits.first { type.is(compatibleWith: $0) }
        }
        units[type] = unit
        return unit
    }
}

/// HealthKit's observer completion handler, callable from any thread.
private struct CompletionBox: @unchecked Sendable {
    let call: () -> Void
    init(_ call: @escaping () -> Void) { self.call = call }
}

/// A value collected from HealthKit callbacks on its own queue.
final class LockedBox<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: Value

    init(_ value: Value) { stored = value }

    var value: Value { lock.withLock { stored } }

    func modify(_ change: (inout Value) -> Void) {
        lock.withLock { change(&stored) }
    }
}

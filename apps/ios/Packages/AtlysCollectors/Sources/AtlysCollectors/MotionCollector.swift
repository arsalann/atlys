import AtlysCore
import CoreMotion
import Foundation

/// Core Motion activity updates and pedometer totals. iOS keeps about seven
/// days of history, so each run catches up from the last cursor (at most
/// seven days back) and the app warns when the gap nears that limit.
public struct MotionCollector: Collector {
    public let id = CollectorID.motion
    /// How far back iOS keeps motion history.
    public static let historyLimit: TimeInterval = 7 * 24 * 3600
    static let hour: TimeInterval = 3600

    public init() {}

    public func permission() async -> CollectorPermission {
        guard CMMotionActivityManager.isActivityAvailable() || CMPedometer.isStepCountingAvailable() else {
            return .unavailable
        }
        return switch CMMotionActivityManager.authorizationStatus() {
        case .authorized: .granted
        case .notDetermined: .notDetermined
        case .denied, .restricted: .denied
        @unknown default: .denied
        }
    }

    /// Core Motion prompts on the first query, so ask with a tiny one.
    public func requestPermission() async {
        let manager = CMMotionActivityManager()
        let now = Date()
        _ = try? await Self.activities(manager, from: now.addingTimeInterval(-60), to: now)
    }

    public func collect(_ context: CollectionContext) async throws -> CollectionOutcome {
        var state = context.state(id)
        let now = Date()
        let earliest = now.addingTimeInterval(-Self.historyLimit + 60)
        var outcome = CollectionOutcome()
        let tz = TimeZone.current.identifier

        if CMMotionActivityManager.isActivityAvailable() {
            // Overlap the previous run by an hour: records dedupe by start time.
            let from = max(state.int("activityCursorMs").map { Date(millis: $0) - Self.hour } ?? earliest, earliest)
            let activities = try await Self.activities(CMMotionActivityManager(), from: from, to: now)
            let records = activities.map { activity in
                makeRecord(
                    type: "device.motion_activity", sourceID: "motion-\(activity.startDate.millis)",
                    start: activity.startDate,
                    data: .object([
                        "stationary": .bool(activity.stationary),
                        "walking": .bool(activity.walking),
                        "running": .bool(activity.running),
                        "cycling": .bool(activity.cycling),
                        "automotive": .bool(activity.automotive),
                        "unknown": .bool(activity.unknown),
                        "confidence": .int(activity.confidence.rawValue),
                    ]),
                    timeZone: tz
                )
            }
            outcome = outcome + (try context.seal(id, records: records))
            state.setInt("activityCursorMs", now.millis)
            try context.saveState(state, for: id)
        }

        if CMPedometer.isStepCountingAvailable() {
            let pedometer = CMPedometer()
            // Hourly buckets; the current hour is re-read on the next run.
            let lastHour = state.int("pedometerHourMs").map { Date(millis: $0) }
            var hour = Self.hourStart(max(lastHour ?? earliest, earliest))
            var records: [RecordDraft] = []
            while hour < now, !context.timeIsUp {
                let end = min(hour + Self.hour, now)
                if let data = try? await Self.pedometer(pedometer, from: hour, to: end) {
                    records.append(makeRecord(
                        type: "device.pedometer", sourceID: "pedometer-\(hour.millis)", start: hour, end: end,
                        data: .object([
                            "steps": .int(data.numberOfSteps.intValue),
                            "distance_m": data.distance.flatMap { .number($0.doubleValue) },
                            "floors_ascended": data.floorsAscended.map { .int($0.intValue) },
                            "floors_descended": data.floorsDescended.map { .int($0.intValue) },
                            "average_active_pace_s_per_m": data.averageActivePace.flatMap { .number($0.doubleValue) },
                        ]),
                        timeZone: tz
                    ))
                }
                // Keep the cursor on the last complete hour.
                if end < now || end == hour + Self.hour { state.setInt("pedometerHourMs", hour.millis) }
                hour = hour + Self.hour
            }
            outcome = outcome + (try context.seal(id, records: records))
            outcome.moreAvailable = outcome.moreAvailable || hour < Self.hourStart(now)
            try context.saveState(state, for: id)
        }
        return outcome
    }

    /// Start of the hour containing `date`.
    static func hourStart(_ date: Date) -> Date {
        Date(timeIntervalSince1970: (date.timeIntervalSince1970 / hour).rounded(.down) * hour)
    }

    static func activities(_ manager: CMMotionActivityManager, from: Date, to: Date) async throws -> [CMMotionActivity] {
        try await withCheckedThrowingContinuation { continuation in
            manager.queryActivityStarting(from: from, to: to, to: OperationQueue()) { activities, error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: (activities ?? []).map(SendableActivity.init).map(\.value))
                }
            }
        }
    }

    static func pedometer(_ pedometer: CMPedometer, from: Date, to: Date) async throws -> CMPedometerData {
        try await withCheckedThrowingContinuation { continuation in
            pedometer.queryPedometerData(from: from, to: to) { data, error in
                if let data {
                    continuation.resume(returning: SendablePedometerData(value: data).value)
                } else {
                    continuation.resume(throwing: error ?? CocoaError(.featureUnsupported))
                }
            }
        }
    }
}

/// Core Motion results are immutable value objects delivered once.
private struct SendableActivity: @unchecked Sendable {
    let value: CMMotionActivity
    init(_ value: CMMotionActivity) { self.value = value }
}

private struct SendablePedometerData: @unchecked Sendable {
    let value: CMPedometerData
}

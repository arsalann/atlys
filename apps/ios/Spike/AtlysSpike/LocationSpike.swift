import AtlysCore
import CoreLocation
import Foundation

/// Visit monitoring + significant location changes (M0 task 0.8).
///
/// iOS relaunches the app in the background to deliver both, without a
/// background mode. Each delivery is sealed into the inbox immediately.
@MainActor
final class LocationSpike: NSObject, CLLocationManagerDelegate {
    static let shared = LocationSpike()

    private let manager = CLLocationManager()
    private var session: CLServiceSession?

    private(set) var authorization: CLAuthorizationStatus = .notDetermined
    var onChange: (@MainActor () -> Void)?

    override private init() {
        super.init()
        manager.delegate = self
        authorization = manager.authorizationStatus
    }

    /// Called at every launch, including background relaunches.
    func start(relaunchedForLocation: Bool) {
        if relaunchedForLocation {
            WakeLog.record("location-relaunch", outcome: "app relaunched by Core Location")
        }
        guard manager.authorizationStatus == .authorizedAlways else { return }
        // iOS 18: hold a session so the app keeps its Always authorization in
        // the background. Visits and significant changes then relaunch the app.
        session = CLServiceSession(authorization: .always)
        manager.startMonitoringVisits()
        manager.startMonitoringSignificantLocationChanges()
    }

    func requestWhenInUse() { manager.requestWhenInUseAuthorization() }
    func requestAlways() { manager.requestAlwaysAuthorization() }

    // MARK: CLLocationManagerDelegate

    nonisolated func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        let status = manager.authorizationStatus
        MainActor.assumeIsolated {
            authorization = status
            if status == .authorizedAlways { start(relaunchedForLocation: false) }
            onChange?()
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didVisit visit: CLVisit) {
        let record = Self.record(for: visit)
        MainActor.assumeIsolated { seal([record], trigger: "visit") }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        let records = locations.map(Self.record(for:))
        MainActor.assumeIsolated { seal(records, trigger: "significant-change") }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didFailWithError error: any Error) {
        let message = (error as NSError).code
        MainActor.assumeIsolated { WakeLog.record("location-error", outcome: "CLError \(message)") }
    }

    // MARK: Sealing

    private func seal(_ records: [RecordDraft], trigger: String) {
        do {
            let name = try SpikeVaultStore.sealer().seal(collector: "location", records: records)
            WakeLog.record(trigger, outcome: "sealed \(records.count) record(s) -> \(name)")
        } catch {
            WakeLog.record(trigger, outcome: "seal failed: \(error)")
        }
        onChange?()
    }

    nonisolated private static func record(for visit: CLVisit) -> RecordDraft {
        let arrival = visit.arrivalDate == .distantPast ? nil : visit.arrivalDate
        let departure = visit.departureDate == .distantFuture ? nil : visit.departureDate
        let data: [String: Any] = [
            "lat": visit.coordinate.latitude,
            "lon": visit.coordinate.longitude,
            "horizontal_accuracy_m": visit.horizontalAccuracy,
            "arrival_known": arrival != nil,
            "departure_known": departure != nil,
        ]
        return RecordDraft(
            recordType: "location.visit",
            sourceId: UUID().uuidString,
            startMs: (arrival ?? departure ?? .now).millis,
            endMs: departure?.millis,
            dataJson: json(data)
        )
    }

    nonisolated private static func record(for location: CLLocation) -> RecordDraft {
        var data: [String: Any] = [
            "lat": location.coordinate.latitude,
            "lon": location.coordinate.longitude,
            "horizontal_accuracy_m": location.horizontalAccuracy,
        ]
        if location.speed >= 0 { data["speed_mps"] = location.speed }
        if location.course >= 0 { data["course_deg"] = location.course }
        return RecordDraft(recordType: "location.point", sourceId: UUID().uuidString,
                           startMs: location.timestamp.millis, endMs: nil, dataJson: json(data))
    }
}

extension Date {
    var millis: Int64 { Int64((timeIntervalSince1970 * 1000).rounded()) }
}

func json(_ object: [String: Any]) -> String {
    let data = (try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])) ?? Data("{}".utf8)
    return String(decoding: data, as: UTF8.self)
}

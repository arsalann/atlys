import AtlysCore
import CoreLocation
import Foundation

/// The app's one `CLLocationManager`. Created at launch, because iOS
/// relaunches the app in the background to deliver visits and significant
/// location changes and only delivers them to a manager that exists.
///
/// Events are sealed into the inbox as they arrive, so they are kept even
/// while the vault is locked.
@MainActor
public final class LocationService: NSObject {
    public static let detailedTrackingKey = "detailedLocationTracking"

    let manager = CLLocationManager()
    private let sealer: @Sendable () throws -> any RecordSealing
    private let states: CollectorStateStore
    private let defaults: UserDefaults
    private var authorizationWaiters: [CheckedContinuation<Void, Never>] = []
    private var detailedTask: Task<Void, Never>?
    private var backgroundSession: CLBackgroundActivitySession?

    public init(
        states: CollectorStateStore,
        defaults: UserDefaults = .standard,
        sealer: @escaping @Sendable () throws -> any RecordSealing
    ) {
        self.sealer = sealer
        self.states = states
        self.defaults = defaults
        super.init()
        manager.delegate = self
        manager.pausesLocationUpdatesAutomatically = true
    }

    public var authorization: CLAuthorizationStatus { manager.authorizationStatus }

    public var permission: CollectorPermission {
        guard CLLocationManager.locationServicesEnabled() else { return .denied }
        return switch manager.authorizationStatus {
        case .authorizedAlways: .granted
        case .authorizedWhenInUse: .limited
        case .notDetermined: .notDetermined
        default: .denied
        }
    }

    /// Detailed tracking: continuous route-level points in the background.
    /// Off by default; it uses noticeably more battery.
    public var detailedTracking: Bool {
        get { defaults.bool(forKey: Self.detailedTrackingKey) }
        set {
            defaults.set(newValue, forKey: Self.detailedTrackingKey)
            startMonitoring()
        }
    }

    /// Starts every kind of monitoring the current authorization allows.
    /// Call at every launch.
    public func startMonitoring() {
        switch manager.authorizationStatus {
        case .authorizedAlways, .authorizedWhenInUse:
            if CLLocationManager.significantLocationChangeMonitoringAvailable() {
                manager.startMonitoringSignificantLocationChanges()
            }
            manager.startMonitoringVisits()
        default:
            break
        }
        if detailedTracking, permission == .granted || permission == .limited {
            startDetailed()
        } else {
            stopDetailed()
        }
    }

    /// When In Use first, then the upgrade to Always.
    public func requestPermission() async {
        if manager.authorizationStatus == .notDetermined {
            manager.requestWhenInUseAuthorization()
            await waitForAuthorizationChange()
        }
        if manager.authorizationStatus == .authorizedWhenInUse {
            manager.requestAlwaysAuthorization()
            await waitForAuthorizationChange(timeout: 30)
        }
        startMonitoring()
    }

    private func waitForAuthorizationChange(timeout: TimeInterval = 120) async {
        await withCheckedContinuation { continuation in
            authorizationWaiters.append(continuation)
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(timeout))
                self?.resumeWaiters()
            }
        }
    }

    private func resumeWaiters() {
        let waiters = authorizationWaiters
        authorizationWaiters.removeAll()
        waiters.forEach { $0.resume() }
    }

    /// One fix now (foreground runs and "Collect now").
    public func currentLocation(timeout: TimeInterval = 10) async -> CLLocation? {
        guard permission == .granted || permission == .limited else { return nil }
        return await withTaskGroup(of: CLLocation?.self) { group in
            group.addTask {
                do {
                    for try await update in CLLocationUpdate.liveUpdates() {
                        if let location = update.location, location.horizontalAccuracy >= 0 { return location }
                    }
                } catch {}
                return nil
            }
            group.addTask {
                try? await Task.sleep(for: .seconds(timeout))
                return nil
            }
            let first = await group.next() ?? nil
            group.cancelAll()
            return first
        }
    }

    // MARK: Detailed tracking

    private func startDetailed() {
        guard detailedTask == nil else { return }
        backgroundSession = CLBackgroundActivitySession()
        let sealer = self.sealer
        let states = self.states
        detailedTask = Task.detached(priority: .utility) {
            var buffer: [CLLocation] = []
            var lastFlush = Date()
            do {
                for try await update in CLLocationUpdate.liveUpdates(.fitness) {
                    if Task.isCancelled { break }
                    if let location = update.location, location.horizontalAccuracy >= 0 {
                        buffer.append(location)
                    }
                    if buffer.count >= 60 || Date().timeIntervalSince(lastFlush) > 120 {
                        Self.seal(points: buffer, trigger: "detailed", sealer: sealer, states: states)
                        buffer.removeAll()
                        lastFlush = Date()
                    }
                }
            } catch {}
            Self.seal(points: buffer, trigger: "detailed", sealer: sealer, states: states)
        }
    }

    private func stopDetailed() {
        detailedTask?.cancel()
        detailedTask = nil
        backgroundSession?.invalidate()
        backgroundSession = nil
    }

    // MARK: Records

    nonisolated static func seal(
        points: [CLLocation], trigger: String,
        sealer: @Sendable () throws -> any RecordSealing, states: CollectorStateStore
    ) {
        guard !points.isEmpty else { return }
        seal(points.map { pointRecord($0, trigger: trigger) }, sealer: sealer, states: states)
    }

    nonisolated static func seal(
        _ records: [RecordDraft], sealer: @Sendable () throws -> any RecordSealing, states: CollectorStateStore
    ) {
        do {
            try sealer().seal(collector: .location, records: records, deletions: [])
            try states.update(.location) {
                $0.recordsSealed += Int64(records.count)
                $0.lastRunMs = Date().millis
                $0.lastSuccessMs = Date().millis
                $0.lastError = nil
            }
        } catch {
            _ = try? states.update(.location) { $0.lastError = CollectionCoordinator.describe(error) }
        }
    }

    nonisolated static func pointRecord(_ location: CLLocation, trigger: String) -> RecordDraft {
        var fields = pointFields(location)
        fields["trigger"] = .string(trigger)
        return makeRecord(
            type: "location.point",
            sourceID: "point-\(location.timestamp.millis)",
            start: location.timestamp,
            data: .object(fields),
            timeZone: TimeZone.current.identifier
        )
    }

    /// Every field Core Location reports, leaving out invalid (negative) values.
    nonisolated static func pointFields(_ location: CLLocation) -> [String: JSONValue?] {
        let nonNegative = { (v: Double) -> JSONValue? in v >= 0 ? .number(v) : nil }
        let validVertical = location.verticalAccuracy >= 0
        let info = location.sourceInformation
        return [
            "latitude": .number(location.coordinate.latitude),
            "longitude": .number(location.coordinate.longitude),
            "horizontal_accuracy_m": .number(location.horizontalAccuracy),
            "altitude_m": validVertical ? .number(location.altitude) : nil,
            "ellipsoidal_altitude_m": validVertical ? .number(location.ellipsoidalAltitude) : nil,
            "vertical_accuracy_m": nonNegative(location.verticalAccuracy),
            "speed_mps": location.speedAccuracy >= 0 ? nonNegative(location.speed) : nil,
            "speed_accuracy_mps": nonNegative(location.speedAccuracy),
            "course_deg": location.courseAccuracy >= 0 && location.course >= 0 && location.course <= 360
                ? .number(location.course) : nil,
            "course_accuracy_deg": nonNegative(location.courseAccuracy),
            "floor": location.floor.map { .int($0.level) },
            "simulated": info.map { .bool($0.isSimulatedBySoftware) },
            "from_accessory": info.map { .bool($0.isProducedByAccessory) },
        ]
    }

    nonisolated static func visitRecord(_ visit: CLVisit) -> RecordDraft? {
        let arrival = visit.arrivalDate == .distantPast ? nil : visit.arrivalDate
        let departure = visit.departureDate == .distantFuture ? nil : visit.departureDate
        guard let start = arrival ?? departure, visit.horizontalAccuracy >= 0 else { return nil }
        return makeRecord(
            type: "location.visit",
            // A visit is first reported on arrival, then again with its departure:
            // the arrival time keys both so the second replaces the first.
            sourceID: arrival.map { "visit-\($0.millis)" } ?? "visit-departure-\(start.millis)",
            start: start,
            end: arrival != nil ? departure : nil,
            data: .object([
                "latitude": .number(visit.coordinate.latitude),
                "longitude": .number(visit.coordinate.longitude),
                "horizontal_accuracy_m": .number(visit.horizontalAccuracy),
            ]),
            timeZone: TimeZone.current.identifier
        )
    }
}

extension LocationService: CLLocationManagerDelegate {
    nonisolated public func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        MainActor.assumeIsolated {
            resumeWaiters()
            startMonitoring()
        }
    }

    nonisolated public func locationManager(_ manager: CLLocationManager, didVisit visit: CLVisit) {
        guard let record = Self.visitRecord(visit) else { return }
        let (sealer, states) = MainActor.assumeIsolated { (self.sealer, self.states) }
        Self.seal([record], sealer: sealer, states: states)
    }

    nonisolated public func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        let (sealer, states) = MainActor.assumeIsolated { (self.sealer, self.states) }
        Self.seal(points: locations.filter { $0.horizontalAccuracy >= 0 }, trigger: "significant_change",
                  sealer: sealer, states: states)
    }

    nonisolated public func locationManager(_ manager: CLLocationManager, didFailWithError error: any Error) {}
}

/// Adapts ``LocationService`` to the coordinator: a fix on foreground runs and
/// "Collect now". Visits and significant changes arrive through the service.
public struct LocationCollector: Collector {
    public let id = CollectorID.location
    let service: LocationService

    public init(service: LocationService) {
        self.service = service
    }

    public func permission() async -> CollectorPermission {
        await service.permission
    }

    public func requestPermission() async {
        await service.requestPermission()
    }

    public func collect(_ context: CollectionContext) async throws -> CollectionOutcome {
        guard context.trigger == .foreground || context.trigger == .manual else { return CollectionOutcome() }
        let remaining = max(1, min(10, context.deadline.timeIntervalSinceNow))
        guard let location = await service.currentLocation(timeout: remaining) else { return CollectionOutcome() }
        return try context.seal(id, records: [LocationService.pointRecord(location, trigger: "foreground")])
    }
}

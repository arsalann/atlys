import AtlysCore
import Foundation
import Network
import UIKit

/// Battery, network path, and device snapshots on each wake (no permission).
/// No network requests are made: `NWPathMonitor` only observes the path.
public struct DeviceCollector: Collector {
    public let id = CollectorID.device
    /// Snapshots closer together than this are skipped, except on "Collect now".
    public var minimumInterval: TimeInterval = 15 * 60

    public init() {}

    public func permission() async -> CollectorPermission { .notRequired }

    public func requestPermission() async {}

    public func collect(_ context: CollectionContext) async throws -> CollectionOutcome {
        let state = context.state(id)
        let now = Date()
        if context.trigger != .manual, let last = state.int("lastSnapshotMs"),
           now.timeIntervalSince(Date(millis: last)) < minimumInterval {
            return CollectionOutcome()
        }
        let tz = TimeZone.current.identifier
        let stamp = now.millis
        var records = [
            makeRecord(type: "device.battery", sourceID: "battery-\(stamp)", start: now,
                       data: await Self.battery(), timeZone: tz),
            makeRecord(type: "device.info", sourceID: "info-\(stamp)", start: now, data: await Self.info(), timeZone: tz),
        ]
        if let path = await Self.currentPath() {
            records.append(makeRecord(type: "device.connectivity", sourceID: "net-\(stamp)", start: now,
                                      data: path, timeZone: tz))
        }
        let outcome = try context.seal(id, records: records)
        var updated = context.state(id)
        updated.setInt("lastSnapshotMs", stamp)
        try context.saveState(updated, for: id)
        return outcome
    }

    @MainActor
    static func battery() -> JSONValue {
        let device = UIDevice.current
        let wasMonitoring = device.isBatteryMonitoringEnabled
        device.isBatteryMonitoringEnabled = true
        defer { device.isBatteryMonitoringEnabled = wasMonitoring }
        let state: String = switch device.batteryState {
        case .unplugged: "unplugged"
        case .charging: "charging"
        case .full: "full"
        default: "unknown"
        }
        let level = device.batteryLevel
        return .object([
            "level": level >= 0 ? .number(Double(level)) : nil,
            "state": .string(state),
            "low_power_mode": .bool(ProcessInfo.processInfo.isLowPowerModeEnabled),
        ])
    }

    @MainActor
    static func info() -> JSONValue {
        let process = ProcessInfo.processInfo
        let bundle = Bundle.main
        let version = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
        let build = bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "?"
        let thermal: String = switch process.thermalState {
        case .nominal: "nominal"
        case .fair: "fair"
        case .serious: "serious"
        case .critical: "critical"
        @unknown default: "nominal"
        }
        let home = URL(fileURLWithPath: NSHomeDirectory())
        let storage = try? home.resourceValues(forKeys: [
            .volumeAvailableCapacityForImportantUsageKey, .volumeTotalCapacityKey,
        ])
        return .object([
            "os_name": .string(UIDevice.current.systemName),
            "os_version": .string(UIDevice.current.systemVersion),
            "model": .string(modelIdentifier()),
            "app_version": .string("\(version) (\(build))"),
            "free_storage_bytes": storage?.volumeAvailableCapacityForImportantUsage.map { .int($0) },
            "total_storage_bytes": storage?.volumeTotalCapacity.map { .int($0) },
            "locale": .string(Locale.current.identifier),
            "time_zone": .string(TimeZone.current.identifier),
            "thermal_state": .string(thermal),
            "uptime_s": .number(process.systemUptime),
            "physical_memory_bytes": .int(Int64(clamping: process.physicalMemory)),
        ])
    }

    /// Hardware identifier such as `iPhone17,1` (the Simulator reports its host's).
    static func modelIdentifier() -> String {
        if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] {
            return simulated
        }
        var info = utsname()
        uname(&info)
        let name = withUnsafeBytes(of: &info.machine) { raw in
            String(decoding: raw.prefix { $0 != 0 }, as: UTF8.self)
        }
        return name.isEmpty ? "unknown" : name
    }

    /// The current network path, or `nil` if the monitor does not report
    /// within a second.
    static func currentPath() async -> JSONValue? {
        let monitor = NWPathMonitor()
        let queue = DispatchQueue(label: "org.atlys.collectors.path")
        let path: NWPath? = await withCheckedContinuation { continuation in
            let once = Once()
            monitor.pathUpdateHandler = { path in
                if once.claim() { continuation.resume(returning: path) }
            }
            monitor.start(queue: queue)
            queue.asyncAfter(deadline: .now() + 1) {
                if once.claim() { continuation.resume(returning: nil) }
            }
        }
        monitor.cancel()
        guard let path else { return nil }
        let status: String = switch path.status {
        case .satisfied: "satisfied"
        case .requiresConnection: "requires_connection"
        default: "unsatisfied"
        }
        let interface: String =
            if path.status != .satisfied { "none" }
            else if path.usesInterfaceType(.wifi) { "wifi" }
            else if path.usesInterfaceType(.cellular) { "cellular" }
            else if path.usesInterfaceType(.wiredEthernet) { "wired" }
            else if path.usesInterfaceType(.loopback) { "loopback" }
            else { "other" }
        return .object([
            "status": .string(status),
            "interface": .string(interface),
            "expensive": .bool(path.isExpensive),
            "constrained": .bool(path.isConstrained),
            "supports_ipv4": .bool(path.supportsIPv4),
            "supports_ipv6": .bool(path.supportsIPv6),
            "supports_dns": .bool(path.supportsDNS),
        ])
    }
}

/// A flag that can be claimed once, from any thread.
final class Once: @unchecked Sendable {
    private let lock = NSLock()
    private var claimed = false

    func claim() -> Bool {
        lock.withLock {
            if claimed { return false }
            claimed = true
            return true
        }
    }
}

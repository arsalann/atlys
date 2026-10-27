import AtlysCore
import BackgroundTasks
import Foundation

/// Owns the collectors for the app's lifetime and wires up every trigger:
/// HealthKit background delivery, location relaunches, `BGAppRefreshTask`,
/// and `BGProcessingTask`. Call ``bootstrap()`` from
/// `application(_:didFinishLaunchingWithOptions:)`.
@MainActor
public final class CollectionService {
    public static let shared = CollectionService()

    /// Background refresh budget. iOS allows about 30 seconds.
    static let refreshBudget: TimeInterval = 25
    /// Processing tasks run while charging and may take minutes.
    static let processingBudget: TimeInterval = 600

    public let vaultRoot: URL
    public let states: CollectorStateStore
    public let location: LocationService
    public let health: HealthCollector
    public let coordinator: CollectionCoordinator
    private var bootstrapped = false

    public init(vaultRoot: URL? = nil, defaults: UserDefaults = .standard) {
        let root = vaultRoot ?? ((try? VaultLocation.applicationSupport().root)
            ?? FileManager.default.temporaryDirectory.appending(path: "Atlys"))
        self.vaultRoot = root
        let states = CollectorStateStore(vaultRoot: root)
        self.states = states
        let sealer: @Sendable () throws -> any RecordSealing = {
            try InboxLocation(vaultRoot: root).sealer()
        }
        let location = LocationService(states: states, defaults: defaults, sealer: sealer)
        self.location = location
        let health = HealthCollector()
        self.health = health
        coordinator = CollectionCoordinator(
            collectors: [DeviceCollector(), MotionCollector(), LocationCollector(service: location), health],
            states: states,
            makeSealer: sealer
        )
    }

    public var refreshTaskID: String { (Bundle.main.bundleIdentifier ?? "org.atlys") + ".refresh" }
    public var processingTaskID: String { (Bundle.main.bundleIdentifier ?? "org.atlys") + ".processing" }

    /// Registers observers and background tasks. Must run before launch
    /// finishes, so wakes that relaunched the app are delivered.
    public func bootstrap() {
        guard !bootstrapped else { return }
        bootstrapped = true
        let coordinator = self.coordinator
        health.startObserving {
            await coordinator.run(.healthDelivery, budget: Self.refreshBudget, only: [.health, .motion, .device])
        }
        location.startMonitoring()
        BGTaskScheduler.shared.register(forTaskWithIdentifier: refreshTaskID, using: nil) { task in
            let box = TaskBox(task)
            Task { @MainActor in CollectionService.shared.handle(box, trigger: .backgroundRefresh) }
        }
        BGTaskScheduler.shared.register(forTaskWithIdentifier: processingTaskID, using: nil) { task in
            let box = TaskBox(task)
            Task { @MainActor in CollectionService.shared.handle(box, trigger: .backgroundProcessing) }
        }
        scheduleBackgroundTasks()
    }

    /// Asks iOS for the next background refresh and, for long backfills, a
    /// processing task while charging.
    public func scheduleBackgroundTasks() {
        let refresh = BGAppRefreshTaskRequest(identifier: refreshTaskID)
        refresh.earliestBeginDate = Date(timeIntervalSinceNow: 30 * 60)
        try? BGTaskScheduler.shared.submit(refresh)
        let processing = BGProcessingTaskRequest(identifier: processingTaskID)
        processing.requiresExternalPower = true
        processing.requiresNetworkConnectivity = false
        processing.earliestBeginDate = Date(timeIntervalSinceNow: 60 * 60)
        try? BGTaskScheduler.shared.submit(processing)
    }

    private func handle(_ box: TaskBox, trigger: CollectionTrigger) {
        scheduleBackgroundTasks()
        let budget = trigger == .backgroundProcessing ? Self.processingBudget : Self.refreshBudget
        let coordinator = self.coordinator
        let work = Task {
            await coordinator.run(trigger, budget: budget)
            box.task.setTaskCompleted(success: true)
        }
        box.task.expirationHandler = {
            work.cancel()
            box.task.setTaskCompleted(success: false)
        }
    }

    /// Runs collectors now (app foreground or "Collect now").
    @discardableResult
    public func collect(_ trigger: CollectionTrigger, budget: TimeInterval = 20,
                        only ids: Set<CollectorID>? = nil) async -> [CollectorID: CollectionOutcome] {
        await coordinator.run(trigger, budget: budget, only: ids)
    }

    public func statuses() async -> [CollectionCoordinator.Status] {
        await coordinator.statuses()
    }

    /// Asks for one collector's permission, then collects from it.
    public func requestPermission(_ id: CollectorID) async {
        guard let collector = await coordinator.collector(id) else { return }
        await collector.requestPermission()
        await collect(.manual, only: [id])
    }
}

/// `BGTask` is used from the main actor and its own expiration handler only.
private struct TaskBox: @unchecked Sendable {
    let task: BGTask
    init(_ task: BGTask) { self.task = task }
}

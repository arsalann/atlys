import AtlysCore
import Foundation

/// Runs collectors one at a time within a time budget. Every trigger (app
/// foreground, background tasks, HealthKit and location wakes) goes through
/// here, so runs never overlap. Each collector resumes from its saved state,
/// so overlapping triggers are harmless.
public actor CollectionCoordinator {
    public struct Status: Sendable, Equatable {
        public let id: CollectorID
        public let permission: CollectorPermission
        public let state: CollectorState
        public let isRunning: Bool
    }

    public let collectors: [any Collector]
    public let states: CollectorStateStore
    private let makeSealer: @Sendable () throws -> any RecordSealing
    private var runningCollector: CollectorID?
    private var queue: [CheckedContinuation<Void, Never>] = []
    private var busy = false

    public init(
        collectors: [any Collector],
        states: CollectorStateStore,
        makeSealer: @escaping @Sendable () throws -> any RecordSealing
    ) {
        self.collectors = collectors
        self.states = states
        self.makeSealer = makeSealer
    }

    public func collector(_ id: CollectorID) -> (any Collector)? {
        collectors.first { $0.id == id }
    }

    /// Runs the given collectors (all by default) until `budget` seconds pass.
    /// Collectors without permission are skipped. Returns what each one did.
    @discardableResult
    public func run(
        _ trigger: CollectionTrigger,
        budget: TimeInterval,
        only ids: Set<CollectorID>? = nil
    ) async -> [CollectorID: CollectionOutcome] {
        await acquire()
        defer { release() }
        let deadline = Date().addingTimeInterval(budget)
        guard let sealer = try? makeSealer() else { return [:] }
        let context = CollectionContext(trigger: trigger, deadline: deadline, sealer: sealer, states: states)
        var outcomes: [CollectorID: CollectionOutcome] = [:]
        for collector in collectors where ids?.contains(collector.id) ?? true {
            if context.timeIsUp { break }
            switch await collector.permission() {
            case .granted, .limited, .notRequired: break
            case .notDetermined, .denied, .unavailable: continue
            }
            runningCollector = collector.id
            let started = Date().millis
            do {
                let outcome = try await collector.collect(context)
                outcomes[collector.id] = outcome
                _ = try? states.update(collector.id) {
                    $0.lastRunMs = started
                    $0.recordsSealed += Int64(outcome.recordsSealed)
                    if outcome.deferred {
                        $0.lastError = "Waiting for the iPhone to be unlocked"
                    } else {
                        $0.lastSuccessMs = Date().millis
                        $0.lastError = nil
                    }
                }
            } catch {
                _ = try? states.update(collector.id) {
                    $0.lastRunMs = started
                    $0.lastError = Self.describe(error)
                }
            }
            runningCollector = nil
        }
        return outcomes
    }

    /// Current status of every collector.
    public func statuses() async -> [Status] {
        var out: [Status] = []
        for collector in collectors {
            out.append(Status(
                id: collector.id, permission: await collector.permission(),
                state: states.load(collector.id), isRunning: runningCollector == collector.id
            ))
        }
        return out
    }

    /// A content-free description: error domain and code only.
    static func describe(_ error: any Error) -> String {
        if let error = error as? AtlysError { return "Atlys: \(error)" }
        let ns = error as NSError
        return "\(ns.domain) \(ns.code)"
    }

    private func acquire() async {
        if !busy {
            busy = true
            return
        }
        await withCheckedContinuation { queue.append($0) }
    }

    private func release() {
        if queue.isEmpty {
            busy = false
        } else {
            queue.removeFirst().resume()
        }
    }
}

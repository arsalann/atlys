import AtlysCollectors
import AtlysCore
import Foundation
import Observation
import UIKit

/// App state: which screen shows, the unlocked session, and lock policy.
///
/// Every call that runs Argon2id (create, unlock, change password, recover,
/// verify) runs off the main actor. Passwords arrive as Swift strings, which
/// cannot be reliably wiped; they are converted to bytes once and handed to
/// Rust, which zeroizes its copies.
@MainActor
@Observable
public final class AppModel {
    public enum Phase: Equatable {
        case launching
        case onboarding
        case locked
        case unlocked
        case failed(String)
    }

    /// Auto-lock choices in seconds. 0 locks as soon as the app leaves the
    /// foreground.
    public static let autoLockChoices: [Int] = [0, 60, 300, 900]
    static let autoLockKey = "autoLockSeconds"

    public private(set) var phase: Phase = .launching
    public private(set) var vaultID: String?
    public private(set) var biometricsEnrolled = false
    /// True while a slow core call (Argon2id) is running.
    public private(set) var isWorking = false

    /// Collector status for the Sources list.
    public private(set) var sources: [CollectionCoordinator.Status] = []
    /// Records and latest merge per collector.
    public private(set) var collectorSummaries: [CollectorSummary] = []
    public private(set) var recordCount: UInt64 = 0
    public private(set) var inbox = InboxStatus(pendingFiles: 0, quarantinedFiles: 0)
    public private(set) var isCollecting = false
    public private(set) var isMerging = false
    /// Bumped after each merge so open screens reload.
    public private(set) var dataVersion = 0

    public var autoLockSeconds: Int {
        didSet { defaults.set(autoLockSeconds, forKey: Self.autoLockKey) }
    }

    let location: VaultLocation
    let collection: CollectionService
    private let defaults: UserDefaults
    private var session: VaultSession?
    private var backgroundedAt: Date?
    private var protectedDataObserver: NSObjectProtocol?

    public init(location: VaultLocation, collection: CollectionService = .shared, defaults: UserDefaults = .standard) {
        self.location = location
        self.collection = collection
        self.defaults = defaults
        autoLockSeconds = defaults.object(forKey: Self.autoLockKey) as? Int ?? 60
    }

    /// The default model for the app: the vault in Application Support.
    public static func live() -> AppModel {
        do {
            return AppModel(location: try .applicationSupport())
        } catch {
            let model = AppModel(location: VaultLocation(root: FileManager.default.temporaryDirectory))
            model.phase = .failed("Could not find the app's storage: \(error.localizedDescription)")
            return model
        }
    }

    public var biometricsAvailable: Bool { BiometricUnlock.isAvailable }
    public var biometryName: String { BiometricUnlock.biometryName }

    /// Decides the first screen. Call once at launch.
    public func start() {
        guard phase == .launching else { return }
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("-AtlysResetForUITests") {
            try? location.removeAll()
            defaults.removeObject(forKey: Self.autoLockKey)
            autoLockSeconds = 60
        }
        #endif
        do {
            try location.prepare()
        } catch {
            phase = .failed("Could not prepare storage: \(error.localizedDescription)")
            return
        }
        observeDeviceLock()
        if location.exists {
            vaultID = try? vaultIdUnverified(vaultRoot: location.path)
            refreshBiometrics()
            phase = .locked
            Task { await collectAndMerge(.foreground) }
        } else {
            phase = .onboarding
        }
    }

    // MARK: Onboarding

    /// Creates the vault and returns the recovery phrase to show once. The
    /// app stays in onboarding until ``finishOnboarding()``.
    public func createVault(password: String) async throws -> [String] {
        let root = location.path
        let bytes = Data(password.utf8)
        let created = try await work { try AtlysCore.createVault(vaultRoot: root, password: bytes) }
        location.protectDatabase()
        session = created.session
        vaultID = try? created.session.vaultId()
        return created.recoveryPhrase
    }

    public func finishOnboarding() {
        guard session != nil else { return }
        phase = .unlocked
        Task { await collectAndMerge(.foreground) }
    }

    // MARK: Unlock and lock

    public func unlock(password: String) async throws {
        let root = location.path
        let bytes = Data(password.utf8)
        let unlocked = try await work { try unlockWithPassword(vaultRoot: root, password: bytes) }
        didUnlock(unlocked)
    }

    public func unlockWithBiometrics() async throws {
        let location = self.location
        let reason = "Unlock your Atlys vault"
        let unlocked = try await work { try BiometricUnlock.unlock(location: location, reason: reason) }
        didUnlock(unlocked)
    }

    /// Unlocks with the recovery phrase and sets a new password.
    public func recover(words: [String], newPassword: String) async throws {
        let root = location.path
        let bytes = Data(newPassword.utf8)
        let unlocked = try await work {
            try recoverVault(vaultRoot: root, recoveryPhrase: words, newPassword: bytes)
        }
        didUnlock(unlocked)
    }

    public func lock() {
        try? session?.lock()
        session = nil
        if phase == .unlocked { phase = .locked }
    }

    private func didUnlock(_ unlocked: VaultSession) {
        session = unlocked
        vaultID = try? unlocked.vaultId()
        location.protectDatabase()
        refreshBiometrics()
        phase = .unlocked
        Task { await collectAndMerge(.foreground) }
    }

    // MARK: Settings

    public func changePassword(current: String, new: String) async throws {
        let session = try requireSession()
        let currentBytes = Data(current.utf8)
        let newBytes = Data(new.utf8)
        try await work { try session.changePassword(currentPassword: currentBytes, newPassword: newBytes) }
    }

    public func regenerateRecovery() async throws -> [String] {
        let session = try requireSession()
        return try await work { try session.regenerateRecovery() }
    }

    public func setBiometrics(enabled: Bool) throws {
        let session = try requireSession()
        if enabled {
            try BiometricUnlock.enroll(session: session)
        } else if let vaultID {
            BiometricUnlock.remove(vaultID: vaultID)
        }
        refreshBiometrics()
    }

    /// Deletes the vault after fresh authentication with the password.
    public func deleteVault(password: String) async throws {
        let session = try requireSession()
        let bytes = Data(password.utf8)
        try await work { try session.verifyPassword(password: bytes) }
        try await finishDelete(session)
    }

    /// Deletes the vault after fresh authentication with Face ID.
    public func deleteVaultWithBiometrics() async throws {
        let session = try requireSession()
        let location = self.location
        let fresh = try await work {
            try BiometricUnlock.unlock(location: location, reason: "Confirm deleting your vault")
        }
        try fresh.lock()
        try await finishDelete(session)
    }

    private func finishDelete(_ session: VaultSession) async throws {
        if let vaultID { BiometricUnlock.remove(vaultID: vaultID) }
        let root = location.path
        try await work {
            do {
                try session.delete()
            } catch {
                // A partial deletion left files behind; finish them.
                try deleteVaultLeftovers(vaultRoot: root)
            }
        }
        self.session = nil
        vaultID = nil
        biometricsEnrolled = false
        recordCount = 0
        collectorSummaries = []
        phase = .onboarding
    }

    // MARK: Lifecycle

    /// Call from `scenePhase` changes.
    public func sceneDidEnterBackground() {
        guard phase == .unlocked else { return }
        if autoLockSeconds == 0 {
            lock()
        } else {
            backgroundedAt = Date()
        }
    }

    public func sceneDidBecomeActive() {
        defer { backgroundedAt = nil }
        if phase == .unlocked, let since = backgroundedAt,
           Date().timeIntervalSince(since) >= Double(autoLockSeconds) {
            lock()
        }
        if phase == .unlocked || phase == .locked {
            Task { await collectAndMerge(.foreground) }
        }
    }

    // MARK: Collection and data

    /// Runs the collectors, then merges the inbox if the vault is unlocked.
    public func collectAndMerge(_ trigger: CollectionTrigger, only ids: Set<CollectorID>? = nil) async {
        guard location.exists, !isCollecting else { return }
        isCollecting = true
        sources = await collection.statuses()
        // While the app is open and a backfill has more to read (a first
        // Apple Health import, for example), keep going in rounds, merging
        // between them so the data shows up as it arrives.
        for _ in 0..<Self.maxForegroundRounds {
            let outcomes = await collection.collect(trigger, only: ids)
            sources = await collection.statuses()
            await mergeInbox()
            let more = outcomes.values.contains { $0.moreAvailable }
            guard more, UIApplication.shared.applicationState == .active else { break }
        }
        isCollecting = false
        sources = await collection.statuses()
    }

    /// Upper bound on back-to-back foreground rounds (about 20 seconds each).
    static let maxForegroundRounds = 30

    public func collectNow() async {
        await collectAndMerge(.manual)
    }

    /// Asks iOS for a collector's permission, then collects from it.
    public func requestPermission(_ id: CollectorID) async {
        await collection.requestPermission(id)
        sources = await collection.statuses()
        await mergeInbox()
    }

    public func refreshSources() async {
        sources = await collection.statuses()
    }

    /// Detailed (continuous) location tracking. Off by default.
    public var detailedLocationTracking: Bool {
        get { collection.location.detailedTracking }
        set { collection.location.detailedTracking = newValue }
    }

    /// Merges pending batches in short slices, so other calls stay responsive.
    public func mergeInbox() async {
        guard let session, phase == .unlocked, !isMerging else { return }
        isMerging = true
        defer { isMerging = false }
        while self.session === session {
            let report = try? await Task.detached(priority: .utility) {
                try session.mergeInbox(maxSeconds: 1.0)
            }.value
            guard let report else { break }
            if report.filesMerged + report.filesReplayed + report.filesQuarantined > 0 { dataVersion += 1 }
            await refreshDataSummary()
            if report.filesRemaining == 0 { break }
        }
    }

    func refreshDataSummary() async {
        guard let session else { return }
        let summary = try? await Task.detached(priority: .utility) {
            (try session.recordCount(), try session.collectorSummaries(), try session.inboxStatus())
        }.value
        guard let summary else { return }
        recordCount = summary.0
        collectorSummaries = summary.1
        inbox = summary.2
    }

    func recordTypes() async throws -> [RecordTypeSummary] {
        let session = try requireSession()
        return try await Task.detached(priority: .userInitiated) { try session.recordTypes() }.value
    }

    func records(type: String, subtype: String?, after: RecordCursor?, limit: UInt32 = 100) async throws -> [RecordRow] {
        let session = try requireSession()
        return try await Task.detached(priority: .userInitiated) {
            try session.records(recordType: type, subtype: subtype, after: after, limit: limit)
        }.value
    }

    func recordJSON(id: String) async throws -> String? {
        let session = try requireSession()
        return try await Task.detached(priority: .userInitiated) { try session.recordJson(id: id) }.value
    }

    private func observeDeviceLock() {
        guard protectedDataObserver == nil else { return }
        protectedDataObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.protectedDataWillBecomeUnavailableNotification,
            object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.lock() }
        }
    }

    private func refreshBiometrics() {
        biometricsEnrolled = vaultID.map(BiometricUnlock.isEnrolled(vaultID:)) ?? false
    }

    private func requireSession() throws -> VaultSession {
        guard let session, phase == .unlocked || phase == .onboarding else { throw AtlysError.Locked }
        return session
    }

    /// Runs a slow core call off the main actor.
    private func work<T: Sendable>(_ body: @escaping @Sendable () throws -> T) async throws -> T {
        isWorking = true
        defer { isWorking = false }
        return try await Task.detached(priority: .userInitiated, operation: body).value
    }
}

extension Error {
    /// A user-facing message for any error from the core or the platform.
    var userMessage: String {
        if let error = self as? AtlysError { return error.userMessage }
        if let error = self as? BiometricUnlock.Failure {
            return switch error {
            case .notEnrolled: "Face ID isn't set up for this vault. Use your password."
            case .unavailable: "Face ID isn't available on this iPhone."
            case .accessControl, .keychain: "Face ID could not be set up. Try again later."
            }
        }
        return localizedDescription
    }
}

import AtlysCore
import AtlysCollectors
import CoreLocation
import SwiftUI
import UIKit

struct SpikeRootView: View {
    var body: some View {
        TabView {
            Tab("Core", systemImage: "cpu") { NavigationStack { CoreSpikeView() } }
            Tab("Argon2", systemImage: "speedometer") { NavigationStack { Argon2SpikeView() } }
            Tab("Face ID", systemImage: "faceid") { NavigationStack { FaceIDSpikeView() } }
            Tab("Background", systemImage: "moon.zzz") { NavigationStack { BackgroundSpikeView() } }
        }
    }
}

// MARK: - 0.3 / 0.4 / 0.5

struct CoreSpikeView: View {
    @State private var sqlcipher: String?
    @State private var inbox: String?

    var body: some View {
        List {
            Section("UniFFI (0.4)") {
                LabeledContent("Core", value: coreInfo().displayVersion)
            }
            Section("SQLCipher (0.3)") {
                Button("Run SQLCipher self-test") { runSqlcipher() }
                if let sqlcipher { Text(sqlcipher).font(.footnote.monospaced()).textSelection(.enabled) }
            }
            Section("Sealed inbox (0.5)") {
                Button("Seal a synthetic batch") { sealSynthetic() }
                Button("Open inbox (keep files)") { openInbox(delete: false) }
                Button("Open inbox and delete opened files") { openInbox(delete: true) }
                if let inbox { Text(inbox).font(.footnote.monospaced()).textSelection(.enabled) }
            }
        }
        .navigationTitle("Core")
    }

    private func runSqlcipher() {
        do {
            let r = try spikeSqlcipherSelfTest(dir: SpikePaths.path(SpikePaths.spikeDir))
            sqlcipher = """
            SQLCipher \(r.cipherVersion) · SQLite \(r.sqliteVersion)
            provider: \(r.provider) (\(r.providerVersion))
            round trip: \(r.roundTripOk) · wrong key rejected: \(r.wrongKeyRejected)
            header encrypted: \(r.headerEncrypted) · plaintext absent: \(r.plaintextAbsent)
            elapsed: \(String(format: "%.1f", r.elapsedMs)) ms
            """
        } catch { sqlcipher = "failed: \(error)" }
    }

    private func sealSynthetic() {
        do {
            _ = try SpikeVaultStore.loadOrCreate()
            let record = RecordDraft(recordType: "device.battery", sourceId: UUID().uuidString,
                                     startMs: Date.now.millis, endMs: nil,
                                     dataJson: json(["level": Double(UIDevice.current.batteryLevel), "synthetic": true]))
            inbox = "sealed → " + (try SpikeVaultStore.sealer().seal(collector: "device", records: [record]))
        } catch { inbox = "failed: \(error)" }
    }

    private func openInbox(delete: Bool) {
        do {
            let vault = try SpikeVaultStore.loadOrCreate()
            let r = try vault.openInbox(inboxDir: SpikePaths.path(SpikePaths.inbox.url), deleteOpened: delete)
            let lines = r.batches.suffix(20).map { "\($0.collector): \($0.recordCount) × \($0.recordTypes.joined(separator: ","))" }
            inbox = "opened \(r.opened) · rejected \(r.rejected) · records \(r.records)\n" + lines.joined(separator: "\n")
        } catch { inbox = "failed: \(error)" }
    }
}

// MARK: - 0.6

struct Argon2SpikeView: View {
    @State private var cells: [Argon2Benchmark.Cell] = []
    @State private var running: Task<Void, Never>?
    @State private var includeLarge = false
    @State private var copied = false

    var body: some View {
        List {
            Section {
                Toggle("Include 384 MiB (may be killed on 3 GB devices)", isOn: $includeLarge)
                if running == nil {
                    Button("Run grid (3 runs per cell)") { start() }
                } else {
                    Button("Cancel", role: .destructive) { running?.cancel(); running = nil }
                    ProgressView(value: Double(cells.count), total: Double(Argon2Benchmark(includeLarge: includeLarge).grid.count))
                }
            } footer: {
                Text("Run on battery, screen on, device cool. Release builds only.")
            }
            if !cells.isEmpty {
                Section("Results") {
                    Text(Argon2Benchmark.markdown(cells, runs: 3)).font(.caption.monospaced()).textSelection(.enabled)
                    Button(copied ? "Copied" : "Copy as Markdown") {
                        UIPasteboard.general.string = Argon2Benchmark.markdown(cells, runs: 3)
                        copied = true
                    }
                }
            }
        }
        .navigationTitle("Argon2id")
    }

    private func start() {
        cells = []
        copied = false
        let benchmark = Argon2Benchmark(includeLarge: includeLarge)
        UIApplication.shared.isIdleTimerDisabled = true
        running = Task.detached(priority: .userInitiated) {
            try? await benchmark.run { cell in await MainActor.run { cells.append(cell) } }
            await MainActor.run {
                running = nil
                UIApplication.shared.isIdleTimerDisabled = false
            }
        }
    }
}

// MARK: - 0.7

struct FaceIDSpikeView: View {
    @State private var status = ""
    @State private var enrolled = (try? SecureEnclaveSpike.load()) != nil

    var body: some View {
        List {
            Section {
                LabeledContent("Secure Enclave", value: SecureEnclaveSpike.isAvailable ? "available" : "unavailable")
                LabeledContent("Envelope", value: enrolled ? "enrolled" : "none")
            }
            Section {
                Button("1. Enable Face ID unlock (wrap vault key)") { enroll() }
                Button("2. Unlock with Face ID") { unlock() }
                Button("Remove envelope", role: .destructive) {
                    SecureEnclaveSpike.reset(); enrolled = false; status = "removed"
                }
            } header: {
                Text("Steps")
            } footer: {
                Text("To test invalidation: enroll, then change Face ID in Settings (add an appearance or reset), then unlock. Unlock must fail.")
            }
            if !status.isEmpty { Section("Result") { Text(status).font(.footnote.monospaced()) } }
        }
        .navigationTitle("Face ID")
    }

    private func enroll() {
        do {
            try SecureEnclaveSpike.enroll(vault: SpikeVaultStore.loadOrCreate())
            enrolled = true
            status = "enrolled: vault key wrapped to a Secure Enclave key (biometryCurrentSet)"
        } catch { status = "enroll failed: \(error)" }
    }

    private func unlock() {
        Task.detached {
            let start = Date.now
            let result: String
            do {
                let ok = try SecureEnclaveSpike.unlock(vault: SpikeVaultStore.loadOrCreate())
                result = ok ? "unlocked: envelope opened to the vault key" : "envelope opened but key mismatch"
            } catch { result = "unlock failed: \(error)" }
            let elapsed = Date.now.timeIntervalSince(start)
            await MainActor.run { status = result + String(format: "\n(%.2f s including prompt)", elapsed) }
        }
    }
}

// MARK: - 0.8

struct BackgroundSpikeView: View {
    @State private var events: [WakeEvent] = []
    @State private var pending = 0
    @State private var locationStatus = ""
    @State private var message = ""

    var body: some View {
        List {
            Section("Permissions") {
                LabeledContent("Location", value: locationStatus)
                Button("Request When In Use") { LocationSpike.shared.requestWhenInUse() }
                Button("Upgrade to Always") { LocationSpike.shared.requestAlways() }
                Button("Request HealthKit (steps)") {
                    Task {
                        do { try await HealthSpike.shared.requestAuthorization(); message = "HealthKit requested" }
                        catch { message = "HealthKit: \(error)" }
                    }
                }
            }
            Section("Collect") {
                LabeledContent("Pending inbox files", value: "\(pending)")
                Button("Collect health now") { Task { await HealthSpike.shared.collect(trigger: "manual"); await refresh() } }
                #if targetEnvironment(simulator)
                Button("Add synthetic steps (simulator)") {
                    Task {
                        do { try await HealthSpike.shared.addSyntheticSteps(); message = "synthetic sample saved" }
                        catch { message = "\(error)" }
                    }
                }
                #endif
                if !message.isEmpty { Text(message).font(.footnote) }
            }
            Section("Wake log (\(events.count))") {
                ShareLink("Export wake log", item: (try? SpikePaths.wakeLog) ?? URL(filePath: "/dev/null"))
                Button("Clear log", role: .destructive) { Task { await WakeLog.shared.clear(); await refresh() } }
                ForEach(events.reversed().prefix(200)) { e in
                    VStack(alignment: .leading) {
                        Text("\(e.date.formatted(date: .abbreviated, time: .standard)) · \(e.trigger)").font(.caption.bold())
                        Text("\(e.appState) · protected data: \(e.protectedDataAvailable ? "yes" : "no")").font(.caption2)
                        Text(e.outcome).font(.caption2.monospaced())
                    }
                }
            }
        }
        .navigationTitle("Background")
        .refreshable { await refresh() }
        .task {
            _ = try? SpikeVaultStore.loadOrCreate()
            LocationSpike.shared.onChange = { Task { await refresh() } }
            await refresh()
        }
    }

    private func refresh() async {
        events = await WakeLog.shared.entries()
        pending = (try? SpikePaths.inbox.pendingCount()) ?? 0
        locationStatus = switch LocationSpike.shared.authorization {
        case .authorizedAlways: "Always"
        case .authorizedWhenInUse: "When In Use"
        case .denied: "Denied"
        case .restricted: "Restricted"
        case .notDetermined: "Not determined"
        @unknown default: "Unknown"
        }
    }
}

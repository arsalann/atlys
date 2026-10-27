import AtlysCollectors
import AtlysCore
import SwiftUI
import UIKit

/// The unlocked home screen: the vault, the data in it, and the sources that
/// fill it.
struct HomeView: View {
    let model: AppModel

    @State private var showSettings = false

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Label {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Vault unlocked").font(.headline)
                            Text("Encrypted on this iPhone").font(.subheadline).foregroundStyle(.secondary)
                        }
                    } icon: {
                        Image(systemName: "lock.open.fill").foregroundStyle(.green)
                    }
                    .accessibilityIdentifier("vault-unlocked")
                    if let id = model.vaultID {
                        LabeledContent("Vault ID", value: String(id.prefix(8)))
                            .font(.footnote.monospaced())
                    }
                }
                Section {
                    NavigationLink {
                        DataBrowserView(model: model)
                    } label: {
                        LabeledContent {
                            Text(model.recordCount.formatted()).monospacedDigit()
                                .accessibilityIdentifier("record-count")
                        } label: {
                            Label("Browse your data", systemImage: "tray.full")
                        }
                    }
                    .accessibilityIdentifier("browse-data")
                    if model.isMerging || model.inbox.pendingFiles > 0 {
                        HStack(spacing: 8) {
                            if model.isMerging { ProgressView() }
                            Text(model.isMerging ? "Adding new data to the vault…"
                                 : "\(model.inbox.pendingFiles) batch(es) waiting to be added")
                                .font(.footnote).foregroundStyle(.secondary)
                        }
                    }
                    if model.inbox.quarantinedFiles > 0 {
                        Text("\(model.inbox.quarantinedFiles) batch(es) could not be read and were set aside.")
                            .font(.footnote).foregroundStyle(.orange)
                    }
                } header: {
                    Text("Your data")
                }
                SourcesSection(model: model)
            }
            .navigationTitle("Atlys")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Lock now", systemImage: "lock.fill") { model.lock() }
                        .accessibilityIdentifier("lock-now")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Settings", systemImage: "gearshape") { showSettings = true }
                        .accessibilityIdentifier("open-settings")
                }
            }
            .sheet(isPresented: $showSettings) { SettingsView(model: model) }
            .refreshable { await model.collectNow() }
            .task { await model.refreshSources() }
        }
    }
}

/// Each collector with its permission, last run, record count, and the
/// action that moves it forward.
struct SourcesSection: View {
    let model: AppModel

    private var needsSetup: Bool {
        model.sources.contains { $0.permission == .notDetermined }
    }

    var body: some View {
        Section {
            ForEach(model.sources, id: \.id) { status in
                SourceRow(model: model, status: status,
                          count: model.collectorSummaries.first { $0.collector == status.id.rawValue }?.count ?? 0)
            }
            if needsSetup {
                Button("Turn on all sources", systemImage: "checkmark.circle") {
                    Task {
                        for status in model.sources where status.permission == .notDetermined {
                            await model.requestPermission(status.id)
                        }
                    }
                }
                .accessibilityIdentifier("turn-on-all")
            }
            Button {
                Task { await model.collectNow() }
            } label: {
                HStack {
                    Label("Collect now", systemImage: "arrow.clockwise")
                    if model.isCollecting {
                        Spacer()
                        ProgressView()
                    }
                }
            }
            .disabled(model.isCollecting)
            .accessibilityIdentifier("collect-now")
        } header: {
            Text("Sources")
        } footer: {
            Text("Atlys collects everything each source makes available and keeps it only in this vault. Data deleted at a source stays here, marked as deleted.")
        }
    }
}

private struct SourceRow: View {
    let model: AppModel
    let status: CollectionCoordinator.Status
    let count: UInt64

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Label(status.id.displayName, systemImage: status.id.symbolName)
                Spacer()
                Text(count.formatted()).monospacedDigit().foregroundStyle(.secondary)
            }
            Text(summary).font(.footnote).foregroundStyle(.secondary)
            ForEach(notes, id: \.self) { note in
                Text(note).font(.footnote).foregroundStyle(.orange)
            }
            if let action {
                Button(action.title) { Task { await action.run() } }
                    .font(.footnote.bold())
                    .buttonStyle(.borderless)
                    .accessibilityIdentifier("source-action-\(status.id.rawValue)")
            }
        }
        .padding(.vertical, 2)
        .accessibilityIdentifier("source-\(status.id.rawValue)")
    }

    private var summary: String {
        var parts: [String] = []
        switch status.permission {
        case .notRequired: parts.append("On")
        case .granted: parts.append(status.id == .health ? "On (access as chosen in Health)" : "On")
        case .limited: parts.append("Only while Atlys is open")
        case .notDetermined: parts.append("Off")
        case .denied: parts.append("Not allowed in iOS Settings")
        case .unavailable: parts.append("Not available on this iPhone")
        }
        if status.isRunning {
            parts.append("collecting…")
        } else if let last = status.state.lastSuccessMs {
            parts.append("last collected \(Date(millis: last).formatted(.relative(presentation: .named)))")
        }
        return parts.joined(separator: " · ")
    }

    private var notes: [String] {
        var out: [String] = []
        if let error = status.state.lastError { out.append(error) }
        if status.id == .health, let total = status.state.int("typesTotal"),
           let done = status.state.int("typesComplete"), done < total {
            out.append("Importing history: \(done) of \(total) types done. Keep Atlys open or charging to finish sooner.")
        }
        if status.id == .motion, status.permission == .granted, let last = status.state.lastSuccessMs,
           Date().timeIntervalSince(Date(millis: last)) > 5 * 86_400 {
            out.append("iOS keeps only 7 days of motion history. Open Atlys soon to avoid a gap.")
        }
        if status.id == .location, status.permission == .granted || status.permission == .limited,
           model.detailedLocationTracking {
            out.append("Detailed tracking is on.")
        }
        return out
    }

    private var action: (title: String, run: () async -> Void)? {
        let model = self.model
        let id = status.id
        switch status.permission {
        case .notDetermined:
            return ("Turn on", { await model.requestPermission(id) })
        case .limited:
            return ("Allow Always for background collection", { await model.requestPermission(id) })
        case .denied:
            return ("Open iOS Settings", { @MainActor in
                if let url = URL(string: UIApplication.openSettingsURLString) {
                    await UIApplication.shared.open(url)
                }
            })
        default:
            return nil
        }
    }
}

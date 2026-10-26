import AtlysCore
import SwiftUI

/// The unlocked home screen. Collected data arrives in M2; for now it shows
/// the vault's state and the way to lock it.
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
                Section("Your data") {
                    Text("Health, location, motion, and device data collection arrives in the next version. Everything will stay in this vault.")
                        .foregroundStyle(.secondary)
                }
                Section {
                    Button("Lock now", systemImage: "lock.fill") { model.lock() }
                        .accessibilityIdentifier("lock-now")
                }
            }
            .navigationTitle("Atlys")
            .toolbar {
                Button("Settings", systemImage: "gearshape") { showSettings = true }
                    .accessibilityIdentifier("open-settings")
            }
            .sheet(isPresented: $showSettings) { SettingsView(model: model) }
        }
    }
}

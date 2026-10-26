import AtlysCore
import SwiftUI

/// Security settings: Face ID, auto-lock, change password, new recovery
/// phrase, delete vault.
struct SettingsView: View {
    let model: AppModel

    @Environment(\.dismiss) private var dismiss
    @State private var error: String?

    var body: some View {
        NavigationStack {
            Form {
                Section("Unlock") {
                    if model.biometricsAvailable {
                        Toggle("Unlock with \(model.biometryName)", isOn: Binding(
                            get: { model.biometricsEnrolled },
                            set: { enabled in
                                do { try model.setBiometrics(enabled: enabled) } catch { self.error = error.userMessage }
                            }
                        ))
                        .accessibilityIdentifier("biometrics-toggle")
                    }
                    Picker("Auto-lock", selection: Binding(
                        get: { model.autoLockSeconds },
                        set: { model.autoLockSeconds = $0 }
                    )) {
                        ForEach(AppModel.autoLockChoices, id: \.self) { seconds in
                            Text(autoLockLabel(seconds)).tag(seconds)
                        }
                    }
                    .accessibilityIdentifier("auto-lock-picker")
                }
                Section("Master password and recovery") {
                    NavigationLink("Change master password") { ChangePasswordView(model: model) }
                        .accessibilityIdentifier("change-password")
                    NavigationLink("New recovery phrase") { RegenerateRecoveryView(model: model) }
                        .accessibilityIdentifier("regenerate-recovery")
                }
                Section {
                    NavigationLink("Delete vault") { DeleteVaultView(model: model) }
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("delete-vault")
                } footer: {
                    Text("Deletes all vault data on this iPhone. Old device backups are not affected.")
                }
                Section("About") {
                    LabeledContent("Version", value: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?")
                    VersionFooter()
                    Text("Atlys stores your data only on this iPhone and makes no network requests.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                if let error {
                    Section { ErrorText(message: error) }
                }
            }
            .navigationTitle("Settings")
            .toolbar { Button("Done") { dismiss() }.accessibilityIdentifier("settings-done") }
            .onChange(of: model.phase) { _, phase in
                if phase != .unlocked { dismiss() }
            }
        }
    }

    private func autoLockLabel(_ seconds: Int) -> String {
        switch seconds {
        case 0: "Immediately"
        case 60: "After 1 minute"
        default: "After \(seconds / 60) minutes"
        }
    }
}

struct ChangePasswordView: View {
    let model: AppModel

    @Environment(\.dismiss) private var dismiss
    @State private var current = ""
    @State private var new = ""
    @State private var confirmation = ""
    @State private var error: String?
    @State private var done = false

    var body: some View {
        Form {
            Section {
                SecureField("Current password", text: $current)
                    .accessibilityIdentifier("current-password")
            }
            Section {
                NewPasswordField(title: "New password", password: $new, identifier: "changed-password")
                SecureField("New password again", text: $confirmation)
                    .accessibilityIdentifier("changed-password-confirm")
            } footer: {
                Text("Only the password changes. Your data is not re-encrypted.")
            }
            ErrorText(message: error)
            if done {
                Text("Password changed.").foregroundStyle(.green).accessibilityIdentifier("password-changed")
            }
            Button(model.isWorking ? "Changing…" : "Change password", action: change)
                .disabled(current.isEmpty || !passwordStrength(password: new).acceptable
                          || confirmation.isEmpty || model.isWorking)
                .accessibilityIdentifier("confirm-change-password")
        }
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .navigationTitle("Change password")
    }

    private func change() {
        guard new == confirmation else {
            error = "The new passwords don't match."
            return
        }
        error = nil
        Task {
            do {
                try await model.changePassword(current: current, new: new)
                current = ""; new = ""; confirmation = ""
                done = true
            } catch {
                self.error = error.userMessage
            }
        }
    }
}

struct RegenerateRecoveryView: View {
    let model: AppModel

    @Environment(\.dismiss) private var dismiss
    @State private var phrase: [String] = []
    @State private var error: String?
    @State private var confirmed = false

    var body: some View {
        Group {
            if confirmed {
                ContentUnavailableView("New recovery phrase saved", systemImage: "checkmark.seal",
                                       description: Text("Your old phrase no longer works."))
            } else if phrase.isEmpty {
                Form {
                    Section {
                        Text("A new recovery phrase replaces the current one. The old phrase stops working immediately.")
                    }
                    ErrorText(message: error)
                    Button("Create new recovery phrase", role: .destructive) {
                        Task {
                            do { phrase = try await model.regenerateRecovery() } catch { self.error = error.userMessage }
                        }
                    }
                    .disabled(model.isWorking)
                    .accessibilityIdentifier("confirm-regenerate")
                }
            } else {
                PhraseCheck(words: phrase) {
                    phrase = []
                    confirmed = true
                }
            }
        }
        .navigationTitle("Recovery phrase")
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct DeleteVaultView: View {
    let model: AppModel

    @State private var password = ""
    @State private var error: String?

    var body: some View {
        Form {
            Section {
                Text("This permanently deletes your vault and all its data from this iPhone. It cannot be undone. Atlys can't delete copies in old device backups.")
            }
            Section {
                SecureField("Master password", text: $password)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("delete-password")
                Button("Delete vault", role: .destructive) {
                    run { try await model.deleteVault(password: password) }
                }
                .disabled(password.isEmpty || model.isWorking)
                .accessibilityIdentifier("confirm-delete")
                if model.biometricsEnrolled {
                    Button("Delete with \(model.biometryName)", role: .destructive) {
                        run { try await model.deleteVaultWithBiometrics() }
                    }
                    .disabled(model.isWorking)
                }
            } footer: {
                Text("Confirm with your password to delete.")
            }
            ErrorText(message: error)
        }
        .navigationTitle("Delete vault")
    }

    private func run(_ action: @escaping @MainActor () async throws -> Void) {
        error = nil
        Task {
            do { try await action() } catch {
                self.error = error.userMessage
                password = ""
            }
        }
    }
}

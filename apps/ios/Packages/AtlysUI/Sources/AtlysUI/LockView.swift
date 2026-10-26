import AtlysCore
import SwiftUI

/// Unlock with the master password or Face ID; "Forgot password?" leads to
/// recovery with the 24-word phrase.
struct LockView: View {
    let model: AppModel

    @State private var password = ""
    @State private var error: String?
    @State private var showRecovery = false
    @State private var triedBiometrics = false
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                Spacer()
                Image(systemName: "lock.fill").font(.system(size: 56)).foregroundStyle(.tint)
                Text("Atlys is locked").font(.title.bold())
                SecureField("Master password", text: $password)
                    .textContentType(.password)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .focused($focused)
                    .onSubmit(unlock)
                    .padding(12)
                    .background(.quaternary, in: .rect(cornerRadius: 10))
                    .accessibilityIdentifier("unlock-password")
                ErrorText(message: error)
                if model.isWorking { ProgressView() }
                Button("Unlock", action: unlock)
                    .buttonStyle(PrimaryButtonStyle())
                    .disabled(password.isEmpty || model.isWorking)
                    .accessibilityIdentifier("unlock")
                if model.biometricsEnrolled {
                    Button("Unlock with \(model.biometryName)", systemImage: "faceid", action: unlockWithBiometrics)
                        .disabled(model.isWorking)
                        .accessibilityIdentifier("unlock-biometrics")
                }
                Spacer()
                Button("Forgot password?") { showRecovery = true }
                    .accessibilityIdentifier("forgot-password")
                VersionFooter()
            }
            .padding()
            .navigationDestination(isPresented: $showRecovery) { RecoveryView(model: model) }
        }
        .task {
            if model.biometricsEnrolled && !triedBiometrics {
                triedBiometrics = true
                unlockWithBiometrics()
            }
        }
    }

    private func unlock() {
        guard !password.isEmpty, !model.isWorking else { return }
        error = nil
        Task {
            do {
                try await model.unlock(password: password)
                password = ""
            } catch {
                self.error = error.userMessage
                password = ""
                focused = true
            }
        }
    }

    private func unlockWithBiometrics() {
        error = nil
        Task {
            do {
                try await model.unlockWithBiometrics()
            } catch {
                // Cancelling the prompt is not an error worth showing.
                if !(error is CancellationError) {
                    self.error = "\(model.biometryName) didn't unlock the vault. Use your password."
                }
            }
        }
    }
}

/// Enter the 24-word phrase and choose a new master password.
struct RecoveryView: View {
    let model: AppModel

    @State private var phraseText = ""
    @State private var password = ""
    @State private var confirmation = ""
    @State private var error: String?

    private var words: [String] {
        phraseText.lowercased().split(whereSeparator: { $0.isWhitespace || $0 == "," }).map(String.init)
    }

    private var phraseValid: Bool { words.count == 24 && isValidRecoveryPhrase(words: words) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Recover your vault").font(.title.bold())
                Text("Enter your 24-word recovery phrase in order, separated by spaces. Then choose a new master password.")
                    .foregroundStyle(.secondary)
                TextEditor(text: $phraseText)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .frame(minHeight: 140)
                    .padding(8)
                    .background(.quaternary, in: .rect(cornerRadius: 10))
                    .accessibilityIdentifier("recovery-phrase-input")
                Text(phraseStatus)
                    .font(.footnote)
                    .foregroundStyle(phraseValid ? .green : .secondary)
                NewPasswordField(title: "New master password", password: $password, identifier: "recovery-new-password")
                SecureField("New master password again", text: $confirmation)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .padding(12)
                    .background(.quaternary, in: .rect(cornerRadius: 10))
                    .accessibilityIdentifier("recovery-confirm-password")
                ErrorText(message: error)
                if model.isWorking { ProgressView() }
                Button("Recover vault", action: recover)
                    .buttonStyle(PrimaryButtonStyle())
                    .disabled(!phraseValid || !passwordStrength(password: password).acceptable
                              || confirmation.isEmpty || model.isWorking)
                    .accessibilityIdentifier("recover-vault")
            }
            .padding()
        }
        .navigationTitle("Recovery")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var phraseStatus: String {
        if phraseValid { return "Recovery phrase looks right." }
        let unknown = words.filter { !isRecoveryWord(word: $0) }
        if let first = unknown.first { return "\"\(first)\" isn't a recovery word." }
        if words.count == 24 { return "Those words don't form a valid phrase. Check their order." }
        return "\(words.count) of 24 words"
    }

    private func recover() {
        guard confirmation == password else {
            error = "The passwords don't match."
            return
        }
        error = nil
        Task {
            do {
                try await model.recover(words: words, newPassword: password)
            } catch {
                self.error = error.userMessage
            }
        }
    }
}

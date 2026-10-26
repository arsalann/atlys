import AtlysCore
import SwiftUI

/// Welcome → how it works → password → confirm → recovery phrase (show and
/// check three words) → Face ID (when available) → home.
struct OnboardingFlow: View {
    let model: AppModel

    @State private var step = Step.welcome
    @State private var password = ""
    @State private var confirmation = ""
    @State private var phrase: [String] = []
    @State private var error: String?

    private enum Step { case welcome, howItWorks, password, confirm, phrase, biometrics }

    var body: some View {
        NavigationStack {
            Group {
                switch step {
                case .welcome: welcome
                case .howItWorks: howItWorks
                case .password: passwordStep
                case .confirm: confirmStep
                case .phrase:
                    PhraseCheck(words: phrase) {
                        phrase = []
                        if model.biometricsAvailable { step = .biometrics } else { model.finishOnboarding() }
                    }
                case .biometrics: biometrics
                }
            }
            .animation(.default, value: step)
        }
    }

    private var welcome: some View {
        VStack(spacing: 20) {
            Spacer()
            Image(systemName: "lock.shield").font(.system(size: 64)).foregroundStyle(.tint)
            Text("Atlys").font(.largeTitle.bold())
            Text("Your data, on your device.").font(.title3).foregroundStyle(.secondary)
            Spacer()
            Button("Get started") { step = .howItWorks }
                .buttonStyle(PrimaryButtonStyle())
                .accessibilityIdentifier("onboarding-start")
            VersionFooter()
        }
        .padding()
    }

    private var howItWorks: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text("How Atlys works").font(.largeTitle.bold())
                card("iphone", "Your data stays on this iPhone",
                     "Atlys keeps everything in a vault on this device. It makes no network requests.")
                card("key.fill", "Encrypted with a password only you know",
                     "Your master password unlocks the vault. Atlys never stores it.")
                card("exclamationmark.triangle.fill", "Nobody can reset it",
                     "If you lose both your password and your recovery phrase, nobody can recover your data. Not even us.")
                Button("Continue") { step = .password }
                    .buttonStyle(PrimaryButtonStyle())
                    .accessibilityIdentifier("onboarding-continue")
            }
            .padding()
        }
    }

    private func card(_ symbol: String, _ title: String, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: symbol).font(.title2).foregroundStyle(.tint).frame(width: 32)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.headline)
                Text(text).foregroundStyle(.secondary)
            }
        }
    }

    private var passwordStep: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Create a master password").font(.title.bold())
                Text("Use at least \(passwordMinLength()) characters. A few unrelated words are easy to remember and hard to guess.")
                    .foregroundStyle(.secondary)
                NewPasswordField(title: "Master password", password: $password, identifier: "new-password")
                Button("Continue") { error = nil; step = .confirm }
                    .buttonStyle(PrimaryButtonStyle())
                    .disabled(!passwordStrength(password: password).acceptable)
                    .accessibilityIdentifier("password-continue")
                Button("Back") { step = .howItWorks }.frame(maxWidth: .infinity)
            }
            .padding()
        }
    }

    private var confirmStep: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Confirm your password").font(.title.bold())
                SecureField("Master password again", text: $confirmation)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .padding(12)
                    .background(.quaternary, in: .rect(cornerRadius: 10))
                    .accessibilityIdentifier("confirm-password")
                ErrorText(message: error)
                if model.isWorking {
                    HStack { ProgressView(); Text("Creating your vault…").foregroundStyle(.secondary) }
                }
                Button("Create vault") { create() }
                    .buttonStyle(PrimaryButtonStyle())
                    .disabled(confirmation.isEmpty || model.isWorking)
                    .accessibilityIdentifier("create-vault")
                Button("Back") { confirmation = ""; step = .password }
                    .frame(maxWidth: .infinity)
                    .disabled(model.isWorking)
            }
            .padding()
        }
    }

    private func create() {
        guard confirmation == password else {
            error = "The passwords don't match."
            return
        }
        error = nil
        Task {
            do {
                phrase = try await model.createVault(password: password)
                password = ""
                confirmation = ""
                step = .phrase
            } catch {
                self.error = error.userMessage
            }
        }
    }

    private var biometrics: some View {
        VStack(spacing: 20) {
            Spacer()
            Image(systemName: "faceid").font(.system(size: 64)).foregroundStyle(.tint)
            Text("Unlock with \(model.biometryName)?").font(.title.bold())
            Text("You can still use your master password at any time, and turn this off in Settings.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            ErrorText(message: error)
            Spacer()
            Button("Use \(model.biometryName)") {
                do {
                    try model.setBiometrics(enabled: true)
                    model.finishOnboarding()
                } catch {
                    self.error = error.userMessage
                }
            }
            .buttonStyle(PrimaryButtonStyle())
            .accessibilityIdentifier("enable-biometrics")
            Button("Not now") { model.finishOnboarding() }
                .accessibilityIdentifier("skip-biometrics")
        }
        .padding()
    }
}

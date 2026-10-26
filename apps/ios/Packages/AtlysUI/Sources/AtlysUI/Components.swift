import AtlysCore
import SwiftUI
import UIKit

/// A secure field plus a strength meter driven by the core's zxcvbn check.
struct NewPasswordField: View {
    let title: String
    @Binding var password: String
    let identifier: String

    private var strength: PasswordStrength { passwordStrength(password: password) }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SecureField(title, text: $password)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .padding(12)
                .background(.quaternary, in: .rect(cornerRadius: 10))
                .accessibilityIdentifier(identifier)
            if !password.isEmpty {
                StrengthMeter(strength: strength)
            }
        }
    }
}

struct StrengthMeter: View {
    let strength: PasswordStrength

    private var label: String {
        if strength.acceptable { return strength.score >= 4 ? "Strong" : "Good" }
        return strength.score <= 1 ? "Weak" : "Fair"
    }

    private var color: Color {
        strength.acceptable ? .green : (strength.score <= 1 ? .red : .orange)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 4) {
                ForEach(0..<4, id: \.self) { i in
                    Capsule()
                        .fill(i < max(Int(strength.score), 1) ? color : Color.secondary.opacity(0.25))
                        .frame(height: 5)
                }
            }
            .accessibilityHidden(true)
            Text(label).font(.footnote.bold()).foregroundStyle(color)
                .accessibilityLabel("Password strength: \(label)")
                .accessibilityIdentifier("password-strength")
            if let warning = strength.warning {
                Text(warning).font(.footnote).foregroundStyle(.secondary)
            }
            ForEach(strength.suggestions, id: \.self) { tip in
                Text(tip).font(.footnote).foregroundStyle(.secondary)
            }
        }
    }
}

/// The 24 recovery words in a numbered grid.
struct PhraseGrid: View {
    let words: [String]

    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 8) {
            ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                HStack(spacing: 6) {
                    Text("\(index + 1).")
                        .font(.footnote.monospacedDigit())
                        .foregroundStyle(.secondary)
                        .frame(width: 26, alignment: .trailing)
                    Text(word)
                        .font(.body.monospaced())
                    Spacer(minLength: 0)
                }
                .padding(.vertical, 6)
                .padding(.horizontal, 8)
                .background(.quaternary, in: .rect(cornerRadius: 8))
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Word \(index + 1): \(word)")
                .accessibilityIdentifier("recovery-word-\(index + 1)")
            }
        }
    }
}

/// Shows a recovery phrase, then asks for three random words back.
struct PhraseCheck: View {
    let words: [String]
    let onConfirmed: () -> Void

    @State private var stage = Stage.show
    @State private var picks: [Int] = []
    @State private var answers = ["", "", ""]
    @State private var mismatch = false

    private enum Stage { case show, confirm }

    var body: some View {
        switch stage {
        case .show: show
        case .confirm: confirm
        }
    }

    private var show: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Your recovery phrase").font(.title.bold())
                Text("These 24 words are the only way back in if you forget your password. Write them down on paper and keep them somewhere safe. Don't take a screenshot or store them in photos or notes.")
                    .foregroundStyle(.secondary)
                PhraseGrid(words: words)
                Button("I've written it down") {
                    picks = Array((0..<words.count).shuffled().prefix(3)).sorted()
                    answers = ["", "", ""]
                    stage = .confirm
                }
                .buttonStyle(PrimaryButtonStyle())
                .accessibilityIdentifier("phrase-written-down")
            }
            .padding()
        }
    }

    private var confirm: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Check your phrase").font(.title.bold())
                Text("Enter these words from your recovery phrase.").foregroundStyle(.secondary)
                ForEach(0..<picks.count, id: \.self) { i in
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Word \(picks[i] + 1)")
                            .font(.headline)
                            .accessibilityIdentifier("confirm-word-label-\(i)")
                        TextField("Word \(picks[i] + 1)", text: $answers[i])
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .padding(12)
                            .background(.quaternary, in: .rect(cornerRadius: 10))
                            .accessibilityIdentifier("confirm-word-field-\(i)")
                    }
                }
                if mismatch {
                    Text("Those words don't match. Check your written copy.")
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("phrase-mismatch")
                }
                Button("Confirm") {
                    let ok = picks.indices.allSatisfy { i in
                        answers[i].trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == words[picks[i]]
                    }
                    if ok { onConfirmed() } else { mismatch = true }
                }
                .buttonStyle(PrimaryButtonStyle())
                .accessibilityIdentifier("phrase-confirm")
                Button("Show the phrase again") { stage = .show; mismatch = false }
                    .frame(maxWidth: .infinity)
            }
            .padding()
        }
    }
}

struct PrimaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.headline)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .background(isEnabled ? Color.accentColor : Color.secondary.opacity(0.4), in: .rect(cornerRadius: 12))
            .foregroundStyle(.white)
            .opacity(configuration.isPressed ? 0.8 : 1)
    }
}

struct ErrorText: View {
    let message: String?

    var body: some View {
        if let message {
            Text(message)
                .font(.callout)
                .foregroundStyle(.red)
                .accessibilityIdentifier("error-message")
        }
    }
}

struct VersionFooter: View {
    var body: some View {
        Text(coreInfo().displayVersion)
            .font(.footnote.monospaced())
            .foregroundStyle(.tertiary)
            .accessibilityIdentifier("core-version")
    }
}

/// Covers the screen while the app is inactive, so the app switcher shows no
/// vault contents.
struct PrivacyCover: View {
    var body: some View {
        ZStack {
            Rectangle().fill(.regularMaterial)
            VStack(spacing: 8) {
                Image(systemName: "lock.fill").font(.largeTitle)
                Text("Atlys").font(.title2.bold())
            }
            .foregroundStyle(.secondary)
        }
        .ignoresSafeArea()
        .accessibilityIdentifier("privacy-cover")
    }
}

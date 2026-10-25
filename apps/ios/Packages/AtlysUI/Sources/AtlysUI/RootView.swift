import AtlysCore
import SwiftUI

/// Placeholder root screen until onboarding lands in M1.
public struct RootView: View {
    private let info = coreInfo()

    public init() {}

    public var body: some View {
        VStack(spacing: 12) {
            Text("Atlys")
                .font(.largeTitle.bold())
            Text("Your data, on your device.")
                .foregroundStyle(.secondary)
            Text(info.displayVersion)
                .font(.footnote.monospaced())
                .foregroundStyle(.tertiary)
                .accessibilityIdentifier("core-version")
        }
        .padding()
    }
}

#Preview {
    RootView()
}

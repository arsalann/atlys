import AtlysCore
import SwiftUI

/// Chooses the screen for the current app phase and applies the lock policy
/// on scene changes.
public struct RootView: View {
    @State private var model: AppModel
    @Environment(\.scenePhase) private var scenePhase

    public init(model: AppModel = .live()) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        ZStack {
            switch model.phase {
            case .launching:
                ProgressView()
            case .onboarding:
                OnboardingFlow(model: model)
            case .locked:
                LockView(model: model)
            case .unlocked:
                HomeView(model: model)
            case .failed(let message):
                ContentUnavailableView("Atlys can't start", systemImage: "exclamationmark.triangle",
                                       description: Text(message))
            }
            if scenePhase != .active && model.phase == .unlocked {
                PrivacyCover()
            }
        }
        .onAppear { model.start() }
        .onChange(of: scenePhase) { _, phase in
            switch phase {
            case .background: model.sceneDidEnterBackground()
            case .active: model.sceneDidBecomeActive()
            default: break
            }
        }
    }
}

import SwiftUI
import UIKit

/// Throwaway M0 spike app. See apps/ios/Spike/README.md.
@main
struct AtlysSpikeApp: App {
    @UIApplicationDelegateAdaptor(SpikeAppDelegate.self) private var delegate

    var body: some Scene {
        WindowGroup {
            SpikeRootView()
        }
    }
}

final class SpikeAppDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        // Unit tests host this app; keep launches side-effect free for them.
        guard ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] == nil else { return true }
        try? SpikePaths.prepare()
        // applicationState is still .background here even for user launches
        // (the scene is not active yet), so gate on protected data instead.
        // Background wakes only need ingest.pub, never the private keys.
        if application.isProtectedDataAvailable {
            _ = try? SpikeVaultStore.loadOrCreate()
        }
        let state = application.applicationState == .background ? "background" : "foreground"
        WakeLog.record("launch", outcome: "launched (\(state))")
        LocationSpike.shared.start(relaunchedForLocation: launchOptions?[.location] != nil)
        // HealthKit requires observer queries to be registered before
        // didFinishLaunching returns for background deliveries to arrive.
        HealthSpike.shared.registerObserver()
        return true
    }
}

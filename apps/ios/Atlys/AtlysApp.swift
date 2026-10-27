import AtlysCollectors
import AtlysUI
import SwiftUI
import UIKit

@main
struct AtlysApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    var body: some Scene {
        WindowGroup {
            RootView()
        }
    }
}

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        // HealthKit observers, the location manager, and background tasks must
        // be registered before launch finishes so background wakes reach them.
        CollectionService.shared.bootstrap()
        return true
    }
}

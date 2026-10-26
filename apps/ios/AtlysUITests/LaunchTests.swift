import XCTest

final class LaunchTests: XCTestCase {
    @MainActor
    func testFreshLaunchShowsWelcomeWithCoreVersion() {
        let app = XCUIApplication()
        app.launchArguments = ["-AtlysResetForUITests"]
        app.launch()
        XCTAssertTrue(app.buttons["onboarding-start"].waitForExistence(timeout: 10))
        let version = app.staticTexts["core-version"]
        XCTAssertTrue(version.exists)
        XCTAssertTrue(version.label.hasPrefix("core "))
    }
}

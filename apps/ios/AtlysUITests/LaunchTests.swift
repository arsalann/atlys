import XCTest

final class LaunchTests: XCTestCase {
    @MainActor
    func testLaunchShowsCoreVersion() {
        let app = XCUIApplication()
        app.launch()
        let version = app.staticTexts["core-version"]
        XCTAssertTrue(version.waitForExistence(timeout: 10))
        XCTAssertTrue(version.label.hasPrefix("core "))
    }
}

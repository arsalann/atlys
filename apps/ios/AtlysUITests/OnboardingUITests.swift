import XCTest

/// M1.5 exit criterion: onboarding completes in the Simulator. Continues
/// through lock, wrong password, unlock, and relaunch. Face ID is not
/// available in the Simulator, so its step is skipped automatically.
final class OnboardingUITests: XCTestCase {
    private let password = "synthetic plum cactus orbit"
    // Argon2id at the creation parameters takes about a second per call.
    private let slow: TimeInterval = 60

    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func testOnboardingLockUnlockAndRelaunch() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-AtlysResetForUITests"]
        app.launch()

        app.buttons["onboarding-start"].tapWhenReady()
        app.buttons["onboarding-continue"].tapWhenReady()

        let field = app.secureTextFields["new-password"]
        field.tapWhenReady()
        field.typeText("password1234")
        XCTAssertFalse(app.buttons["password-continue"].isEnabled, "weak password accepted")
        field.clear(count: 12)
        field.typeText(password)
        XCTAssertTrue(app.buttons["password-continue"].isEnabled)
        app.buttons["password-continue"].tap()

        let confirm = app.secureTextFields["confirm-password"]
        confirm.tapWhenReady()
        confirm.typeText(password)
        app.buttons["create-vault"].tap()

        // Read the phrase, then answer the three-word check.
        let cell = { (i: Int) in app.descendants(matching: .any)["recovery-word-\(i)"].firstMatch }
        XCTAssertTrue(cell(24).waitForExistence(timeout: slow))
        var words: [Int: String] = [:]
        for i in 1...24 {
            // Label format: "Word 7: orbit".
            words[i] = cell(i).label.components(separatedBy: ": ").last
        }
        XCTAssertEqual(Set(words.values).count > 1, true)
        app.buttons["phrase-written-down"].tapWhenReady()
        for i in 0..<3 {
            let label = app.staticTexts["confirm-word-label-\(i)"]
            XCTAssertTrue(label.waitForExistence(timeout: 5))
            let index = try XCTUnwrap(Int(label.label.replacingOccurrences(of: "Word ", with: "")))
            let answer = app.textFields["confirm-word-field-\(i)"]
            answer.tap()
            answer.typeText(try XCTUnwrap(words[index]))
        }
        app.buttons["phrase-confirm"].tap()

        XCTAssertTrue(app.staticTexts["Vault unlocked"].waitForExistence(timeout: 10))

        // Lock, try a wrong password, then unlock.
        app.buttons["lock-now"].tapWhenReady()
        let unlockField = app.secureTextFields["unlock-password"]
        unlockField.tapWhenReady()
        unlockField.typeText("synthetic wrong password")
        app.buttons["unlock"].tap()
        XCTAssertTrue(app.staticTexts["error-message"].waitForExistence(timeout: slow))
        unlockField.tapWhenReady()
        unlockField.typeText(password)
        app.buttons["unlock"].tap()
        XCTAssertTrue(app.staticTexts["Vault unlocked"].waitForExistence(timeout: slow))

        // A relaunch finds the existing vault and starts locked.
        app.terminate()
        app.launchArguments = []
        app.launch()
        XCTAssertTrue(app.secureTextFields["unlock-password"].waitForExistence(timeout: 10))
    }
}

extension XCUIElement {
    func tapWhenReady(timeout: TimeInterval = 10) {
        XCTAssertTrue(waitForExistence(timeout: timeout), "\(self) did not appear")
        tap()
    }

    func clear(count: Int) {
        typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: count))
    }
}

import XCTest

final class ClipboardHistoryCaptureUITests: XCTestCase {
  @MainActor private func openHistory(_ app: XCUIApplication) {
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["我的"].tap()
    let privacy = app.buttons["accountPrivacyLink"]
    for _ in 0..<6 where !privacy.isHittable { app.swipeUp() }
    XCTAssertTrue(privacy.isHittable)
    privacy.tap()
    app.buttons["privacyClipboardHistoryLink"].tap()
    XCTAssertTrue(app.navigationBars["剪贴板历史"].waitForExistence(timeout: 5))
  }

  @MainActor func testCaptureSettingsScreenshot() {
    continueAfterFailure = false
    let app = XCUIApplication(bundleIdentifier: "app.msime.ios")
    openHistory(app)
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "剪贴板历史页面"
    screenshot.lifetime = .keepAlways
    add(screenshot)
  }

  @MainActor func testAutomaticCaptureSettingPersists() {
    continueAfterFailure = false
    let app = XCUIApplication(bundleIdentifier: "app.msime.ios")
    openHistory(app)
    let toggle = app.switches["clipboardHistoryAutomaticCapture"]
    XCTAssertTrue(toggle.waitForExistence(timeout: 5))
    let original = toggle.value as? String
    // SwiftUI 的无障碍行包含标题旁的「?」，点击右侧实际开关，避免命中说明按钮。
    toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    let changed = toggle.value as? String
    XCTAssertNotEqual(changed, original)
    app.terminate()
    openHistory(app)
    XCTAssertEqual(toggle.value as? String, changed)
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "剪贴板自动采集设置"
    screenshot.lifetime = .keepAlways
    add(screenshot)
    toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    XCTAssertEqual(toggle.value as? String, original)
  }
}

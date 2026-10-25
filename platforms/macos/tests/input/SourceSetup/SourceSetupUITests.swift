import AppKit
import XCTest

// 只在一次性 CI Mac 中通过系统设置添加产品的真实韩语模式。
// 使用 runner 已配置的 XCTest 自动化权限，不修改 TCC 或输入源数据库。
final class SourceSetupUITests: XCTestCase {
    func testAddKoreanSource() throws {
        continueAfterFailure = false
        let settings = XCUIApplication(bundleIdentifier: "com.apple.systempreferences")
        settings.launch()
        let url = try XCTUnwrap(URL(string: "x-apple.systempreferences:com.apple.Keyboard-Settings.extension?InputSources"))
        XCTAssertTrue(NSWorkspace.shared.open(url))
        settings.activate()
        XCTAssertTrue(settings.windows.firstMatch.waitForExistence(timeout: 10))
        print("已打开键盘设置")

        // 某些系统版本直接打开输入源面板，另一些停在键盘页的编辑按钮。
        let edit = settings.buttons.matching(NSPredicate(format: "label == 'Edit…' OR label == 'Edit...'")).firstMatch
        if edit.waitForExistence(timeout: 3) { edit.click() }
        print("已打开输入源列表")
        let plus = settings.buttons.matching(NSPredicate(format:
            "label == '+' OR label CONTAINS[c] 'Add Input Source' OR identifier CONTAINS[c] 'addInputSource' OR label == 'Add'"
        )).firstMatch
        XCTAssertTrue(plus.waitForExistence(timeout: 5), "没有找到添加输入源按钮")
        plus.click()
        print("已打开添加输入源对话框")
        let korean = settings.staticTexts.matching(identifier: "Korean").firstMatch
        XCTAssertTrue(korean.waitForExistence(timeout: 5), "没有找到韩语分类")
        korean.click()
        let mode = settings.staticTexts.matching(identifier: "Metasequoia · KO").firstMatch
        XCTAssertTrue(mode.waitForExistence(timeout: 5), "系统设置尚未列出本次安装的韩语模式")
        mode.click()
        let add = settings.buttons.matching(identifier: "Add").firstMatch
        XCTAssertTrue(add.waitForExistence(timeout: 5) && add.isEnabled)
        add.click()
        let done = settings.buttons.matching(identifier: "Done").firstMatch
        if done.waitForExistence(timeout: 2) { done.click() }
        print("已添加韩语模式并关闭输入源面板")
        settings.terminate()
    }
}

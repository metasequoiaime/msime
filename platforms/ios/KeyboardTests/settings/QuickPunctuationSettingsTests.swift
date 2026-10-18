import XCTest
import UIKit

/// 快捷符号栏只接受单字符、去重后的有限列表，避免异常 App Group 数据把键盘菜单撑大。
final class QuickPunctuationSettingsTests: XCTestCase {
  private var stored: Any?

  override func setUp() {
    super.setUp()
    stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
  }

  override func tearDown() {
    if let stored {
      KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    } else {
      KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    }
    super.tearDown()
  }

  func testDefaultsAndCustomSymbols() {
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols,
                   KeyboardLayoutPreference.defaultQuickPunctuationSymbols)
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    XCTAssertTrue(KeyboardLayoutPreference.saveQuickPunctuationSymbols(["☆", "☆", "", "\n", "😀"], stateRoot: root))
    XCTAssertEqual(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: root)?["touch_quick_punctuation_symbols"] as? [String], ["☆", "😀"])
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols, ["☆", "😀"])
  }

  func testInvalidStoredValueFallsBackAndInputIsBounded() {
    KeyboardLayoutPreference.defaults.set(["ab", "\u{0000}"],
                                           forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols,
                   KeyboardLayoutPreference.defaultQuickPunctuationSymbols)
    let text = "①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳"
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols(from: text).count, 12)
  }
  @MainActor
  func testCustomSymbolsReachPhoneKeyAndMenu() throws {
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences()?["touch_quick_punctuation_symbols"]
    let scheme = InputSchemePreference.scheme
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_quick_punctuation_symbols"] = previous }
      InputSchemePreference.scheme = scheme
    }
    XCTAssertTrue(KeyboardLayoutPreference.saveQuickPunctuationSymbols(["☆", "=", "！"]))
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "quickPunctuationKey" } as? KeyboardKeyButton)
    XCTAssertFalse(key.isHidden)
    XCTAssertEqual(key.configuration?.title, "☆")
    XCTAssertEqual(key.menu?.children.map(\.title), ["☆", "=", "！"])
    XCTAssertNil(key.onCornerHint, "自定义菜单使用系统长按，不与固定句号手势竞争")
    controller.viewDidDisappear(false)
  }

  @MainActor
  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

}

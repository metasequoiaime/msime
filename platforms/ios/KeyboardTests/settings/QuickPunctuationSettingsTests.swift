import XCTest

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
    KeyboardLayoutPreference.quickPunctuationSymbols = ["☆", "☆", "", "\n", "😀"]
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
}

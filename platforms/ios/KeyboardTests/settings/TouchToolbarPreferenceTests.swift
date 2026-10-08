import XCTest

/// 工具栏按钮 read from the shared `touch_toolbar`.
final class TouchToolbarPreferenceTests: XCTestCase {
  func testAnUntouchedDocumentKeepsTheBarTheKeyboardAlwaysHad() {
    for preferences in [nil, [:], ["touch_toolbar": "not an object"]] as [[String: Any]?] {
      XCTAssertEqual(TouchToolbarPreference(in: preferences), TouchToolbarPreference())
    }
    let defaults = TouchToolbarPreference()
    XCTAssertTrue(defaults.layout && defaults.emoji && defaults.skin)
    XCTAssertFalse(defaults.clipboard || defaults.ai || defaults.characterSet || defaults.fullwidth || defaults.punctuation)
  }

  func testStoredSwitchesWinAndMissingOnesKeepTheirDefault() {
    let pinned = TouchToolbarPreference(in: ["touch_toolbar": [
      "skin": false, "clipboard": true, "character_set": true, "punctuation": "yes",
    ]])
    XCTAssertFalse(pinned.skin)
    XCTAssertTrue(pinned.clipboard)
    XCTAssertTrue(pinned.characterSet)
    XCTAssertTrue(pinned.layout, "absent, so on by default")
    XCTAssertFalse(pinned.punctuation, "a value that is not a Bool reads as the default")
  }

  func testTheNativePageWritesTheWholeObjectTheSharedValidatorAccepts() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-toolbar-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(TouchToolbarPreference.load(stateRoot: state), TouchToolbarPreference())

    var toolbar = TouchToolbarPreference()
    toolbar.skin = false
    toolbar.clipboard = true
    toolbar.punctuation = true
    XCTAssertTrue(TouchToolbarPreference.save(toolbar, stateRoot: state))
    XCTAssertEqual(TouchToolbarPreference.load(stateRoot: state), toolbar)
    let stored = try XCTUnwrap(
      MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["touch_toolbar"] as? [String: Any])
    XCTAssertEqual(Set(stored.keys), Set(TouchToolbarPreference.options.map(\.name)))
  }

  /// 常用语、输入方式 和 显示方式 属于本设备自己的设置，默认值与 Android 的本地设置一致：两个按钮都打开，输入时显示工具栏。
  func testTheLocalSwitchesKeepAndroidsDefaults() {
    let defaults = TouchToolbarLocalPreference.defaults
    let keys = TouchToolbarLocalPreference.keys
    let saved = keys.map { defaults.object(forKey: $0) }
    defer {
      for (key, value) in zip(keys, saved) {
        if let value { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
      }
    }
    keys.forEach(defaults.removeObject(forKey:))
    XCTAssertTrue(TouchToolbarLocalPreference.phrases)
    XCTAssertTrue(TouchToolbarLocalPreference.scheme)
    XCTAssertFalse(TouchToolbarLocalPreference.hidden)
    TouchToolbarLocalPreference.hidden = true
    TouchToolbarLocalPreference.scheme = false
    XCTAssertEqual(defaults.object(forKey: "keyboard.toolbar.hidden") as? Bool, true)
    XCTAssertEqual(defaults.object(forKey: "keyboard.toolbar.scheme") as? Bool, false)
    XCTAssertTrue(TouchToolbarLocalPreference.hidden)
    XCTAssertFalse(TouchToolbarLocalPreference.scheme)
    defaults.set("yes", forKey: TouchToolbarLocalPreference.phrasesKey)
    XCTAssertTrue(TouchToolbarLocalPreference.phrases, "a value that is not a Bool reads as the default")
  }
}

import XCTest

/// The app's 键盘布局 page saves where the keyboard reads: the shared document first, the App Group after it.
final class KeyboardGeometrySettingsTests: XCTestCase {
  func testHiddenLanguageKeyPersistsUntilLayoutReset() throws {
    KeyboardLayoutPreference.hideLanguageKey = true
    XCTAssertFalse(KeyboardLayoutPreference.geometry.showsBottomLanguage)
    KeyboardLayoutPreference.mirrorGeometry(["touch_key_spacing_tenths": 45])
    XCTAssertTrue(KeyboardLayoutPreference.hideLanguageKey, "共享几何镜像不覆盖本机布局选择")
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(KeyboardLayoutPreference.resetGeometry(stateRoot: state))
    XCTAssertFalse(KeyboardLayoutPreference.hideLanguageKey)
    XCTAssertTrue(KeyboardLayoutPreference.geometry.showsBottomLanguage)
  }

  func testSharedGeometryRejectsFractionalAndBooleanValues() {
    XCTAssertNil(KeyboardLayoutPreference.sharedKeySpacing(NSNumber(value: 45.5)))
    XCTAssertNil(KeyboardLayoutPreference.sharedRowSpacing(NSNumber(value: true)))
    XCTAssertNil(KeyboardLayoutPreference.sharedHeightAdjustment(NSNumber(value: 2.5)))
    XCTAssertEqual(KeyboardLayoutPreference.sharedKeySpacing(NSNumber(value: 45)), 4.5)
    XCTAssertEqual(KeyboardLayoutPreference.sharedRowSpacing(NSNumber(value: 70)), 7)
    XCTAssertEqual(KeyboardLayoutPreference.sharedHeightAdjustment(NSNumber(value: 20)), 20)
  }

  private var state: URL!
  private var previous: (Double, Double, Double) = (0, 0, 0)
  private var previousShuangpinKeyHints = true
  private var previousHideLanguageKey = false

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-geometry-settings-\(UUID().uuidString)", isDirectory: true)
    previous = (KeyboardLayoutPreference.keySpacing, KeyboardLayoutPreference.rowSpacing,
                KeyboardLayoutPreference.heightAdjustment)
    previousShuangpinKeyHints = KeyboardLayoutPreference.shuangpinKeyHints
    previousHideLanguageKey = KeyboardLayoutPreference.hideLanguageKey
  }

  override func tearDown() {
    KeyboardLayoutPreference.keySpacing = previous.0
    KeyboardLayoutPreference.rowSpacing = previous.1
    KeyboardLayoutPreference.heightAdjustment = previous.2
    KeyboardLayoutPreference.shuangpinKeyHints = previousShuangpinKeyHints
    KeyboardLayoutPreference.hideLanguageKey = previousHideLanguageKey
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testSavedGeometryReachesTheDocumentTheKeyboardCopiesFrom() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(
      keySpacing: 4.5, rowSpacing: 7, heightAdjustment: 20, stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_key_spacing_tenths"] as? Int, 45)
    XCTAssertEqual(document["touch_row_spacing_tenths"] as? Int, 70)
    XCTAssertEqual(document["touch_keyboard_height_adjustment"] as? Int, 20)
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 4.5)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 20)
  }

  func testResetClearsTheDocumentOverrides() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(
      keySpacing: 5, rowSpacing: 9, heightAdjustment: -8, stateRoot: state))
    let fresh = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-geometry-defaults-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: fresh) }
    _ = MetasequoiaInputSessionBridge(stateRoot: fresh)
    let defaults = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: fresh))

    XCTAssertTrue(KeyboardLayoutPreference.resetGeometry(stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    for key in ["touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment"] {
      XCTAssertEqual(document[key] as? NSObject, defaults[key] as? NSObject, key)
    }
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 0)
  }

  /// `touch_voice_shortcut` 是 Android 的工具栏语音按钮，从别的设备同步过来。iOS 工具栏不放语音按钮，App 和键盘保存、恢复几何时都不碰它，免得盖掉或删掉 Android 上的选择。
  func testGeometryNeverTouchesTheAndroidToolbarVoiceButton() throws {
    let keyboard = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["touch_voice_shortcut"] = true
    })
    func voiceShortcut() throws -> Bool? {
      try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))["touch_voice_shortcut"] as? Bool
    }

    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(keySpacing: 5, rowSpacing: 8, heightAdjustment: 6, stateRoot: state))
    XCTAssertEqual(try voiceShortcut(), true)
    XCTAssertTrue(keyboard.persistTouchKeyboardGeometry(keySpacing: 5, rowSpacing: 8, heightAdjustment: 10))
    XCTAssertEqual(try voiceShortcut(), true)
    XCTAssertTrue(KeyboardLayoutPreference.resetGeometry(stateRoot: state))
    XCTAssertEqual(try voiceShortcut(), true)
    XCTAssertTrue(keyboard.resetTouchKeyboardGeometry())
    XCTAssertEqual(try voiceShortcut(), true)
  }

  /// 用户报告：在键盘里调的高度会变回设置页的值。设置页一直停在后台，页面上留着旧高度；回来动一下别的控件，旧高度就随着四项一起写回了文档。现在设置页只写动过的那一项。
  func testKeyboardHeightSurvivesALaterSettingsSaveOfAnotherField() throws {
    var keyboard: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 0, stateRoot: state))

    XCTAssertTrue(try XCTUnwrap(keyboard).persistTouchKeyboardGeometry(
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 24))
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(keySpacing: 4.5, stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_keyboard_height_adjustment"] as? Int, 24)
    XCTAssertEqual(document["touch_key_spacing_tenths"] as? Int, 45)

    // 重新创建的会话（键盘下次出现）也从文档里读到这个高度。
    keyboard = nil
    let reloaded = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(reloaded.sharedPreferences?["touch_keyboard_height_adjustment"] as? Int, 24)
    XCTAssertEqual(reloaded.sharedPreferences?["touch_key_spacing_tenths"] as? Int, 45)
  }

  /// 组字中会话把偏好推迟到空闲时再应用；这时键盘里调的高度原先根本没写进文档，下次唤出键盘就变回去了。
  func testKeyboardHeightIsSavedWhileComposing() throws {
    let keyboard = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(keyboard.handleCharacter("n").isHandled)
    XCTAssertFalse(keyboard.handleCharacter("i").preedit.isEmpty, "the session is composing")
    XCTAssertFalse(keyboard.setTouchKeyboardGeometry(
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 18), "the live session defers while composing")

    XCTAssertTrue(keyboard.persistTouchKeyboardGeometry(
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 18))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_keyboard_height_adjustment"] as? Int, 18)
  }

  /// 键盘出现、设置页回到前台时都按文档对齐 App Group，高度也在内：键盘的布局面板从 App Group 读初值。
  func testMirroringTheDocumentCopiesHeightIntoTheAppGroup() {
    KeyboardLayoutPreference.heightAdjustment = 0
    KeyboardLayoutPreference.keySpacing = 6
    KeyboardLayoutPreference.mirrorGeometry(["touch_keyboard_height_adjustment": 30, "touch_key_spacing_tenths": 40])
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 30)
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 4)
    // 文档里没有的项不动。
    KeyboardLayoutPreference.mirrorGeometry([:])
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 30)
  }

  /// 「双拼键位提示」缺省为关：旧文档没有这个键、或者值不是布尔时都按关；保存只写这一项，文档接受了才更新 App Group 镜像。
  func testShuangpinKeyHintsDefaultOffAndAreSavedAsOneDocumentField() throws {
    XCTAssertFalse(KeyboardLayoutPreference.sharedShuangpinKeyHints(in: nil))
    XCTAssertFalse(KeyboardLayoutPreference.sharedShuangpinKeyHints(in: [:]))
    XCTAssertFalse(KeyboardLayoutPreference.sharedShuangpinKeyHints(in: ["touch_shuangpin_key_hints": "off"]))
    XCTAssertFalse(KeyboardLayoutPreference.sharedShuangpinKeyHints(in: ["touch_shuangpin_key_hints": false]))
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.shuangpinKeyHintsKey)
    XCTAssertFalse(KeyboardLayoutPreference.shuangpinKeyHints, "the App Group mirror also starts off")

    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(heightAdjustment: 12, stateRoot: state))
    XCTAssertTrue(KeyboardLayoutPreference.saveShuangpinKeyHints(false, stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_shuangpin_key_hints"] as? Bool, false)
    XCTAssertEqual(document["touch_keyboard_height_adjustment"] as? Int, 12)
    XCTAssertFalse(KeyboardLayoutPreference.shuangpinKeyHints)
  }
}

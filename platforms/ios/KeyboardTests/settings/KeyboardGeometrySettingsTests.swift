import XCTest

/// The app's 键盘布局 page saves where the keyboard reads: the shared document first, the App Group after it.
final class KeyboardGeometrySettingsTests: XCTestCase {
  func testSharedGeometryRejectsFractionalAndBooleanValues() {
    XCTAssertNil(KeyboardLayoutPreference.sharedKeySpacing(NSNumber(value: 45.5)))
    XCTAssertNil(KeyboardLayoutPreference.sharedRowSpacing(NSNumber(value: true)))
    XCTAssertNil(KeyboardLayoutPreference.sharedHeightAdjustment(NSNumber(value: 2.5)))
    XCTAssertEqual(KeyboardLayoutPreference.sharedKeySpacing(NSNumber(value: 45)), 4.5)
    XCTAssertEqual(KeyboardLayoutPreference.sharedRowSpacing(NSNumber(value: 70)), 7)
    XCTAssertEqual(KeyboardLayoutPreference.sharedHeightAdjustment(NSNumber(value: 20)), 20)
  }

  private var state: URL!
  private var previous: (Double, Double, Double, Bool) = (0, 0, 0, false)
  private var previousOrder = KeyboardLayoutPreference.NumberKeypadOrder.phone

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-geometry-settings-\(UUID().uuidString)", isDirectory: true)
    previous = (KeyboardLayoutPreference.keySpacing, KeyboardLayoutPreference.rowSpacing,
                KeyboardLayoutPreference.heightAdjustment, KeyboardLayoutPreference.voiceShortcutEnabled)
    previousOrder = KeyboardLayoutPreference.numberKeypadOrder
  }

  override func tearDown() {
    KeyboardLayoutPreference.keySpacing = previous.0
    KeyboardLayoutPreference.rowSpacing = previous.1
    KeyboardLayoutPreference.heightAdjustment = previous.2
    KeyboardLayoutPreference.voiceShortcutEnabled = previous.3
    KeyboardLayoutPreference.numberKeypadOrder = previousOrder
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testSavedGeometryReachesTheDocumentTheKeyboardCopiesFrom() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(
      keySpacing: 4.5, rowSpacing: 7, heightAdjustment: 20, voiceShortcut: true, stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_key_spacing_tenths"] as? Int, 45)
    XCTAssertEqual(document["touch_row_spacing_tenths"] as? Int, 70)
    XCTAssertEqual(document["touch_keyboard_height_adjustment"] as? Int, 20)
    XCTAssertEqual(document["touch_voice_shortcut"] as? Bool, true)
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 4.5)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 20)
  }

  func testResetClearsTheDocumentOverrides() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(
      keySpacing: 5, rowSpacing: 9, heightAdjustment: -8, voiceShortcut: false, stateRoot: state))
    let fresh = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-geometry-defaults-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: fresh) }
    _ = MetasequoiaInputSessionBridge(stateRoot: fresh)
    let defaults = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: fresh))

    XCTAssertTrue(KeyboardLayoutPreference.resetGeometry(stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    for key in ["touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment",
                "touch_voice_shortcut"] {
      XCTAssertEqual(document[key] as? NSObject, defaults[key] as? NSObject, key)
    }
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 0)
  }

  /// 用户报告：在键盘里调的高度会变回设置页的值。设置页一直停在后台，页面上留着旧高度；回来动一下别的控件，旧高度就随着四项一起写回了文档。现在设置页只写动过的那一项。
  func testKeyboardHeightSurvivesALaterSettingsSaveOfAnotherField() throws {
    var keyboard: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 0, voiceShortcut: false, stateRoot: state))

    XCTAssertTrue(try XCTUnwrap(keyboard).persistTouchKeyboardGeometry(
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 24, voiceEnabled: false))
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(keySpacing: 4.5, stateRoot: state))
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(voiceShortcut: true, stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_keyboard_height_adjustment"] as? Int, 24)
    XCTAssertEqual(document["touch_key_spacing_tenths"] as? Int, 45)
    XCTAssertEqual(document["touch_voice_shortcut"] as? Bool, true)

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
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 18, voiceEnabled: false), "the live session defers while composing")

    XCTAssertTrue(keyboard.persistTouchKeyboardGeometry(
      keySpacing: 6, rowSpacing: 7, heightAdjustment: 18, voiceEnabled: false))

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

  func testNumberKeypadOrderIsSavedAsOneDocumentField() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(heightAdjustment: 12, stateRoot: state))

    XCTAssertTrue(KeyboardLayoutPreference.saveNumberKeypadOrder(.calculator, stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_number_keypad_order"] as? String, "calculator")
    XCTAssertEqual(document["touch_keyboard_height_adjustment"] as? Int, 12)
    XCTAssertEqual(KeyboardLayoutPreference.numberKeypadOrder, .calculator)
  }
}

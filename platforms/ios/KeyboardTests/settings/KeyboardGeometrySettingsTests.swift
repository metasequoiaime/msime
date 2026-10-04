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

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-geometry-settings-\(UUID().uuidString)", isDirectory: true)
    previous = (KeyboardLayoutPreference.keySpacing, KeyboardLayoutPreference.rowSpacing,
                KeyboardLayoutPreference.heightAdjustment, KeyboardLayoutPreference.voiceShortcutEnabled)
  }

  override func tearDown() {
    KeyboardLayoutPreference.keySpacing = previous.0
    KeyboardLayoutPreference.rowSpacing = previous.1
    KeyboardLayoutPreference.heightAdjustment = previous.2
    KeyboardLayoutPreference.voiceShortcutEnabled = previous.3
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
}

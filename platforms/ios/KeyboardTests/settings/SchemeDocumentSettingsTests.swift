import XCTest

/// The app's scheme and output-form choices reach the shared document, which the keyboard copies over the App Group every time it appears.
final class SchemeDocumentSettingsTests: XCTestCase {
  private var state: URL!
  private var previousScheme = ChineseInputScheme.quanpin
  private var previousEnabled: [ChineseInputScheme] = []
  private var previousTraditional = false
  private var previousWubiProfile = "wubi86"

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-scheme-document-\(UUID().uuidString)", isDirectory: true)
    previousEnabled = InputSchemePreference.enabledSchemes
    previousScheme = InputSchemePreference.scheme
    previousTraditional = ChineseOutputPreference.usesTraditional
    previousWubiProfile = WubiProfilePreference.profile
  }

  override func tearDown() {
    InputSchemePreference.enabledSchemes = previousEnabled
    InputSchemePreference.scheme = previousScheme
    ChineseOutputPreference.usesTraditional = previousTraditional
    WubiProfilePreference.profile = previousWubiProfile
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// The keyboard's own switch wrote `touch_keyboard_schemes` first; a later choice in the app has to replace it there, not only in the App Group.
  func testAppSchemeChoiceReplacesTheKeyboardsEarlierSelection() throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setTouchKeyboardScheme(.nineKey, enabledSchemes: [.quanpin, .nineKey]))

    XCTAssertTrue(InputSchemePreference.save(scheme: .wubi, enabled: [.quanpin, .nineKey, .wubi], stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    let schemes = try XCTUnwrap(document["touch_keyboard_schemes"] as? [String: Any])
    XCTAssertEqual(schemes["selected"] as? String, ChineseInputScheme.wubi.sharedIdentifier)
    XCTAssertEqual(schemes["enabled"] as? [String], [ChineseInputScheme.quanpin, .nineKey, .wubi].map(\.sharedIdentifier))
    XCTAssertEqual(document["scheme"] as? String, "wubi")
    XCTAssertEqual(document["touch_keyboard_layout"] as? String, "twenty_six_key")
    XCTAssertEqual(InputSchemePreference.scheme, .wubi)
  }

  /// 五笔版本写进文档的 `wubi_profile`，方案名和卡片角标跟着变；之后再切方案不会把它改回去。
  func testWubiProfileReachesTheDocumentAndSurvivesSchemeChanges() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(WubiProfilePreference.save("wubi98", stateRoot: state))
    XCTAssertEqual(WubiProfilePreference.profile, "wubi98")
    XCTAssertEqual(ChineseInputScheme.wubi.title, "98 五笔")
    XCTAssertEqual(WubiProfilePreference.badge(WubiProfilePreference.profile), "98")

    XCTAssertTrue(InputSchemePreference.save(scheme: .quanpin, enabled: [.quanpin, .wubi], stateRoot: state))
    XCTAssertTrue(InputSchemePreference.save(scheme: .wubi, enabled: [.quanpin, .wubi], stateRoot: state))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["scheme"] as? String, "wubi")
    XCTAssertEqual(document["wubi_profile"] as? String, "wubi98")
    XCTAssertEqual(WubiProfilePreference.profile(in: document), "wubi98")

    XCTAssertFalse(WubiProfilePreference.save("wubi06", stateRoot: state))
    XCTAssertEqual(WubiProfilePreference.profile, "wubi98")
    XCTAssertEqual(WubiProfilePreference.profile(in: ["wubi_profile": "wubi06"]), "wubi86")
    WubiProfilePreference.mirror(["wubi_profile": "wubi86"])
    XCTAssertEqual(ChineseInputScheme.wubi.title, "86 五笔")
  }

  func testTraditionalOutputReachesTheDocument() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(ChineseOutputPreference.save(true, stateRoot: state))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["traditional_chinese_output"] as? Bool, true)
    XCTAssertTrue(ChineseOutputPreference.usesTraditional)
  }
}

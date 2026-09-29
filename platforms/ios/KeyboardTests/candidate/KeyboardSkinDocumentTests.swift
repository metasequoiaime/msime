import XCTest

/// A theme picked or a design applied in the app has to reach the shared document: the keyboard copies the document's `global_theme` and `custom_theme.keyboard` over the App Group every time it appears.
final class KeyboardSkinDocumentTests: XCTestCase {
  private var state: URL!
  private var previousTheme: String?
  private var previousDesign: Data?

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-skin-document-\(UUID().uuidString)", isDirectory: true)
    previousTheme = KeyboardFeedbackPreference.defaults.string(forKey: GlobalThemePreference.key)
    previousDesign = KeyboardFeedbackPreference.defaults.data(forKey: CustomKeyboardSkinStore.key)
  }

  override func tearDown() {
    let defaults = KeyboardFeedbackPreference.defaults
    if let previousTheme { defaults.set(previousTheme, forKey: GlobalThemePreference.key) }
    else { defaults.removeObject(forKey: GlobalThemePreference.key) }
    if let previousDesign { defaults.set(previousDesign, forKey: CustomKeyboardSkinStore.key) }
    else { defaults.removeObject(forKey: CustomKeyboardSkinStore.key) }
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testBuiltInSelectionReachesTheDocument() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(GlobalThemePreference.save("night", stateRoot: state))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["global_theme"] as? String, "night")
    // Selecting a built-in theme leaves the custom theme empty.
    XCTAssertTrue(GlobalThemePreference.customTheme(in: document).isEmpty)
    XCTAssertEqual(GlobalThemePreference.selected, "night")
    // A retired built-in skin id is not a theme; the document refuses it and nothing changes.
    XCTAssertFalse(GlobalThemePreference.save("midnight", stateRoot: state))
    XCTAssertEqual(GlobalThemePreference.selected, "night")
  }

  /// A photo design is far larger than the 16 KiB other buffers are held to; the document still takes it, and the keyboard reads back the same design.
  func testCustomDesignWithAPhotoRoundTripsThroughTheDocument() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(GlobalThemePreference.save("paper", stateRoot: state))
    var design = CustomKeyboardSkin(background: 0x203040, keyShape: .capsule)
    design.photo = Data([0xFF, 0xD8, 0xFF]) + Data(count: 40_000)
    design.photoShade = 0.4

    XCTAssertTrue(GlobalThemePreference.apply(design, stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["global_theme"] as? String, "custom")
    let custom = try XCTUnwrap(document["custom_theme"] as? [String: Any])
    // The theme on screen before the design becomes its base (THEME_CONTRACT section 5).
    XCTAssertEqual(custom["base"] as? String, "paper")
    let stored = try XCTUnwrap(custom["keyboard"] as? [String: Any])
    let decoded = try JSONDecoder().decode(
      CustomKeyboardSkin.self, from: JSONSerialization.data(withJSONObject: stored))
    XCTAssertEqual(decoded.normalized, design.normalized)
    XCTAssertEqual(GlobalThemePreference.design(in: document), design.normalized)
    XCTAssertEqual(CustomKeyboardSkinStore.current, design.normalized)
    XCTAssertEqual(GlobalThemePreference.selected, "custom")
    let theme = KeyboardTheme.resolve(document: document)
    XCTAssertEqual(theme.id, "custom")
    XCTAssertEqual(theme.design, design.normalized)
    XCTAssertEqual(theme.appearance, .light)
  }

  /// Selecting another theme keeps the custom theme's design, and a document without one clears the App Group copy.
  func testTheAppGroupFollowsTheDocument() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let design = CustomKeyboardSkin(background: 0x102030)
    XCTAssertTrue(GlobalThemePreference.apply(design, stateRoot: state))
    XCTAssertTrue(GlobalThemePreference.save("ink", stateRoot: state))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(GlobalThemePreference.design(in: document), design.normalized)
    XCTAssertEqual(KeyboardTheme.resolve(document: document).design, nil)
    XCTAssertNotNil(KeyboardFeedbackPreference.defaults.data(forKey: CustomKeyboardSkinStore.key))

    XCTAssertTrue(GlobalThemePreference.mirror(["global_theme": "system"]))
    XCTAssertEqual(GlobalThemePreference.selected, "system")
    XCTAssertNil(KeyboardFeedbackPreference.defaults.data(forKey: CustomKeyboardSkinStore.key))
    XCTAssertFalse(GlobalThemePreference.mirror(["global_theme": "system"]))
  }

  /// 恢复皮肤颜色 clears only the pickers the app edits; slots another host set (here `number`) survive the reset and keep syncing back (THEME_CONTRACT section 5).
  func testResettingCandidateColorsKeepsSlotsTheAppDoesNotEdit() throws {
    var document: [String: Any] = [
      "global_theme": "custom",
      "custom_theme": ["base": "ink", "candidate_colors": ["number": "#112233", "text": "#445566", "hover": "#778899"]],
    ]
    GlobalThemePreference.clearingCandidateColors(&document)
    let custom = GlobalThemePreference.customTheme(in: document)
    XCTAssertEqual(custom["base"] as? String, "ink")
    let colors = try XCTUnwrap(custom["candidate_colors"] as? [String: Any])
    XCTAssertEqual(colors["number"] as? String, "#112233")
    XCTAssertNil(colors["text"])
    XCTAssertNil(colors["hover"])
    XCTAssertEqual(document["global_theme"] as? String, "custom")

    // With only editable slots set, the reset leaves no empty object behind.
    var onlyEditable: [String: Any] = ["custom_theme": ["candidate_colors": ["surface": "#000000"]]]
    GlobalThemePreference.clearingCandidateColors(&onlyEditable)
    XCTAssertNil(GlobalThemePreference.customTheme(in: onlyEditable)["candidate_colors"])
  }
}

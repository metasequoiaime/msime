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
    // 比较的是选择前后，不假定自定义主题为空：用户可能设计过皮肤，早期安装的文档里也带着当时默认的薄荷晨光（#2178）。
    let before = GlobalThemePreference.customTheme(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertTrue(GlobalThemePreference.save("night", stateRoot: state))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["global_theme"] as? String, "night")
    // 选择内置主题不改动自定义主题：底色、外部皮肤、取色和键盘设计都原样保留，切回 `custom` 时还是原来那套。
    XCTAssertEqual(GlobalThemePreference.customTheme(in: document) as NSDictionary, before as NSDictionary)
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

  /// 照片不进交给 Rust 的偏好：键盘自己读的文档里照片还在，会话建得起来，词库版本的选项里没有照片，键盘写回文档时也不会把照片丢掉。
  func testThePhotoStaysInTheDocumentButNotInWhatTheEngineIsSent() throws {
    // 先建一次会话把共享文档准备好，再把照片设计写进去；下面的会话从带照片的文档建起。
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    var design = CustomKeyboardSkin(background: 0x203040)
    design.photo = Data([0xFF, 0xD8, 0xFF]) + Data(repeating: 0x5A, count: 40_000)
    design.photoShade = 0.4
    XCTAssertTrue(GlobalThemePreference.apply(design, stateRoot: state))
    let photo = try XCTUnwrap(design.photo).base64EncodedString()

    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed)
    let shared = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(GlobalThemePreference.design(in: shared), design.normalized)
    let engine = MetasequoiaInputSessionBridge.engineBound(shared)
    let keyboard = try XCTUnwrap(GlobalThemePreference.customTheme(in: engine)["keyboard"] as? [String: Any])
    XCTAssertNil(keyboard["photo"])
    XCTAssertEqual(keyboard["photoShade"] as? Double, 0.4)
    XCTAssertEqual(engine["global_theme"] as? String, "custom")
    // 原文档不受影响。
    XCTAssertNotNil(GlobalThemePreference.design(in: shared)?.photo)

    let versionOptions = String(decoding: try bridge.dictionaryStateVersionOptions(), as: UTF8.self)
    // 跳过开头：`/9j/` 在 JSON 里写成 `\/`，从后面取一段才查得出照片。
    XCTAssertFalse(versionOptions.contains(String(photo.dropFirst(8).prefix(64))))
    XCTAssertNoThrow(try bridge.localDictionaryStateVersion())

    // 会话照常改偏好、照常重建；写回文档时照片原样保留。
    XCTAssertTrue(bridge.setTraditionalChineseOutput(true))
    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    XCTAssertTrue(bridge.setTraditionalChineseOutput(false))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(GlobalThemePreference.design(in: document), design.normalized)
    XCTAssertEqual(GlobalThemePreference.design(in: bridge.sharedPreferences), design.normalized)
  }

  /// 主题缓存键里照片换成了长度加哈希，但仍要分得出同样长度的两张照片：第二张不是图片，Rust 拒绝它，不能拿第一张的缓存答案顶上。
  func testTheThemeCacheTellsApartPhotosOfTheSameLength() throws {
    ResolvedTheme.clearCacheForMemoryPressure()
    defer { ResolvedTheme.clearCacheForMemoryPressure() }
    var design = CustomKeyboardSkin(background: 0x203040)
    design.photo = Data([0xFF, 0xD8, 0xFF]) + Data(repeating: 0x11, count: 30_000)
    var custom: [String: Any] = ["keyboard": try XCTUnwrap(CustomKeyboardSkin.documentValue(design))]
    let resolved = try XCTUnwrap(ResolvedTheme.resolve(globalTheme: "custom", customTheme: custom, dark: false, skinsRoot: nil))
    XCTAssertEqual(resolved.id, "custom")
    XCTAssertEqual(ResolvedTheme.resolve(globalTheme: "custom", customTheme: custom, dark: false, skinsRoot: nil), resolved)

    var keyboard = try XCTUnwrap(custom["keyboard"] as? [String: Any])
    keyboard["photo"] = Data(repeating: 0x11, count: 30_003).base64EncodedString()
    XCTAssertEqual((keyboard["photo"] as? String)?.count, design.photo?.base64EncodedString().count)
    custom["keyboard"] = keyboard
    XCTAssertNil(ResolvedTheme.resolve(globalTheme: "custom", customTheme: custom, dark: false, skinsRoot: nil))
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

import XCTest

/// The strip's desktop candidate palette is the one client-core resolves for the global theme (a built-in theme, or the custom theme's base, package and pickers), and stays off until the iOS switch is on.
final class CandidatePaletteTests: XCTestCase {
  override func tearDown() {
    CandidatePalette.defaults.removeObject(forKey: CandidatePalette.followsDesktopKey)
    super.tearDown()
  }

  private func hex(_ color: UIColor) -> String { ThemeColor.hex(color) }

  /// What `global_theme` alone draws, as client-core resolves it.
  private func builtIn(_ id: String) throws -> CandidatePalette {
    CandidatePalette.palette(try XCTUnwrap(ResolvedTheme.resolve(globalTheme: id, customTheme: nil, dark: false)))
  }

  private func custom(_ theme: [String: Any]) -> [String: Any] {
    ["global_theme": GlobalThemeCatalog.customId, "custom_theme": theme]
  }

  /// The keyboard palette keeps the strip until the switch is on, and `system` keeps it even then: it resolves no candidate colours.
  func testFollowsTheKeyboardPaletteUntilSwitchedOn() throws {
    CandidatePalette.defaults.set(false, forKey: CandidatePalette.followsDesktopKey)
    XCTAssertNil(CandidatePalette.active(in: ["global_theme": "paper"], systemDark: false))
    CandidatePalette.defaults.set(true, forKey: CandidatePalette.followsDesktopKey)
    XCTAssertEqual(CandidatePalette.active(in: ["global_theme": "paper"], systemDark: false), try builtIn("paper"))
    XCTAssertNil(CandidatePalette.active(in: ["global_theme": "system"], systemDark: false))
    XCTAssertNil(CandidatePalette.active(in: [:], systemDark: true))
  }

  /// `candidate_theme` wins, then the app's light or dark mode (`theme`), then the system.
  func testThemeResolutionOrder() {
    XCTAssertTrue(CandidatePalette.isDark(candidateTheme: "dark", appMode: "light", systemDark: false))
    XCTAssertFalse(CandidatePalette.isDark(candidateTheme: "light", appMode: "dark", systemDark: true))
    XCTAssertTrue(CandidatePalette.isDark(candidateTheme: "follow", appMode: "dark", systemDark: false))
    XCTAssertFalse(CandidatePalette.isDark(candidateTheme: "follow", appMode: "system", systemDark: false))
    XCTAssertTrue(CandidatePalette.isDark(candidateTheme: nil, appMode: nil, systemDark: true))
  }

  /// 内置主题画设计稿的候选覆盖色（`candBg = panel`、`candSelBg = accent + '24'`）；`system` 和目录里没有的主题 id 画键盘的跟随系统调色板。
  func testBuiltInThemesAndTheSystemStrip() throws {
    let paper = try builtIn("paper")
    XCTAssertEqual(hex(paper.surface), "#F7F5F0")
    XCTAssertEqual(hex(paper.text), "#1A1E1B")
    XCTAssertEqual(hex(paper.accent), "#2C7A4B")
    XCTAssertEqual(hex(paper.selected), "#2C7A4B")
    XCTAssertEqual(paper.selected.cgColor.alpha, CGFloat(0x24) / 255, accuracy: 0.001)
    XCTAssertEqual(hex(try builtIn("night").surface), "#16262F")

    let native = CandidatePalette.resolve(["global_theme": "system"], systemDark: false)
    // `system` 画跟随系统的键盘调色板：应用主题能解析时用季节配色，否则用原生令牌。
    XCTAssertEqual(native.surface, KeyboardTheme.system.background)
    XCTAssertEqual(native.accent, KeyboardTheme.system.accent)
    XCTAssertEqual(native.selected, KeyboardTheme.system.accentSoft)
    XCTAssertEqual(CandidatePalette.resolve(["global_theme": "ocean"], systemDark: false), native)
  }

  /// A text picker sets the number colour to itself half-transparent, unless the number picker is set as well.
  func testTextPickerDerivesTheNumberColour() {
    let derived = CandidatePalette.resolve(custom(["candidate_colors": ["text": "#112233"]]), systemDark: false)
    XCTAssertEqual(hex(derived.text), "#112233")
    XCTAssertEqual(hex(derived.number), "#112233")
    XCTAssertEqual(derived.number.cgColor.alpha, CGFloat(0x9d) / 255, accuracy: 0.001)

    let explicit = CandidatePalette.resolve(
      custom(["candidate_colors": ["text": "#112233", "number": "#445566"]]), systemDark: false)
    XCTAssertEqual(hex(explicit.number), "#445566")
    XCTAssertEqual(explicit.number.cgColor.alpha, 1)
  }

  /// Pickers lie over the base; a malformed picker makes client-core refuse the whole custom theme, and the strip draws the native tokens.
  func testPickersOverTheBaseAndMalformedColours() throws {
    let picked = CandidatePalette.resolve(
      custom(["base": "paper", "candidate_colors": ["surface": "#040506"]]), systemDark: false)
    XCTAssertEqual(hex(picked.surface), "#040506")
    XCTAssertEqual(picked.text, try builtIn("paper").text)

    for bad in ["#12345", "123456", "#12345g", "#1234567"] {
      XCTAssertNil(CandidatePalette.resolveTheme(custom(["base": "paper", "candidate_colors": ["hover": bad]]),
                                                 systemDark: false), bad)
      XCTAssertEqual(CandidatePalette.resolve(custom(["base": "paper", "candidate_colors": ["hover": bad]]),
                                              systemDark: false), CandidatePalette.palette(.system), bad)
    }
  }

  /// What the App's colour picker writes reads back as the same colour.
  func testHexRoundTrip() throws {
    let color = try XCTUnwrap(ThemeColor.parse("#A0B1C2"))
    XCTAssertEqual(ThemeColor.hex(color), "#A0B1C2")
    XCTAssertEqual(ThemeColor.parse("#00FF0080")?.cgColor.alpha ?? 0, CGFloat(0x80) / 255, accuracy: 0.001)
    XCTAssertNil(ThemeColor.parse("#00ff0"))
    XCTAssertNil(ThemeColor.parse("00ff00"))
    XCTAssertNil(ThemeColor.parse("#00ff00zz"))
  }

  private func skinsRoot(_ manifests: [String: String]) throws -> URL {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: root) }
    for (id, manifest) in manifests {
      let folder = root.appendingPathComponent(id, isDirectory: true)
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      try manifest.write(to: folder.appendingPathComponent("skin.toml"), atomically: true, encoding: .utf8)
    }
    return root
  }

  private func manifest(_ id: String, base: String = "paper", layouts: String = "'horizontal', 'vertical'",
                        themes: String = "'light'", palette: String = "light", colors: String) -> String {
    """
    schema_version = 1
    id = '\(id)'
    name = 'Sample'
    version = '1.0'
    base = '\(base)'
    [supports]
    layouts = [\(layouts)]
    themes = [\(themes)]
    [candidate_window]
    [candidate_window.decoration]
    [candidate.\(palette)]
    \(colors)
    """
  }

  private func package(_ id: String, base: String = "paper", pickers: [String: String] = [:]) -> [String: Any] {
    var theme: [String: Any] = ["candidate_skin": id, "base": base]
    if !pickers.isEmpty { theme["candidate_colors"] = pickers }
    return custom(theme)
  }

  /// An imported skin extends the base its manifest names with the colours it declares; the user's own pickers still come last.
  func testImportedSkinExtendsItsBase() throws {
    let root = try skinsRoot(["sakura": manifest("sakura", colors: """
      surface = '#fff0f5'
      text = '#112233'
      border = '#ff000080'
      accent = 'pink'
      """)])
    // The custom theme's own base gives way to the package's.
    let palette = CandidatePalette.resolve(package("sakura", base: "night"), systemDark: false, skinsRoot: root)
    let base = try builtIn("paper")
    XCTAssertEqual(hex(palette.surface), "#FFF0F5")
    XCTAssertEqual(hex(palette.text), "#112233")
    XCTAssertEqual(hex(palette.border), "#FF0000")
    XCTAssertEqual(palette.border.cgColor.alpha, CGFloat(0x80) / 255, accuracy: 0.001)
    XCTAssertEqual(palette.accent, base.accent, "a colour the desktop windows would refuse is ignored")
    XCTAssertEqual(palette.selected, base.selected)

    let overridden = CandidatePalette.resolve(package("sakura", pickers: ["surface": "#010203"]),
                                              systemDark: false, skinsRoot: root)
    XCTAssertEqual(hex(overridden.surface), "#010203")
  }

  /// A package's translation colour draws the hints and translations; without one they follow the numbers, as before.
  func testImportedSkinTranslationColourIsTheSecondaryText() throws {
    let root = try skinsRoot([
      "glossed": manifest("glossed", colors: "number = '#123456'\ntranslation = '#9fb4e0'"),
      "plain": manifest("plain", colors: "number = '#123456'"),
    ])
    let glossed = CandidatePalette.resolve(package("glossed"), systemDark: false, skinsRoot: root)
    XCTAssertEqual(hex(glossed.secondary), "#9FB4E0")
    XCTAssertEqual(hex(glossed.number), "#123456")
    let plain = CandidatePalette.resolve(package("plain"), systemDark: false, skinsRoot: root)
    XCTAssertEqual(hex(plain.secondary), "#123456")
  }

  /// 皮肤包只在横排、并且只在它的底所属、清单也声明了的明暗下绘制；目录拒收的文件夹从不绘制。剩下的是底加取色器。
  func testImportedSkinOutsideWhatItSupportsFallsBack() throws {
    let root = try skinsRoot([
      "tall": manifest("tall", layouts: "'vertical'", colors: "surface = '#fff0f5'"),
      "lightonly": manifest("lightonly", colors: "surface = '#fff0f5'"),
      "darkbase": manifest("darkbase", base: "night", colors: "surface = '#fff0f5'"),
      "mismatch": manifest("other", colors: "surface = '#fff0f5'"),
    ])
    let paper = try builtIn("paper")
    XCTAssertEqual(CandidatePalette.resolve(package("tall"), systemDark: false, skinsRoot: root), paper)
    XCTAssertEqual(CandidatePalette.resolve(package("mismatch"), systemDark: false, skinsRoot: root), paper)
    // 深色底的皮肤在浅色模式下不画，自定义主题的浅色底 `paper` 照常作底。
    XCTAssertEqual(CandidatePalette.resolve(package("darkbase"), systemDark: false, skinsRoot: root), paper)
    // 浅色底的皮肤只在浅色模式下画：深色模式不画它，留下的浅色底也不属于深色模式，于是按跟随系统。
    XCTAssertEqual(CandidatePalette.resolveTheme(package("lightonly"), systemDark: false, skinsRoot: root)?.candidateSkin,
                   "lightonly")
    let dark = try XCTUnwrap(CandidatePalette.resolveTheme(package("lightonly"), systemDark: true, skinsRoot: root))
    XCTAssertNil(dark.candidateSkin)
    XCTAssertNil(dark.candidate)
  }

  /// 深色模式画 `candidate_skin_dark`，浅色模式画 `candidate_skin`；换掉深色槽位那款皮肤的文件后，深色模式不会从缓存里拿到旧颜色。
  func testEachModeDrawsItsSlotAndTheDarkPackageIsPartOfTheCacheKey() throws {
    let root = try skinsRoot([
      "sakura": manifest("sakura", colors: "surface = '#fff0f5'"),
      "dusk": manifest("dusk", base: "night", themes: "'dark'", palette: "dark", colors: "surface = '#101820'"),
    ])
    let preferences = custom(["base": "night", "candidate_skin": "sakura", "candidate_skin_dark": "dusk"])
    XCTAssertEqual(hex(CandidatePalette.resolve(preferences, systemDark: false, skinsRoot: root).surface), "#FFF0F5")
    XCTAssertEqual(hex(CandidatePalette.resolve(preferences, systemDark: true, skinsRoot: root).surface), "#101820")
    XCTAssertEqual(CandidatePalette.resolveTheme(preferences, systemDark: true, skinsRoot: root)?.candidateSkin, "dusk")

    // 清单变长，大小一定变了，不依赖修改时间的精度。
    try manifest("dusk", base: "night", themes: "'dark'", palette: "dark", colors: "surface = '#203040'\ntext = '#eeeeee'")
      .write(to: root.appendingPathComponent("dusk/skin.toml"), atomically: true, encoding: .utf8)
    XCTAssertEqual(hex(CandidatePalette.resolve(preferences, systemDark: true, skinsRoot: root).surface), "#203040")
  }

  /// 浅色槽位放浅色底的皮肤、深色槽位放深色底的皮肤时，键盘不被浅色那次解析钉成浅色：不固定明暗，深色 trait 下画深色槽位那款皮肤的底。
  func testTheKeyboardFollowsTheModeWhenTheSlotsHoldSkinsOfDifferentModes() throws {
    let root = try skinsRoot([
      "sakura": manifest("sakura", colors: "surface = '#fff0f5'"),
      "dusk": manifest("dusk", base: "night", themes: "'dark'", palette: "dark", colors: "surface = '#101820'"),
    ])
    let document = custom(["base": "night", "candidate_skin": "sakura", "candidate_skin_dark": "dusk"])
    let theme = KeyboardTheme.resolve(GlobalThemeCatalog.customId, document: document, skinsRoot: root)
    XCTAssertNil(theme.appearance)
    let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
    let paper = KeyboardTheme.resolve("paper", document: ["global_theme": "paper"], skinsRoot: root)
    let night = KeyboardTheme.resolve("night", document: ["global_theme": "night"], skinsRoot: root)
    XCTAssertEqual(paper.appearance, .light)
    XCTAssertEqual(night.appearance, .dark)
    XCTAssertEqual(hex(theme.background.resolvedColor(with: light)), hex(paper.background.resolvedColor(with: light)))
    XCTAssertEqual(hex(theme.background.resolvedColor(with: dark)), hex(night.background.resolvedColor(with: dark)))
    XCTAssertEqual(hex(theme.accent.resolvedColor(with: dark)), hex(night.accent.resolvedColor(with: dark)))
    // 键盘跟随深色时，候选栏解析出深色槽位的皮肤。
    XCTAssertEqual(CandidatePalette.resolveTheme(document, systemDark: true, skinsRoot: root)?.candidateSkin, "dusk")

    // 只设浅色皮肤：深色模式没有能画的包，跟随系统，键盘同样不固定明暗。
    let lightOnly = KeyboardTheme.resolve(GlobalThemeCatalog.customId,
                                          document: custom(["base": "paper", "candidate_skin": "sakura"]), skinsRoot: root)
    XCTAssertNil(lightOnly.appearance)
    XCTAssertEqual(hex(lightOnly.background.resolvedColor(with: light)), hex(paper.background.resolvedColor(with: light)))
    // 两种明暗画同一个固定底时照旧固定明暗。
    XCTAssertEqual(KeyboardTheme.resolve(GlobalThemeCatalog.customId, document: custom(["base": "night"]), skinsRoot: root).appearance,
                   .dark)
    XCTAssertEqual(night, KeyboardTheme.resolve("night", document: ["global_theme": "night"], skinsRoot: root))
  }

  /// 只设过一款皮肤的旧文档：深色槽位没设时深色模式也取 `candidate_skin`。
  func testDarkModeFallsBackToTheLightSlot() {
    XCTAssertEqual(GlobalThemePreference.candidateSkin(in: ["candidate_skin": "sakura", "candidate_skin_dark": "dusk"], dark: true), "dusk")
    XCTAssertEqual(GlobalThemePreference.candidateSkin(in: ["candidate_skin": "sakura", "candidate_skin_dark": "dusk"], dark: false), "sakura")
    XCTAssertEqual(GlobalThemePreference.candidateSkin(in: ["candidate_skin": "starry"], dark: true), "starry")
    XCTAssertNil(GlobalThemePreference.candidateSkin(in: ["candidate_skin_dark": "dusk"], dark: false))
    XCTAssertNil(GlobalThemePreference.candidateSkin(in: [:], dark: true))
    XCTAssertEqual(GlobalThemePreference.candidateSkins(in: ["candidate_skin": "mist", "candidate_skin_dark": "mist"]), ["mist"])
  }

  // 合成的皮肤：sakura、paper-notes 是浅色底，dusk、starry 是深色底，mist 跟随系统。与 `apps/desktop/tests/candidate/candidate-skin-slots.test.ts` 同一组用例。
  private let slots: [String: CandidateSkinSlot] = [
    "sakura": .light, "paper-notes": .light, "dusk": .dark, "starry": .dark, "mist": .both,
  ]

  private func applying(_ id: String, base: String, to custom: [String: Any]) -> [String: Any] {
    var document: [String: Any] = ["global_theme": "paper", "custom_theme": custom]
    let slots = self.slots
    GlobalThemePreference.applyingPackage(id, base: base, slotOf: { slots[$0] })(&document)
    XCTAssertEqual(document["global_theme"] as? String, GlobalThemeCatalog.customId)
    return GlobalThemePreference.customTheme(in: document)
  }

  private func removing(_ id: String, from custom: [String: Any]) -> [String: Any] {
    var document: [String: Any] = ["custom_theme": custom]
    GlobalThemePreference.removingPackage(id)(&document)
    return GlobalThemePreference.customTheme(in: document)
  }

  func testASkinBelongsToTheSlotOfItsBase() {
    XCTAssertEqual(CandidateSkinSlot(base: "paper"), .light)
    XCTAssertEqual(CandidateSkinSlot(base: "light"), .light)
    XCTAssertEqual(CandidateSkinSlot(base: "night"), .dark)
    XCTAssertEqual(CandidateSkinSlot(base: "ink"), .dark)
    XCTAssertEqual(CandidateSkinSlot(base: "shuishan"), .dark)
    XCTAssertEqual(CandidateSkinSlot(base: "system"), .both)
  }

  func testApplyingASkinFillsOnlyItsOwnSlot() {
    let light = applying("sakura", base: "paper", to: ["candidate_skin_dark": "dusk"])
    XCTAssertEqual(light["base"] as? String, "paper")
    XCTAssertEqual(light["candidate_skin"] as? String, "sakura")
    XCTAssertEqual(light["candidate_skin_dark"] as? String, "dusk")
    let dark = applying("starry", base: "night", to: light)
    XCTAssertEqual(dark["base"] as? String, "night")
    XCTAssertEqual(dark["candidate_skin"] as? String, "sakura")
    XCTAssertEqual(dark["candidate_skin_dark"] as? String, "starry")
    // `system` 底的写两个槽位；`system` 是默认底，文档里不写。
    let both = applying("mist", base: "system", to: dark)
    XCTAssertNil(both["base"])
    XCTAssertEqual(both["candidate_skin"] as? String, "mist")
    XCTAssertEqual(both["candidate_skin_dark"] as? String, "mist")
  }

  func testALegacyDarkSkinIsKeptWhenALightSkinIsApplied() {
    // 旧文档只有一款深色皮肤，存在 candidate_skin 里。
    let next = applying("sakura", base: "paper", to: ["base": "night", "candidate_skin": "starry"])
    XCTAssertEqual(next["candidate_skin"] as? String, "sakura")
    XCTAssertEqual(next["candidate_skin_dark"] as? String, "starry")
    // 原来是浅色皮肤时不挪：深色槽位保持空着。
    let replaced = applying("sakura", base: "paper", to: ["candidate_skin": "paper-notes"])
    XCTAssertEqual(replaced["candidate_skin"] as? String, "sakura")
    XCTAssertNil(replaced["candidate_skin_dark"])
    // 跟随系统的皮肤原来靠回退画在深色模式，同样挪过去。
    let system = applying("sakura", base: "paper", to: ["candidate_skin": "mist"])
    XCTAssertEqual(system["candidate_skin_dark"] as? String, "mist")
    // 不知道槽位的（包已经不在了）哪个模式都画不出来，直接覆盖，不挪进深色槽位。
    let unknown = applying("sakura", base: "paper", to: ["candidate_skin": "gone"])
    XCTAssertEqual(unknown["candidate_skin"] as? String, "sakura")
    XCTAssertNil(unknown["candidate_skin_dark"])
  }

  func testANewDarkSkinReplacesALegacyDarkSkinInTheLightSlot() {
    let next = applying("dusk", base: "night", to: ["base": "night", "candidate_skin": "starry"])
    XCTAssertNil(next["candidate_skin"])
    XCTAssertEqual(next["candidate_skin_dark"] as? String, "dusk")
  }

  func testRemovingASkinClearsOnlyTheSlotsThatHoldIt() {
    let one = removing("dusk", from: ["candidate_skin": "sakura", "candidate_skin_dark": "dusk"])
    XCTAssertEqual(one["candidate_skin"] as? String, "sakura")
    XCTAssertNil(one["candidate_skin_dark"])
    let both = removing("mist", from: ["candidate_skin": "mist", "candidate_skin_dark": "mist", "base": "ink"])
    XCTAssertNil(both["candidate_skin"])
    XCTAssertNil(both["candidate_skin_dark"])
    // 最后一款皮肤取下后底回到跟随系统，留下的固定明暗的底不会在两种明暗下都生效。
    XCTAssertNil(both["base"])
    // 没放在任何槽位里的包，取下时什么都不改。
    XCTAssertEqual(removing("dusk", from: ["base": "ink"])["base"] as? String, "ink")
  }

  /// 先用浅色皮肤再用深色皮肤，底是 night：取下浅色那款时底不动，两款都取下后底回到跟随系统。
  func testRemovingTheLastSkinDoesNotLeaveItsBaseFixingBothModes() {
    let both = applying("starry", base: "night", to: applying("sakura", base: "paper", to: [:]))
    XCTAssertEqual(both["base"] as? String, "night")
    let darkLeft = removing("sakura", from: both)
    XCTAssertEqual(darkLeft["base"] as? String, "night")
    XCTAssertEqual(darkLeft["candidate_skin_dark"] as? String, "starry")
    let none = removing("starry", from: darkLeft)
    XCTAssertNil(none["base"])
    XCTAssertTrue(GlobalThemePreference.candidateSkins(in: none).isEmpty)
  }

  /// 从别的主题开始自定义时两个槽位一起清，深色槽位的皮肤不会留在新的自定义主题里。
  func testCustomizingFromAnotherThemeClearsBothSlots() {
    var document: [String: Any] = [
      "global_theme": "paper",
      "custom_theme": ["candidate_skin": "sakura", "candidate_skin_dark": "dusk"],
    ]
    GlobalThemePreference.pickingCandidateColor("text", hex: "#123456")(&document)
    let custom = GlobalThemePreference.customTheme(in: document)
    XCTAssertNil(custom["candidate_skin"])
    XCTAssertNil(custom["candidate_skin_dark"])
    XCTAssertEqual(custom["base"] as? String, "paper")
  }

  /// The native settings app copies a folder picked in Files into the shared root through the Rust import, and can delete it again.
  func testAPickedFolderIsImportedListedAndRemoved() throws {
    let files = try skinsRoot(["sakura": manifest("sakura", colors: "surface = '#fff0f5'")])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: root) }
    let source = files.appendingPathComponent("sakura", isDirectory: true)
    XCTAssertEqual(try ExternalCandidateSkin.importFolder(source, root: root).get(), "sakura")
    let listed = try XCTUnwrap(ExternalCandidateSkin.scan(root))
    XCTAssertEqual(listed.map(\.id), ["sakura"])
    XCTAssertEqual(listed.first?.skin.name, "Sample")
    XCTAssertEqual(hex(CandidatePalette.resolve(package("sakura"), systemDark: false, skinsRoot: root).surface),
                   "#FFF0F5")

    let bare = files.appendingPathComponent("bare", isDirectory: true)
    try FileManager.default.createDirectory(at: bare, withIntermediateDirectories: true)
    XCTAssertEqual(ExternalCandidateSkin.importFolder(bare, root: root), .failure(.manifest))
    // A global theme id is not a package name.
    let theme = files.appendingPathComponent("night", isDirectory: true)
    try FileManager.default.createDirectory(at: theme, withIntermediateDirectories: true)
    try "id = 'night'".write(to: theme.appendingPathComponent("skin.toml"), atomically: true, encoding: .utf8)
    XCTAssertEqual(ExternalCandidateSkin.importFolder(theme, root: root), .failure(.name))

    XCTAssertFalse(ExternalCandidateSkin.remove("../sakura", root: root))
    XCTAssertFalse(ExternalCandidateSkin.remove(".hidden", root: root))
    XCTAssertTrue(ExternalCandidateSkin.remove("sakura", root: root))
    XCTAssertEqual(ExternalCandidateSkin.scan(root)?.count, 0)
  }
}

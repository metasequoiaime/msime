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

  /// A built-in theme draws the design's candidate override (`candBg = panel`, `candSelBg = accent + '24'`); `system` and a theme id the catalog does not list draw the keyboard's native tokens.
  func testBuiltInThemesAndTheSystemStrip() throws {
    let paper = try builtIn("paper")
    XCTAssertEqual(hex(paper.surface), "#F7F5F0")
    XCTAssertEqual(hex(paper.text), "#1A1E1B")
    XCTAssertEqual(hex(paper.accent), "#2C7A4B")
    XCTAssertEqual(hex(paper.selected), "#2C7A4B")
    XCTAssertEqual(paper.selected.cgColor.alpha, CGFloat(0x24) / 255, accuracy: 0.001)
    XCTAssertEqual(hex(try builtIn("night").surface), "#16262F")

    let native = CandidatePalette.resolve(["global_theme": "system"], systemDark: false)
    XCTAssertEqual(native.surface, NativeKeyboardTokens.background)
    XCTAssertEqual(native.accent, NativeKeyboardTokens.accent)
    XCTAssertEqual(native.selected, NativeKeyboardTokens.accentSoft)
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
                        themes: String = "'light'", colors: String) -> String {
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
    [candidate.light]
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
    let palette = CandidatePalette.resolve(package("sakura", base: "night"), systemDark: true, skinsRoot: root)
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

  /// A package is drawn only for the horizontal layout and the mode of its base, if it declares that mode; a folder the catalog rejects is never drawn. What is left is the base with the pickers.
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
    XCTAssertEqual(CandidatePalette.resolve(package("darkbase"), systemDark: false, skinsRoot: root),
                   try builtIn("night"))
    // A built-in base fixes the mode, so the system's dark mode does not hide the light palette.
    XCTAssertEqual(hex(CandidatePalette.resolve(package("lightonly"), systemDark: true, skinsRoot: root).surface),
                   "#FFF0F5")
    XCTAssertEqual(CandidatePalette.resolveTheme(package("lightonly"), systemDark: true, skinsRoot: root)?.candidateSkin,
                   "lightonly")
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

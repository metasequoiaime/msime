import XCTest
import UIKit

final class KeyboardSkinTests: XCTestCase {
  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to
  // whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  func testCuratedDesignsRemainReadableAndRoundTrip() throws {
    for (name, design) in CustomKeyboardSkin.templates {
      XCTAssertTrue(design.hasReadableText, name)
      XCTAssertEqual(design, design.normalized, name)
      XCTAssertEqual(try JSONDecoder().decode(CustomKeyboardSkin.self, from: JSONEncoder().encode(design)), design)
      XCTAssertGreaterThanOrEqual(CustomKeyboardSkin.contrast(CustomKeyboardSkin.readableText(on: design.actionBackground), design.actionBackground), 4.5, name)
    }
  }

  func testOnlyNativeTokensLeaveTheBackgroundToTheSystemBackdrop() throws {
    XCTAssertTrue(KeyboardTheme.system.drawsNativeBackground)
    let design = try XCTUnwrap(CustomKeyboardSkin.templates.first?.1)
    XCTAssertFalse(KeyboardTheme.designed(design).drawsNativeBackground)
  }

  func testSharedPreferencesDesignUsesRustCamelCaseAndSwiftDataEncoding() throws {
    let photo = Data([0x89, 0x50, 0x4E, 0x47])
    let document: [String: Any] = [
      "background": 0xE8F0EB,
      "keyBackground": 0xFFFFFF,
      "keyForeground": 0x17251D,
      "accent": 0x185C47,
      "actionBackground": 0x185C47,
      "cornerRadius": 12.0,
      "borderWidth": 0.5,
      "shadow": 0.1,
      "pattern": 3,
      "monospaced": true,
      "keyShape": "ticket",
      "keyMaterial": "paper",
      "keyOpacity": 0.8,
      "gradientEnd": 0xDDEFE9,
      "gradientHorizontal": true,
      "patternOpacity": 0.05,
      "customBorderColor": 0xB8CDBE,
      "photo": photo.base64EncodedString(),
      "photoShade": 0.25,
      "photoPosition": 0.5,
    ]
    let data = try JSONSerialization.data(withJSONObject: document)
    let decoded = try JSONDecoder().decode(CustomKeyboardSkin.self, from: data)
    XCTAssertEqual(decoded.keyShape, .ticket)
    XCTAssertEqual(decoded.keyMaterial, .paper)
    XCTAssertEqual(decoded.photo, photo)
    XCTAssertEqual(decoded.gradientEnd, 0xDDEFE9)
    XCTAssertEqual(decoded.keyOpacity, 0.8)
  }

  func testCustomSkinPersistenceValidationAndContrast() throws {
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.object(forKey: CustomKeyboardSkinStore.key)
    defer {
      if let previous { defaults.set(previous, forKey: CustomKeyboardSkinStore.key) }
      else { defaults.removeObject(forKey: CustomKeyboardSkinStore.key) }
    }
    var design = CustomKeyboardSkin()
    design.cornerRadius = 100
    design.borderWidth = -4
    design.pattern = 99
    design.keyBackground = 0x132536
    design.keyForeground = 0xFFFFFF
    design.actionBackground = 0xFFFFFF
    CustomKeyboardSkinStore.save(design)
    let stored = CustomKeyboardSkinStore.current
    XCTAssertEqual(stored.cornerRadius, 20)
    XCTAssertEqual(stored.borderWidth, 0)
    XCTAssertEqual(stored.pattern, 0)
    XCTAssertEqual(stored.keyBackground, 0x132536)
    XCTAssertEqual(CustomKeyboardSkin.rgb(KeyboardTheme.designed(stored).actionForeground), 0)
    XCTAssertEqual(CustomKeyboardSkin.rgb(CustomKeyboardSkin.color(0x123456)), 0x123456)
    defaults.set(Data("invalid".utf8), forKey: CustomKeyboardSkinStore.key)
    XCTAssertEqual(CustomKeyboardSkinStore.current, CustomKeyboardSkin())
    XCTAssertTrue(CustomKeyboardSkinStore.current.hasReadableText)
    for background: UInt32 in [0, 0x808080, 0xFFFFFF, 0xFF8800] {
      XCTAssertGreaterThanOrEqual(CustomKeyboardSkin.contrast(CustomKeyboardSkin.readableText(on: background), background), 4.5)
    }
  }
  func testLegacyDesignsAndLibraryRoundTrip() throws {
    let legacy = Data(#"{"background":15266027,"keyBackground":16777215,"keyForeground":1516829,"accent":1596487,"actionBackground":1596487,"cornerRadius":8,"borderWidth":0,"shadow":0,"pattern":0,"monospaced":false}"#.utf8)
    let old = try JSONDecoder().decode(CustomKeyboardSkin.self, from: legacy)
    XCTAssertNil(old.gradientEnd)
    XCTAssertNil(old.photo)
    let previous = CustomSkinLibrary.designs
    defer { CustomSkinLibrary.save(previous) }
    var design = CustomKeyboardSkin.templates[2].1
    design.photoShade = .infinity
    design.photoPosition = -4
    design.photo = Data(repeating: 0, count: 512_001)
    design.patternOpacity = 2
    let item = SavedKeyboardSkin(name: "  我的夜色  ", design: design)
    CustomSkinLibrary.save([item])
    let restored = try XCTUnwrap(CustomSkinLibrary.designs.first)
    XCTAssertEqual(restored.id, item.id)
    XCTAssertEqual(restored.name, "我的夜色")
    XCTAssertEqual(restored.design.gradientEnd, design.gradientEnd)
    XCTAssertEqual(restored.design.photoShade, 0.25)
    XCTAssertEqual(restored.design.photoPosition, 0)
    XCTAssertNil(restored.design.photo)
    XCTAssertEqual(restored.design.patternOpacity, 0.5)
    CustomSkinLibrary.save([])
    XCTAssertTrue(CustomSkinLibrary.designs.isEmpty)
  }

  func testCustomSkinLibraryRejectsMoreThanMaximumItems() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let directory = root.appendingPathComponent("CustomSkins", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let items = (0..<13).map { _ in SavedKeyboardSkin(name: "合成", design: CustomKeyboardSkin()) }
    try JSONEncoder().encode(items).write(to: directory.appendingPathComponent("library.json"))
    XCTAssertTrue(CustomSkinLibrary.designs(in: root).isEmpty)
  }

  func testCustomSkinLibraryRejectsASymlinkedDirectory() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let outside = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outside)
    }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    let linked = root.appendingPathComponent("linked", isDirectory: true)
    try FileManager.default.createSymbolicLink(at: linked, withDestinationURL: outside)

    XCTAssertFalse(CustomSkinLibrary.save([], in: linked))
    XCTAssertTrue(try FileManager.default.contentsOfDirectory(at: outside, includingPropertiesForKeys: nil).isEmpty)
  }

  @MainActor
  func testGradientAndPhotoRenderInKeyboardBackdrop() throws {
    let view = KeyboardSkinBackgroundView(frame: CGRect(x: 0, y: 0, width: 320, height: 260))
    func render(_ design: CustomKeyboardSkin) -> UIImage {
      view.skin = .designed(design)
      return UIGraphicsImageRenderer(bounds: view.bounds).image { view.layer.render(in: $0.cgContext) }
    }
    var design = CustomKeyboardSkin()
    let plain = render(design)
    design.gradientEnd = 0x224466
    let gradient = render(design)
    XCTAssertNotEqual(plain.pngData(), gradient.pngData())
    let photo = UIGraphicsImageRenderer(size: CGSize(width: 100, height: 100)).image { context in
      UIColor.red.setFill(); context.fill(CGRect(x: 0, y: 0, width: 100, height: 100))
    }
    design.photo = try XCTUnwrap(photo.jpegData(compressionQuality: 0.8))
    let wallpaper = render(design)
    XCTAssertNotEqual(gradient.pngData(), wallpaper.pngData())
    design.photoShade = 0.8
    XCTAssertNotEqual(wallpaper.pngData(), render(design).pngData())
    design.photo = nil
    design.photoShade = nil
    XCTAssertEqual(gradient.pngData(), render(design).pngData())
  }

  @MainActor
  func testPhotoImportBoundsAndSavedKeyboardSelection() throws {
    let library = CustomSkinLibrary.designs
    defer { CustomSkinLibrary.save(library) }
    let format = UIGraphicsImageRendererFormat(); format.scale = 1
    let original = UIGraphicsImageRenderer(size: CGSize(width: 2400, height: 800), format: format).image {
      UIColor.blue.setFill(); $0.fill(CGRect(x: 0, y: 0, width: 2400, height: 800))
    }
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".png")
    defer { try? FileManager.default.removeItem(at: url) }
    try XCTUnwrap(original.pngData()).write(to: url)
    let data = try XCTUnwrap(SkinPhotoData.thumbnail(at: url))
    let image = try XCTUnwrap(UIImage(data: data))
    XCTAssertLessThanOrEqual(max(image.size.width, image.size.height), 1024)
    XCTAssertLessThanOrEqual(data.count, 512_000)
    try Data("not an image".utf8).write(to: url)
    XCTAssertNil(SkinPhotoData.thumbnail(at: url))
    let decoded = try XCTUnwrap(SkinPhotoData.image(from: try XCTUnwrap(original.pngData()), maxPixelSize: 600))
    XCTAssertEqual(max(decoded.size.width, decoded.size.height), 600, accuracy: 1)
    XCTAssertNil(SkinPhotoData.image(from: Data("not an image".utf8)))
    var design = CustomKeyboardSkin.templates[2].1
    design.photo = data; design.keyOpacity = 0.45
    let item = SavedKeyboardSkin(name: "照片夜色", design: design)
    CustomSkinLibrary.save([item])
    var chosen: CustomKeyboardSkin?
    let picker = KeyboardSkinPickerView(selected: GlobalThemeCatalog.systemId, document: [:], onSelect: { _ in },
                                        onSelectDesign: { chosen = $0 }, onClose: {})
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    let button = try XCTUnwrap(descendants(picker).first { $0.accessibilityIdentifier == "savedSkinCard-" + item.id.uuidString } as? UIButton)
    button.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(chosen, design.normalized)
    XCTAssertEqual(KeyboardTheme.designed(design).keyBackground.cgColor.alpha, 0.45, accuracy: 0.001)
  }

  func testPhotoImportRejectsAnOversizedSourceBeforeImageIO() throws {
    let file = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: file) }
    try Data(repeating: 0x5A, count: SkinPhotoData.maximumSourceBytes + 1).write(to: file)

    XCTAssertNil(SkinPhotoData.sourceData(at: file))
  }

  private func luminance(_ color: UIColor, style: UIUserInterfaceStyle) -> Double {
    let resolved = color.resolvedColor(with: UITraitCollection(userInterfaceStyle: style))
    var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
    resolved.getRed(&r, green: &g, blue: &b, alpha: &a)
    func linear(_ value: CGFloat) -> Double {
      let v = Double(value)
      return v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4)
    }
    return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
  }

  @MainActor
  func testChangingDesignUpdatesActualKeyboardKeys() throws {
    preserveSharedTheme()
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    let design = CustomKeyboardSkin.templates[1].1
    for id in GlobalThemeCatalog.ids {
      if id == GlobalThemeCatalog.customId {
        XCTAssertTrue(GlobalThemePreference.apply(design))
      } else {
        XCTAssertTrue(GlobalThemePreference.save(id), id)
      }
      controller.viewWillAppear(false)
      controller.view.layoutIfNeeded()
      let skin = KeyboardTheme.current
      XCTAssertEqual(skin.id, id)
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "returnKey" } as? UIButton)
      if let design = skin.design {
        let surface = try XCTUnwrap(key.configuration?.background.customView as? SkinKeySurfaceView)
        XCTAssertEqual(surface.design, design)
        XCTAssertEqual(surface.fillColor, skin.actionBackground)
        continue
      }
      XCTAssertEqual(key.configuration?.background.cornerRadius, skin.cornerRadius, id)
      XCTAssertEqual(key.configuration?.background.strokeWidth, skin.borderWidth, id)
      XCTAssertEqual(key.layer.shadowOpacity, skin.hasShadow ? 1 : 0, id)
      // At rest return is a function key; it only takes the accent while it commits a composition (dc.html L2217).
      XCTAssertEqual(key.configuration?.background.backgroundColor?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)),
        skin.functionKeyBackground.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), id)
      let shift = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "shiftButton" } as? UIButton)
      XCTAssertEqual(shift.configuration?.background.backgroundColor?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)),
        skin.functionKeyBackground.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), id)
    }
  }

  @MainActor
  func testReturnTakesTheAccentOnlyWhileComposing() throws {
    preserveSharedTheme()
    let scheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = scheme }
    InputSchemePreference.scheme = .quanpin
    XCTAssertTrue(GlobalThemePreference.save("paper"))
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    func button(_ id: String) throws -> UIButton {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == id } as? UIButton, id)
    }
    let light = UITraitCollection(userInterfaceStyle: .light)
    let skin = KeyboardTheme.current
    func fill() throws -> UIColor? { try button("returnKey").configuration?.background.backgroundColor?.resolvedColor(with: light) }
    XCTAssertEqual(try fill(), skin.functionKeyBackground.resolvedColor(with: light))
    for character in "nihao" {
      let key = try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityLabel == "字母 \(String(character).uppercased())"
      } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("returnKey").accessibilityLabel, "确认")
    XCTAssertEqual(try fill(), skin.actionBackground.resolvedColor(with: light))
    XCTAssertEqual(try button("returnKey").configuration?.baseForegroundColor?.resolvedColor(with: light),
      skin.actionForeground.resolvedColor(with: light))
    // The first candidate is the selected one: the accent, no chip.
    let first = try button("candidate-1")
    XCTAssertEqual(first.configuration?.baseForegroundColor?.resolvedColor(with: light), skin.accent.resolvedColor(with: light))
    XCTAssertEqual(first.configuration?.background.backgroundColor?.cgColor.alpha ?? 0, 0)
    XCTAssertEqual(first.configuration?.background.strokeWidth, 0)
    XCTAssertEqual(first.layer.shadowOpacity, 0)
    XCTAssertEqual(try button("candidate-2").configuration?.baseForegroundColor?.resolvedColor(with: light),
      skin.keyForeground.resolvedColor(with: light))
    // Committing ends the composition and hands return back to the function fill and the field's own label.
    try button("returnKey").sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotEqual(try button("returnKey").accessibilityLabel, "确认")
    XCTAssertEqual(try fill(), skin.functionKeyBackground.resolvedColor(with: light))
  }

  /// Hints, glosses and markers on the keyboard palette take the theme's `secondary`, in the strip and in the expanded panel alike, so they match the 候选栏 preview (THEME_CONTRACT: hints and numbers = `secondary`).
  @MainActor
  func testCandidateAnnotationsUseTheThemeSecondary() throws {
    preserveSharedTheme()
    let scheme = InputSchemePreference.scheme
    let enabledSchemes = InputSchemePreference.enabledSchemes
    let wubiHint = WubiCodeHintPreference.isEnabled
    let followsDesktop = CandidatePalette.defaults.object(forKey: CandidatePalette.followsDesktopKey)
    defer {
      InputSchemePreference.enabledSchemes = enabledSchemes
      InputSchemePreference.scheme = scheme
      WubiCodeHintPreference.isEnabled = wubiHint
      CandidatePalette.defaults.set(followsDesktop, forKey: CandidatePalette.followsDesktopKey)
    }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .wubi
    WubiCodeHintPreference.isEnabled = true
    CandidatePalette.defaults.set(false, forKey: CandidatePalette.followsDesktopKey)
    let light = UITraitCollection(userInterfaceStyle: .light)
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    func annotationColors(_ button: UIButton) -> [UIColor] {
      guard let title = button.configuration?.attributedTitle else { return [] }
      return title.runs.compactMap { $0.uiKit.foregroundColor?.resolvedColor(with: light) }
    }
    let themes = GlobalThemeCatalog.ids.filter { $0 != GlobalThemeCatalog.systemId && $0 != GlobalThemeCatalog.customId }
    XCTAssertFalse(themes.isEmpty)
    for id in themes {
      XCTAssertTrue(GlobalThemePreference.save(id), id)
      let secondary = KeyboardTheme.current.secondary.resolvedColor(with: light)
      XCTAssertNotEqual(secondary, KeyboardTheme.current.keyForeground.withAlphaComponent(0.55).resolvedColor(with: light), id)

      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
      controller.viewWillAppear(false)
      controller.view.layoutIfNeeded()
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 G" } as? UIButton, id)
      key.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let hinted = descendants(controller.view).compactMap { $0 as? UIButton }
        .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && ($0.accessibilityLabel ?? "").contains("还需输入") }
      XCTAssertFalse(hinted.isEmpty, id)
      for button in hinted {
        let colors = annotationColors(button)
        XCTAssertFalse(colors.isEmpty, id)
        XCTAssertTrue(colors.allSatisfy { $0 == secondary }, "\(id): \(colors)")
      }

      let panel = KeyboardCandidatePanelView(
        candidates: ["你好"], preedit: "nihao",
        annotations: [KeyboardCandidateAnnotation(text: "hello", accessibilityDescription: "释义")],
        display: { $0 }, menuElements: { _ in [] }, onSelect: { _ in }, onClose: {})
      panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
      panel.layoutIfNeeded()
      let glossed = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-1" } as? UIButton, id)
      let colors = annotationColors(glossed)
      XCTAssertFalse(colors.isEmpty, id)
      XCTAssertTrue(colors.allSatisfy { $0 == secondary }, "\(id): \(colors)")
    }
  }

  @MainActor
  func testThemedKeySurfacesReachRealKeyboardWithoutChangingLayout() throws {
    preserveSharedTheme()
    let scheme = InputSchemePreference.scheme
    let enabledSchemes = InputSchemePreference.enabledSchemes
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .quanpin
    defer {
      InputSchemePreference.enabledSchemes = enabledSchemes
      InputSchemePreference.scheme = scheme
    }
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    // Compare the same letter-key layout even after UI tests select nine keys. The nine-key
    // sidebar deliberately uses flat punctuation buttons without individual skin surfaces.
    var referenceFrames: [CGRect]?
    for (name, template) in CustomKeyboardSkin.templates.prefix(4) {
      var design = template
      design.monospaced = false // Typography is independent of key geometry.
      XCTAssertTrue(GlobalThemePreference.apply(design), name)
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      func visible(_ view: UIView) -> Bool {
        if view.isHidden { return false }
        return view.superview.map(visible) ?? true
      }
      let keys = descendants(controller.view).compactMap { $0 as? KeyboardKeyButton }.filter(visible)
      XCTAssertGreaterThan(keys.count, 20)
      let frames = keys.map { $0.convert($0.bounds, to: controller.view) }
      if let referenceFrames { XCTAssertEqual(frames, referenceFrames) } else { referenceFrames = frames }
      for key in keys {
        let surface = try XCTUnwrap(key.configuration?.background.customView as? SkinKeySurfaceView)
        XCTAssertEqual(surface.design.keyShape, design.keyShape)
        XCTAssertEqual(surface.design.keyMaterial, design.keyMaterial)
      }
      let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { controller.view.layer.render(in: $0.cgContext) }
      let attachment = XCTAttachment(image: image)
      attachment.name = "实际键盘-" + name; attachment.lifetime = .keepAlways; add(attachment)
    }
  }

  /// Every theme but a design's custom one, in both modes: key labels on keys and function keys, the return key's label on the accent, and the selected candidate (the accent, no fill) on the keyboard.
  ///
  /// The selected candidate is held to 3:1 rather than 4.5:1. It is drawn semibold, and it is the design's own pairing: the native light accent `#2C7A4B` on `#D1D4DB` measures 3.5:1 and paper's `#2C7A4B` on `#E6E1D5` 4.0:1 (dc.html L1557-1559, client-core's built-in palettes). The floor still catches a theme that loses the selection outright.
  func testSkinTextContrastInBothAppearances() {
    for id in GlobalThemeCatalog.ids {
      let skin = KeyboardTheme.resolve(id, document: [:])
      for style in [UIUserInterfaceStyle.light, .dark] {
        for (name, foreground, background, floor) in [("key", skin.keyForeground, skin.keyBackground, 4.5),
          ("function", skin.keyForeground, skin.functionKeyBackground, 4.5),
          ("action", skin.actionForeground, skin.actionBackground, 4.5), ("accent", skin.accent, skin.background, 3)] {
          let a = luminance(foreground, style: style), b = luminance(background, style: style)
          XCTAssertGreaterThanOrEqual((max(a, b) + 0.05) / (min(a, b) + 0.05), floor, "\(id) \(style.rawValue) \(name)")
        }
      }
    }
  }

  /// A switched-on tile is drawn from the platform tokens in every theme (dc.html `tileOn`: `k.accentSoft` / `k.accentText`), the same on every host; only a keyboard design tints it with its own accent.
  func testSwitchedOnTileUsesThePlatformAccentInEveryTheme() {
    // The built-in themes carry their own accent (ink's is white); those are the ones a theme-derived fill used to tint.
    XCTAssertTrue(GlobalThemeCatalog.ids.contains { KeyboardTheme.resolve($0, document: [:]).palette != nil })
    for id in GlobalThemeCatalog.ids {
      let skin = KeyboardTheme.resolve(id, document: [:])
      guard skin.design == nil else { continue }
      for style in [UIUserInterfaceStyle.light, .dark] {
        let traits = UITraitCollection(userInterfaceStyle: style)
        XCTAssertEqual(skin.toggleBackground.resolvedColor(with: traits), NativeKeyboardTokens.accentSoft.resolvedColor(with: traits), "\(id) \(style.rawValue)")
        XCTAssertEqual(skin.toggleForeground.resolvedColor(with: traits), NativeKeyboardTokens.accent.resolvedColor(with: traits), "\(id) \(style.rawValue)")
      }
    }
    var design = CustomKeyboardSkin()
    design.accent = 0xD4BBFF
    let designed = KeyboardTheme.designed(design)
    XCTAssertEqual(packed(designed.toggleForeground), 0xD4BBFF)
    XCTAssertEqual(packed(designed.toggleBackground), 0xD4BBFF)
  }

  private func packed(_ color: UIColor) -> UInt32 {
    var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
    color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
    let channel = { (value: CGFloat) in UInt32((value * 255).rounded()) }
    return channel(red) << 16 | channel(green) << 8 | channel(blue)
  }

  func testSchemePickerAccentStaysReadableInBothAppearances() {
    // The picker used to carry its own orange, which measured 2.6:1 against white in light mode
    // and 2.1:1 in dark -- below the large-text floor. Both shades of forest are on opposite sides
    // of that line, so the foreground has to follow the appearance rather than be picked once.
    for style in [UIUserInterfaceStyle.light, .dark] {
      let traits = UITraitCollection(userInterfaceStyle: style)
      let background = packed(MetasequoiaTheme.forestUIColor.resolvedColor(with: traits))
      let foreground = packed(MetasequoiaTheme.onForestUIColor.resolvedColor(with: traits))
      XCTAssertGreaterThanOrEqual(CustomKeyboardSkin.contrast(foreground, background), 4.5,
                                  "\(style) 下选中态文字对比度不足")
    }
  }
}

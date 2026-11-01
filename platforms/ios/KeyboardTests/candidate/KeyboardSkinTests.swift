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

  /// 只有经典兜底（没有可取季节的应用主题）把背景交给系统底板；跟随系统的季节配色和设计都画自己的背景。
  func testOnlyNativeTokensLeaveTheBackgroundToTheSystemBackdrop() throws {
    XCTAssertEqual(KeyboardTheme.system.drawsNativeBackground, !SeasonKeyboardTokens.isAvailable)
    XCTAssertEqual(KeyboardTheme.system.palette == nil, !SeasonKeyboardTokens.isAvailable)
    let design = try XCTUnwrap(CustomKeyboardSkin.templates.first?.1)
    XCTAssertFalse(KeyboardTheme.designed(design).drawsNativeBackground)
    XCTAssertNil(KeyboardTheme.designed(design).palette)
  }

  /// 「原生」不跟季节：背景交给系统底板，回车、候选和标识使用柔和水杉绿。
  func testNativeDrawsTheSystemKeyboardAndLeavesTheBackgroundToTheSystemBackdrop() throws {
    selectAppTheme("qiushan")
    XCTAssertTrue(GlobalThemeCatalog.contains(GlobalThemeCatalog.nativeId))
    XCTAssertEqual(GlobalThemeCatalog.title(GlobalThemeCatalog.nativeId), "原生")
    XCTAssertEqual(Array(GlobalThemeCatalog.ids.prefix(2)), [GlobalThemeCatalog.systemId, GlobalThemeCatalog.nativeId])
    XCTAssertFalse(GlobalThemeCatalog.isBase(GlobalThemeCatalog.nativeId))
    let skin = KeyboardTheme.resolve(GlobalThemeCatalog.nativeId, document: [:])
    XCTAssertEqual(skin.id, GlobalThemeCatalog.nativeId)
    XCTAssertTrue(skin.isNative)
    XCTAssertNil(skin.palette)
    XCTAssertNil(skin.appearance)
    XCTAssertTrue(skin.drawsNativeBackground)

    let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
    func hex(_ color: UIColor, _ traits: UITraitCollection) -> UInt32 { packed(color.resolvedColor(with: traits)) }
    XCTAssertEqual(hex(skin.accent, light), 0x32664B)
    XCTAssertEqual(hex(skin.accent, dark), 0x8AC4A2)
    XCTAssertEqual(hex(skin.actionBackground, light), 0x32664B)
    XCTAssertEqual(hex(skin.actionBackground, dark), 0x8AC4A2)
    XCTAssertEqual(hex(skin.actionForeground, light), 0xFFFFFF)
    XCTAssertEqual(hex(skin.actionForeground, dark), 0x000000)
    XCTAssertEqual(hex(skin.keyForeground, light), 0x000000)
    XCTAssertEqual(skin.accentSoft, SystemKeyboardTokens.accentSoft)
    // 工具栏 logo 同样使用水杉绿，白色描边仍需清楚。
    for (traits, percent, card) in [(light, 14.0, UIColor.white), (dark, 22.0, UIColor(red: 0x1C / 255, green: 0x1C / 255, blue: 0x1E / 255, alpha: 1))] {
      let green = skin.accent.resolvedColor(with: traits)
      XCTAssertGreaterThanOrEqual(CustomKeyboardSkin.contrast(hex(skin.actionForeground, traits), hex(skin.actionBackground, traits)), 4.5)
      XCTAssertGreaterThanOrEqual(CustomKeyboardSkin.contrast(hex(skin.accent, traits), hex(skin.background, traits)), 4.5)
      XCTAssertEqual(hex(skin.logoCircle, traits), packed(AppThemePalette.mix(green, percent, card)))
      XCTAssertEqual(hex(skin.logoMark, traits), packed(AppThemePalette.mix(green, traits.userInterfaceStyle == .dark ? 38 : 82, .black)))
      XCTAssertGreaterThanOrEqual(CustomKeyboardSkin.contrast(packed(skin.logoMark.resolvedColor(with: traits)), 0xFFFFFF), 4.5)
    }
    XCTAssertNotEqual(hex(skin.logoCircle, light), hex(KeyboardTheme.resolve(GlobalThemeCatalog.systemId, document: [:]).logoCircle, light))
    if #available(iOS 26.0, *) {
      XCTAssertEqual(hex(skin.keyBackground, light), 0xFFFFFF)
      XCTAssertEqual(hex(skin.functionKeyBackground, light), 0xFFFFFF)
      XCTAssertEqual(hex(skin.keyBackground, dark), 0x464646)
      XCTAssertEqual(hex(skin.functionKeyBackground, dark), 0x464646)
      XCTAssertEqual(hex(skin.background, light), 0xE2E3E8)
      XCTAssertFalse(skin.hasShadow)
      XCTAssertEqual(skin.cornerRadius, UIDevice.current.userInterfaceIdiom == .pad ? 8 : 6)
    } else {
      XCTAssertEqual(hex(skin.functionKeyBackground, light), 0xABB0BB)
      XCTAssertTrue(skin.hasShadow)
      XCTAssertEqual(hex(skin.keyShadowColor, light), 0x000000)
    }
    // 跟随系统仍是季节配色。
    XCTAssertEqual(KeyboardTheme.resolve(GlobalThemeCatalog.systemId, document: [:]).palette, SeasonKeyboardTokens.palette)
  }

  /// 真实键盘选「原生」后背景只铺一层几乎透明的键盘底色，仍透出系统底板，回车使用水杉绿；附上浅色和深色渲染图。
  @MainActor
  func testNativeKeyboardLeavesItsBackgroundToTheSystemBackdropOnTheRealKeyboard() throws {
    preserveSharedTheme()
    XCTAssertTrue(GlobalThemePreference.save(GlobalThemeCatalog.nativeId))
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    for style in [UIUserInterfaceStyle.light, .dark] {
      let controller = KeyboardViewController()
      controller.traitOverrides.verticalSizeClass = .regular
      // 放进窗口，明暗才会在渲染前传到每个按键；键盘出现时按偏好设明暗，所以之后再固定成要渲染的那一种。
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight))
      window.overrideUserInterfaceStyle = style
      window.rootViewController = controller
      controller.loadViewIfNeeded()
      window.frame.size.height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
      window.isHidden = false
      defer { window.isHidden = true }
      controller.viewWillAppear(false)
      controller.overrideUserInterfaceStyle = style
      window.layoutIfNeeded()
      let skin = KeyboardTheme.current
      XCTAssertTrue(skin.isNative)
      let traits = UITraitCollection(userInterfaceStyle: style)
      // 不是完全透明（那样空白处的触摸送不到扩展，#10173），但透明度不超过 `nativeBackgroundFillAlpha`，系统底板照样透出来。
      let backdrop = try XCTUnwrap(descendants(controller.view).first { $0 is KeyboardSkinBackgroundView })
      for surface in [controller.view!, backdrop] {
        let fill = try XCTUnwrap(surface.backgroundColor).resolvedColor(with: traits)
        XCTAssertGreaterThan(fill.cgColor.alpha, 0)
        XCTAssertLessThanOrEqual(fill.cgColor.alpha, KeyboardTheme.nativeBackgroundFillAlpha + 0.0001)
        XCTAssertEqual(packed(fill), packed(skin.background.resolvedColor(with: traits)), "填充用键盘自己的底色")
      }
      let enter = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "returnKey" } as? UIButton)
      XCTAssertEqual(enter.configuration?.background.backgroundColor?.resolvedColor(with: traits), skin.actionBackground.resolvedColor(with: traits))
      XCTAssertEqual(enter.layer.shadowOpacity, skin.hasShadow ? 1 : 0)
      let shift = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "shiftButton" } as? UIButton)
      XCTAssertEqual(shift.configuration?.background.backgroundColor?.resolvedColor(with: traits), skin.functionKeyBackground.resolvedColor(with: traits))
      let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        SystemKeyboardTokens.background.resolvedColor(with: traits).setFill()
        context.fill(controller.view.bounds)
        controller.view.layer.render(in: context.cgContext)
      }
      let attachment = XCTAttachment(image: image)
      attachment.name = "原生键盘-" + (style == .dark ? "深色" : "浅色"); attachment.lifetime = .keepAlways; add(attachment)
    }
  }

  /// 原生皮肤下整块盖住键盘的符号面板也透出系统底板：面板和选中分类透明，下面的工具栏和按键藏起来，关面板后再放出来。附上浅色渲染图供对照。
  @MainActor
  func testNativeSymbolPanelShowsTheSystemBackdropOverHiddenKeys() throws {
    preserveSharedTheme()
    XCTAssertTrue(GlobalThemePreference.save(GlobalThemeCatalog.nativeId))
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    func hidden(_ node: UIView) -> Bool { sequence(first: node, next: \.superview).contains { $0.alpha == 0 } }
    let controller = KeyboardViewController()
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight))
    window.overrideUserInterfaceStyle = .light
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    controller.viewWillAppear(false)
    controller.overrideUserInterfaceStyle = .light
    window.layoutIfNeeded()
    XCTAssertTrue(KeyboardTheme.current.isNative)
    let enter = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "returnKey" })
    XCTAssertFalse(hidden(enter))

    // 九键从底行的 符 打开符号面板。
    let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "symbolPanelKey" } as? UIButton)
    key.sendActions(for: .primaryActionTriggered)
    window.layoutIfNeeded()
    let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })
    // 面板和选中分类只铺一层几乎透明的键盘底色（#10173），系统底板照样透出来。
    for surface in [panel, try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "symbolCategory_0" })] {
      XCTAssertLessThanOrEqual(surface.backgroundColor?.cgColor.alpha ?? 0, KeyboardTheme.nativeBackgroundFillAlpha + 0.0001)
    }
    XCTAssertTrue(hidden(enter), "面板透明时下面的按键不能透出来")
    let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      SystemKeyboardTokens.background.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).setFill()
      context.fill(controller.view.bounds)
      controller.view.layer.render(in: context.cgContext)
    }
    let attachment = XCTAttachment(image: image)
    attachment.name = "原生符号面板-浅色"; attachment.lifetime = .keepAlways; add(attachment)

    let close = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "closeSymbolPanel" } as? UIButton)
    close.sendActions(for: .primaryActionTriggered)
    window.layoutIfNeeded()
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })
    XCTAssertFalse(hidden(enter))
  }

  /// #10173：原生皮肤下每一个要收触摸的视图自己带一层 `nativeBackgroundFill`：根视图、底板、候选栏和候选滚动视图，功能面板、它的翻页滚动视图和每一个功能格子，符号面板和它的两个滚动视图。#7682 在真机上看到，背后的根视图不透明并不能让一个只挂 `CAShapeLayer` 的按钮收到触摸，所以这里逐个检查收触摸的视图本身，而不是渲染出来的像素（祖先的底色会让每个像素都不透明）。这条用例只守住这个前提；系统跨进程命中是否因此把触摸送到扩展，只有真机能验证。换回「跟随系统」后功能格子和翻页滚动视图仍然无背景，其他皮肤不受影响。
  @MainActor
  func testNativeKeyboardGivesEveryTouchSurfaceItsOwnFill() throws {
    preserveSharedTheme()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    let traits = UITraitCollection(userInterfaceStyle: .light)

    /// 按 `themeId` 起一个真实键盘，打开功能面板，交给 `inspect` 检查，再打开符号面板交给 `inspectSymbols`。
    func withKeyboard(_ themeId: String, inspect: (KeyboardViewController, UIView) throws -> Void, inspectSymbols: (UIView) throws -> Void) throws {
      XCTAssertTrue(GlobalThemePreference.save(themeId))
      let controller = KeyboardViewController()
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight))
      window.overrideUserInterfaceStyle = .light
      window.rootViewController = controller
      window.isHidden = false
      defer { window.isHidden = true }
      controller.viewWillAppear(false)
      controller.overrideUserInterfaceStyle = .light
      window.layoutIfNeeded()
      let more = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "moreShortcut" } as? UIButton)
      more.sendActions(for: .primaryActionTriggered)
      window.layoutIfNeeded()
      try inspect(controller, try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" }))
      more.sendActions(for: .primaryActionTriggered)
      window.layoutIfNeeded()
      let symbolKey = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "symbolPanelKey" } as? UIButton)
      symbolKey.sendActions(for: .primaryActionTriggered)
      window.layoutIfNeeded()
      try inspectSymbols(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" }))
    }

    try withKeyboard(GlobalThemeCatalog.nativeId, inspect: { controller, picker in
      let skin = KeyboardTheme.current
      XCTAssertTrue(skin.drawsNativeBackground)
      let expected = packed(skin.background.resolvedColor(with: traits))
      func assertOwnFill(_ view: UIView, _ name: String, file: StaticString = #filePath, line: UInt = #line) {
        guard let fill = view.backgroundColor?.resolvedColor(with: traits) else {
          return XCTFail("\(name) 没有自己的底色", file: file, line: line)
        }
        XCTAssertGreaterThan(fill.cgColor.alpha, 0, "\(name) 自己要带内容", file: file, line: line)
        XCTAssertLessThanOrEqual(fill.cgColor.alpha, KeyboardTheme.nativeBackgroundFillAlpha + 0.0001, "\(name) 仍要透出系统底板", file: file, line: line)
        XCTAssertEqual(packed(fill), expected, "\(name) 用键盘自己的底色", file: file, line: line)
      }
      let tree = descendants(controller.view)
      assertOwnFill(controller.view, "根视图")
      assertOwnFill(try XCTUnwrap(tree.first { $0 is KeyboardSkinBackgroundView }), "底板")
      assertOwnFill(try XCTUnwrap(tree.first { $0.accessibilityIdentifier == "candidateStrip" }), "候选栏")
      assertOwnFill(try XCTUnwrap(tree.first { $0 is CandidateScrollView }), "候选滚动视图")
      assertOwnFill(picker, "功能面板")
      assertOwnFill(try XCTUnwrap(descendants(picker).first { $0 is UIScrollView }), "功能面板的翻页滚动视图")
      let tiles = descendants(picker).compactMap { $0 as? KeyboardFunctionTileView }
      XCTAssertFalse(tiles.isEmpty)
      for tile in tiles { assertOwnFill(tile, "功能格子 \(tile.tool.title)") }
    }, inspectSymbols: { panel in
      let skin = KeyboardTheme.current
      for (view, name) in [
        (panel, "符号面板"),
        (try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "symbolGrid" }), "符号滚动视图"),
        (try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "symbolCategories" }), "分类滚动视图"),
        (try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "symbolCategory_0" }), "选中的分类"),
      ] {
        let fill = try XCTUnwrap(view.backgroundColor, name).resolvedColor(with: traits)
        XCTAssertGreaterThan(fill.cgColor.alpha, 0, name)
        XCTAssertLessThanOrEqual(fill.cgColor.alpha, KeyboardTheme.nativeBackgroundFillAlpha + 0.0001, name)
        XCTAssertEqual(packed(fill), packed(skin.background.resolvedColor(with: traits)), name)
      }
    })

    // 其他皮肤不变：面板本来就是不透明的皮肤底色，格子和翻页滚动视图保持无背景。
    try withKeyboard(GlobalThemeCatalog.systemId, inspect: { _, picker in
      let skin = KeyboardTheme.current
      XCTAssertFalse(skin.drawsNativeBackground)
      XCTAssertEqual(picker.backgroundColor?.resolvedColor(with: traits), skin.background.resolvedColor(with: traits))
      XCTAssertEqual(try XCTUnwrap(descendants(picker).first { $0 is UIScrollView }).backgroundColor?.cgColor.alpha ?? 0, 0)
      for tile in descendants(picker).compactMap({ $0 as? KeyboardFunctionTileView }) {
        XCTAssertEqual(tile.backgroundColor?.cgColor.alpha ?? 0, 0, tile.tool.title)
      }
    }, inspectSymbols: { panel in
      XCTAssertEqual(panel.backgroundColor?.resolvedColor(with: traits), KeyboardTheme.current.background.resolvedColor(with: traits))
      XCTAssertEqual(try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "symbolGrid" }).backgroundColor?.cgColor.alpha ?? 0, 0)
    })
  }

  /// 从「原生」开始自定义时不把它写成底：`native` 不能当自定义主题的底，自定义主题画在跟随系统上。
  func testCustomizingTheNativeThemeDrawsOverSystem() {
    var document: [String: Any] = ["global_theme": GlobalThemeCatalog.nativeId, "custom_theme": ["base": "night"]]
    GlobalThemePreference.pickingCandidateColor("text", hex: "#123456")(&document)
    XCTAssertEqual(document["global_theme"] as? String, GlobalThemeCatalog.customId)
    XCTAssertNil((document["custom_theme"] as? [String: Any])?["base"])
    XCTAssertEqual(GlobalThemePreference.base(in: ["custom_theme": ["base": GlobalThemeCatalog.nativeId]]), GlobalThemeCatalog.systemId)
  }

  /// 「原生」先不上传，云端保留原来的主题，免得还没升级的设备拒收整份设置。
  func testCloudUploadLeavesTheNativeThemeOut() throws {
    preserveSharedTheme()
    XCTAssertTrue(GlobalThemePreference.save(GlobalThemeCatalog.nativeId))
    XCTAssertNil(try RustSettingsExport.settings()["platform.ios.global_theme"])
    XCTAssertTrue(GlobalThemePreference.save("night"))
    XCTAssertEqual(try RustSettingsExport.settings()["platform.ios.global_theme"] as? String, "night")
  }

  /// 为测试选中应用主题 `id`，结束后恢复原先存储的选择。
  private func selectAppTheme(_ id: String) {
    let defaults = UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier)
    let previous = defaults?.object(forKey: AppThemePalette.storageKey)
    addTeardownBlock {
      if let previous { defaults?.set(previous, forKey: AppThemePalette.storageKey) } else { defaults?.removeObject(forKey: AppThemePalette.storageKey) }
      KeyboardTheme.refreshSeason()
    }
    AppThemePalette.select(id)
  }

  /// 秋杉下的跟随系统画出设计稿的秋季键盘（ios-kb-1）：面板、字母键和功能键由季节色混合而成，文字为黑或白，提示色为设计稿的 `kbSub`，回车键填季节强调色。
  func testFollowSystemDrawsTheSeasonPalette() throws {
    selectAppTheme("qiushan")
    let lightSeason = try XCTUnwrap(AppThemePalette.resolved(dark: false))
    let darkSeason = try XCTUnwrap(AppThemePalette.resolved(dark: true))
    XCTAssertEqual(lightSeason.season, "autumn")
    let skin = KeyboardTheme.resolve(GlobalThemeCatalog.systemId, document: [:])
    XCTAssertEqual(skin.palette, SeasonKeyboardTokens.palette)
    XCTAssertFalse(skin.drawsNativeBackground)
    XCTAssertEqual(skin, KeyboardTheme.resolve(GlobalThemeCatalog.systemId, document: [:]))

    let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
    func hex(_ color: UIColor, _ traits: UITraitCollection) -> UInt32 { packed(color.resolvedColor(with: traits)) }
    XCTAssertEqual(hex(skin.background, light), 0xEED7C7)
    // 绿色通道算出来是 249.5；设计稿表格和 Android（`AppThemePaletteSmoke`）都写成 F9。
    XCTAssertEqual(hex(skin.keyBackground, light), 0xFDF9F6)
    XCTAssertEqual(hex(skin.functionKeyBackground, light), 0xE6C6B2)
    XCTAssertEqual(hex(skin.accent, light), 0xB5562B)
    XCTAssertEqual(hex(skin.actionBackground, light), 0xB5562B)
    XCTAssertEqual(hex(skin.actionForeground, light), 0xFFFFFF)
    XCTAssertEqual(hex(skin.keyForeground, light), 0x000000)
    XCTAssertEqual(hex(skin.secondary, light), 0x5A6B5D)

    XCTAssertEqual(skin.background.resolvedColor(with: dark), AppThemePalette.mix(darkSeason.accent, 10, CustomKeyboardSkin.color(0x161716)))
    XCTAssertEqual(skin.keyBackground.resolvedColor(with: dark), AppThemePalette.mix(darkSeason.accent, 10, CustomKeyboardSkin.color(0x3A3C3A)))
    XCTAssertEqual(skin.functionKeyBackground.resolvedColor(with: dark), AppThemePalette.mix(darkSeason.accent, 14, CustomKeyboardSkin.color(0x262826)))
    XCTAssertEqual(skin.accent.resolvedColor(with: dark), darkSeason.accent)
    XCTAssertEqual(skin.actionForeground.resolvedColor(with: dark), darkSeason.onAccent)
    XCTAssertEqual(hex(skin.keyForeground, dark), 0xFFFFFF)
    XCTAssertEqual(hex(skin.secondary, dark), 0x93A596)

    XCTAssertEqual(skin.accentSoft.resolvedColor(with: light).cgColor.alpha, 0x22 / 255, accuracy: 0.001)
    XCTAssertEqual(skin.accentSoft.resolvedColor(with: dark).cgColor.alpha, 0x40 / 255, accuracy: 0.001)
    XCTAssertEqual(hex(skin.toolbarActiveBackground, light), 0xB5562B)

    // 品牌键从当前皮肤取色，具名主题和自定义设计不再残留季节色。
    for theme in [skin, KeyboardTheme.resolve("night", document: [:]),
                  KeyboardTheme.designed(try XCTUnwrap(CustomKeyboardSkin.templates.first?.1))] {
      XCTAssertEqual(theme.logoCircle.resolvedColor(with: light), AppThemePalette.mix(theme.accent.resolvedColor(with: light), theme.appearance == .dark ? 22 : 14, theme.background.resolvedColor(with: light)))
      XCTAssertEqual(theme.logoCircle.resolvedColor(with: dark), AppThemePalette.mix(theme.accent.resolvedColor(with: dark), theme.appearance == .light ? 14 : 22, theme.background.resolvedColor(with: dark)))
      XCTAssertEqual(theme.logoMark.resolvedColor(with: light), AppThemePalette.mix(theme.accent.resolvedColor(with: light), 82, .black))
      XCTAssertEqual(theme.logoMark.resolvedColor(with: dark), AppThemePalette.mix(theme.accent.resolvedColor(with: dark), 82, .black))
    }
  }

  /// 季节刷新后会传到键盘已持有的主题上：配色每次解析时都会读取应用主题。
  func testRefreshedSeasonReachesTheHeldTheme() throws {
    selectAppTheme("qiushan")
    let skin = KeyboardTheme.resolve(GlobalThemeCatalog.systemId, document: [:])
    let light = UITraitCollection(userInterfaceStyle: .light)
    XCTAssertEqual(packed(skin.background.resolvedColor(with: light)), 0xEED7C7)
    AppThemePalette.select("dongxue")
    let winter = try XCTUnwrap(AppThemePalette.resolved(dark: false))
    XCTAssertEqual(winter.season, "winter")
    XCTAssertEqual(skin.background.resolvedColor(with: light), AppThemePalette.mix(winter.accent, 12, winter.background))
    XCTAssertEqual(skin.accent.resolvedColor(with: light), winter.accent)
  }

  /// 浅色四季的跟随系统候选条：首选画在字母键色的底片上，设置页的候选预览把它直接画在键盘底色上，两处都要到 WCAG AA 的 4.5:1。强调色本身在键盘底色上到不了，所以首选另用 `candidateAccent`；回车键、开启态仍是强调色，深色不变。
  func testFirstCandidateMeetsAAOnLightSeasonKeyboards() throws {
    let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
    func ratio(_ a: UIColor, _ b: UIColor, _ traits: UITraitCollection) -> Double {
      CustomKeyboardSkin.contrast(packed(a.resolvedColor(with: traits)), packed(b.resolvedColor(with: traits)))
    }
    let expected = ["chunya": 0x267308, "xiayin": 0x0F6B46, "qiushan": 0x9F4214, "dongxue": 0x346484] as [String: UInt32]
    for (id, candidate) in expected {
      selectAppTheme(id)
      let skin = KeyboardTheme.resolve(GlobalThemeCatalog.systemId, document: [:])
      XCTAssertEqual(skin.palette, SeasonKeyboardTokens.palette, id)
      XCTAssertEqual(packed(skin.candidateAccent.resolvedColor(with: light)), candidate, id)
      XCTAssertGreaterThanOrEqual(ratio(skin.candidateAccent, skin.background, light), 4.5, id)
      XCTAssertGreaterThanOrEqual(ratio(skin.candidateAccent, skin.keyBackground, light), 4.5, id)
      XCTAssertLessThan(ratio(skin.accent, skin.background, light), 4.5, id)
      XCTAssertEqual(skin.actionBackground.resolvedColor(with: light), skin.accent.resolvedColor(with: light), id)
      XCTAssertEqual(skin.candidateAccent.resolvedColor(with: dark), skin.accent.resolvedColor(with: dark), id)
    }
    // 应用主题解析不了时的经典兜底同样加深，深色不变；具名主题仍用自己的强调色。
    XCTAssertEqual(packed(NativeKeyboardTokens.candidateAccent.resolvedColor(with: light)), 0x15683B)
    XCTAssertGreaterThanOrEqual(ratio(NativeKeyboardTokens.candidateAccent, NativeKeyboardTokens.background, light), 4.5)
    XCTAssertGreaterThanOrEqual(ratio(NativeKeyboardTokens.candidateAccent, NativeKeyboardTokens.key, light), 4.5)
    XCTAssertEqual(packed(NativeKeyboardTokens.candidateAccent.resolvedColor(with: dark)), 0x5FBF84)
    let paper = KeyboardTheme.resolve("paper", document: [:])
    XCTAssertEqual(paper.candidateAccent.resolvedColor(with: light), paper.accent.resolvedColor(with: light))
  }

  /// 键盘进程跨过一次应用主题的改动再出现时，跟随系统的功能键（⌫、分词、！、123）画新季节的颜色。按键在进窗口时就按旧季节解析了动态色，`viewWillAppear` 刷新季节之后，`applyKeyboardSkin` 给的若还是同一个动态色实例，UIButton 视为配置没变，不再重新解析，功能键就停在旧季节，直到下一次出现。
  @MainActor
  func testFollowSystemKeysRedrawInTheNewSeasonWhenTheKeyboardReappears() throws {
    preserveSharedTheme()
    XCTAssertTrue(GlobalThemePreference.save(GlobalThemeCatalog.systemId))
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    selectAppTheme("qiushan")
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    /// 按键左缘中间的像素：图标居中，这里只有按键底色。
    func drawnFill(_ key: UIView) throws -> UInt32 {
      let format = UIGraphicsImageRendererFormat()
      format.scale = 1
      let image = UIGraphicsImageRenderer(bounds: key.bounds, format: format).image { key.layer.render(in: $0.cgContext) }
      var pixel = [UInt8](repeating: 0, count: 4)
      let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB))
      let context = try XCTUnwrap(CGContext(data: &pixel, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                            space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
      context.draw(try XCTUnwrap(image.cgImage), in: CGRect(x: -4, y: -(key.bounds.height / 2).rounded(), width: key.bounds.width, height: key.bounds.height))
      return UInt32(pixel[0]) << 16 | UInt32(pixel[1]) << 8 | UInt32(pixel[2])
    }
    func assertClose(_ drawn: UInt32, _ expected: UInt32, _ message: String, line: UInt = #line) {
      let channels = { (value: UInt32) in [value >> 16 & 0xff, value >> 8 & 0xff, value & 0xff].map(Int.init) }
      XCTAssertTrue(zip(channels(drawn), channels(expected)).allSatisfy { abs($0 - $1) <= 2 },
                    "\(message): drawn \(String(drawn, radix: 16)), expected \(String(expected, radix: 16))", line: line)
    }

    let controller = KeyboardViewController()
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight))
    window.overrideUserInterfaceStyle = .light
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    controller.viewWillAppear(false)
    controller.overrideUserInterfaceStyle = .light
    window.layoutIfNeeded()
    let light = UITraitCollection(userInterfaceStyle: .light)
    let delete = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "nineKeyDelete" })
    let autumn = packed(KeyboardTheme.current.functionKeyBackground.resolvedColor(with: light))
    assertClose(try drawnFill(delete), autumn, "秋杉下的 ⌫")

    // App 换成冬雪：只改 App Group 里的选择，键盘进程的季节缓存要等 `viewWillAppear` 才刷新。
    UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier)?.set("dongxue", forKey: AppThemePalette.storageKey)
    controller.viewWillAppear(false)
    window.layoutIfNeeded()
    XCTAssertEqual(AppThemePalette.resolved(dark: false)?.season, "winter")
    let winter = packed(KeyboardTheme.current.functionKeyBackground.resolvedColor(with: light))
    XCTAssertNotEqual(winter, autumn)
    assertClose(try drawnFill(delete), winter, "冬雪下的 ⌫")
  }

  /// 键盘再出现时皮肤和明暗都没变，`viewWillAppear` 和 `viewIsAppearing` 都不再重走整棵按键树、重设每个按键的配置；皮肤换了照样重新换装。按键不在窗口里时阴影色按默认外观解析，要由 `viewIsAppearing` 补走一遍。字母键先被涂成皮肤里没有的颜色，看换装有没有把它改回去。
  @MainActor
  func testReappearingWithAnUnchangedSkinLeavesTheKeysAlone() throws {
    preserveSharedTheme()
    let scheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = scheme }
    InputSchemePreference.scheme = .quanpin
    XCTAssertTrue(GlobalThemePreference.save("paper"))
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    let marker = UIColor(red: 0.1, green: 0.9, blue: 0.2, alpha: 1)
    func markLetterKey(_ controller: KeyboardViewController) throws -> UIButton {
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 A" } as? UIButton)
      var configuration = try XCTUnwrap(key.configuration)
      configuration.background.backgroundColor = marker
      key.configuration = configuration
      return key
    }
    let light = UITraitCollection(userInterfaceStyle: .light)

    let controller = KeyboardViewController()
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight))
    window.overrideUserInterfaceStyle = .light
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    controller.viewWillAppear(false)
    window.layoutIfNeeded()
    let key = try markLetterKey(controller)
    controller.viewWillAppear(false)
    controller.viewIsAppearing(false)
    XCTAssertEqual(key.configuration?.background.backgroundColor, marker, "皮肤没变时不重设按键")

    XCTAssertTrue(GlobalThemePreference.save(GlobalThemeCatalog.nativeId))
    controller.viewWillAppear(false)
    XCTAssertEqual(key.configuration?.background.backgroundColor?.resolvedColor(with: light),
                   KeyboardTheme.current.keyBackground.resolvedColor(with: light), "皮肤换了要重新换装")

    let detached = KeyboardViewController()
    detached.loadViewIfNeeded()
    detached.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    detached.viewWillAppear(false)
    let detachedKey = try markLetterKey(detached)
    // 真实的顺序：`viewWillAppear` 时视图还不在窗口里，trait 也没定，皮肤没变时不在这里换装（`viewDidLoad` 在窗口外走过一遍，留着标记）；进了窗口的 `viewIsAppearing` 才补走一遍。
    detached.viewWillAppear(false)
    XCTAssertEqual(detachedKey.configuration?.background.backgroundColor, marker, "窗口外的 viewWillAppear 不换装")
    detached.viewIsAppearing(false)
    XCTAssertEqual(detachedKey.configuration?.background.backgroundColor?.resolvedColor(with: light),
                   KeyboardTheme.current.keyBackground.resolvedColor(with: light), "不在窗口里换的装到 viewIsAppearing 要再换")
  }

  /// `kbHair`（ios-kb-31）：浅色键盘上是黑色 .12；深色键盘、任一模式下的深色表面内置主题，以及背景为深色的设计上是白色 .14。
  func testHairlineFollowsTheDrawnSurface() throws {
    let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
    func rgba(_ color: UIColor) -> [CGFloat] {
      var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
      color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
      return [red, green, blue, (alpha * 100).rounded() / 100]
    }
    let lightHair: [CGFloat] = [0, 0, 0, 0.12], darkHair: [CGFloat] = [1, 1, 1, 0.14]
    XCTAssertEqual(rgba(KeyboardTheme.system.hairline.resolvedColor(with: light)), lightHair)
    XCTAssertEqual(rgba(KeyboardTheme.system.hairline.resolvedColor(with: dark)), darkHair)
    for id in ["shuishan", "night", "ink"] where GlobalThemeCatalog.contains(id) {
      XCTAssertEqual(rgba(KeyboardTheme.resolve(id, document: [:]).hairline.resolvedColor(with: light)), darkHair, id)
    }
    if GlobalThemeCatalog.contains("paper") {
      let paper = KeyboardTheme.resolve("paper", document: [:])
      let traits = paper.appearance == .dark ? dark : light
      XCTAssertEqual(rgba(paper.hairline.resolvedColor(with: traits)), paper.appearance == .dark ? darkHair : lightHair)
    }
    var design = CustomKeyboardSkin()
    design.background = 0x132536
    XCTAssertEqual(rgba(KeyboardTheme.designed(design).hairline.resolvedColor(with: light)), darkHair)
    design.background = 0xF4F1EA
    XCTAssertEqual(rgba(KeyboardTheme.designed(design).hairline.resolvedColor(with: dark)), lightHair)
  }

  /// 按键底边（ios-kb-30）：浅色为 `rgba(38,62,44,.3)`，深色为 `rgba(0,0,0,.55)`，正下方偏移一个点、不模糊；设计保留它自己的阴影。
  func testKeyShadowUsesTheDesignEdge() {
    let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
    XCTAssertEqual(packed(NativeKeyboardTokens.keyShadowColor.resolvedColor(with: light)), 0x263E2C)
    XCTAssertEqual(packed(NativeKeyboardTokens.keyShadowColor.resolvedColor(with: dark)), 0x000000)
    XCTAssertEqual(NativeKeyboardTokens.keyShadowOpacity.light, 0.3)
    XCTAssertEqual(NativeKeyboardTokens.keyShadowOpacity.dark, 0.55)
    // 原生皮肤的底边是中性黑色，见 `testNativeDrawsTheSystemKeyboardAndLeavesTheBackgroundToTheSystemBackdrop`。
    for id in GlobalThemeCatalog.ids where id != GlobalThemeCatalog.customId && id != GlobalThemeCatalog.nativeId {
      let skin = KeyboardTheme.resolve(id, document: [:])
      XCTAssertEqual(packed(skin.keyShadowColor.resolvedColor(with: light)), 0x263E2C, id)
      XCTAssertEqual(skin.keyShadowColor.resolvedColor(with: light).cgColor.alpha, 0.3, accuracy: 0.001, id)
      XCTAssertEqual(skin.keyShadowColor.resolvedColor(with: dark).cgColor.alpha, 0.55, accuracy: 0.001, id)
      XCTAssertEqual(skin.shadowColor.resolvedColor(with: dark), skin.keyShadowColor.resolvedColor(with: dark), id)
      XCTAssertEqual(skin.shadowOffset, 1, id)
      XCTAssertEqual(skin.shadowRadius, 0, id)
    }
    var design = CustomKeyboardSkin()
    design.shadow = 0.2
    XCTAssertEqual(KeyboardTheme.designed(design).keyShadowColor.cgColor.alpha, 0.2, accuracy: 0.001)
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

  func testCustomSkinStoreRejectsOversizedPayload() throws {
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.object(forKey: CustomKeyboardSkinStore.key)
    defer {
      if let previous { defaults.set(previous, forKey: CustomKeyboardSkinStore.key) }
      else { defaults.removeObject(forKey: CustomKeyboardSkinStore.key) }
    }
    var design = CustomKeyboardSkin()
    design.background = 0x123456
    design.photo = Data(repeating: 0, count: 800_000)
    let data = try JSONEncoder().encode(design)
    XCTAssertGreaterThan(data.count, 1_000_000)
    defaults.set(data, forKey: CustomKeyboardSkinStore.key)
    XCTAssertEqual(CustomKeyboardSkinStore.current, CustomKeyboardSkin())
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
    let picker = KeyboardSkinPickerView(selected: GlobalThemeCatalog.systemId, document: [:], dark: false, onSelect: { _ in },
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
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
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
      // 属于另一种明暗的主题按跟随系统画。
      XCTAssertEqual(skin.id, GlobalThemeCatalog.effectiveId(id, dark: controller.traitCollection.userInterfaceStyle == .dark))
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
      // 回车键无论空闲还是组字中都用主题的操作色（dc.html `retBg`）。
      XCTAssertEqual(key.configuration?.background.backgroundColor?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)),
        skin.actionBackground.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), id)
      let shift = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "shiftButton" } as? UIButton)
      XCTAssertEqual(shift.configuration?.background.backgroundColor?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)),
        skin.functionKeyBackground.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), id)
    }
  }

  @MainActor
  func testReturnKeepsTheAccentWhetherOrNotComposing() throws {
    preserveSharedTheme()
    let scheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = scheme }
    InputSchemePreference.scheme = .quanpin
    XCTAssertTrue(GlobalThemePreference.save("paper"))
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    func button(_ id: String) throws -> UIButton {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == id } as? UIButton, id)
    }
    let light = UITraitCollection(userInterfaceStyle: .light)
    let skin = KeyboardTheme.current
    func fill() throws -> UIColor? { try button("returnKey").configuration?.background.backgroundColor?.resolvedColor(with: light) }
    XCTAssertEqual(try fill(), skin.actionBackground.resolvedColor(with: light))
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
    // 首个候选是选中的那个：首选字色（`candidateAccent`）放在键色的底片上；其余候选直接落在键盘上。
    let first = try button("candidate-1")
    XCTAssertEqual(first.configuration?.baseForegroundColor?.resolvedColor(with: light), skin.candidateAccent.resolvedColor(with: light))
    XCTAssertEqual(first.configuration?.background.backgroundColor?.resolvedColor(with: light),
      skin.keyBackground.resolvedColor(with: light))
    XCTAssertEqual(first.configuration?.background.strokeWidth, 0)
    XCTAssertEqual(try button("candidate-2").configuration?.background.backgroundColor?.cgColor.alpha ?? 0, 0)
    XCTAssertEqual(first.layer.shadowOpacity, 0)
    XCTAssertEqual(try button("candidate-2").configuration?.baseForegroundColor?.resolvedColor(with: light),
      skin.keyForeground.resolvedColor(with: light))
    // 上屏结束组字，回车键交回输入框自己的标签；填充色仍是操作色。
    try button("returnKey").sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotEqual(try button("returnKey").accessibilityLabel, "确认")
    XCTAssertEqual(try fill(), skin.actionBackground.resolvedColor(with: light))
  }

  /// 设计稿（dc.html `mobCands`）里拼写和展开箭头都不带边框。iOS 27 给每个按钮配置默认一条透明的 1pt 描边，换肤重新着色时不能把它变成强调色描边。
  @MainActor
  func testSpellingAndExpandChevronDrawNoOutline() throws {
    preserveSharedTheme()
    let scheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = scheme }
    InputSchemePreference.scheme = .quanpin
    XCTAssertTrue(GlobalThemePreference.save("paper"))
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }
    for character in "nihao" {
      let key = try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityLabel == "字母 \(String(character).uppercased())"
      } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    for id in ["preeditButton", "expandCandidates"] {
      let button = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == id } as? UIButton, id)
      let background = try XCTUnwrap(button.configuration?.background, id)
      let drawn = background.strokeWidth > 0 && (background.strokeColor?.cgColor.alpha ?? 0) > 0
      XCTAssertFalse(drawn, "\(id) draws an outline")
    }
  }

  /// Hints, glosses and markers on the keyboard palette take the theme's `secondary`, in the strip and in the expanded panel alike, so they match the 候选栏 preview (THEME_CONTRACT: hints and numbers = `secondary`).
  @MainActor
  func testCandidateAnnotationsUseTheThemeSecondary() throws {
    preserveSharedTheme()
    let scheme = InputSchemePreference.scheme
    let enabledSchemes = InputSchemePreference.enabledSchemes
    let followsDesktop = CandidatePalette.defaults.object(forKey: CandidatePalette.followsDesktopKey)
    defer {
      InputSchemePreference.enabledSchemes = enabledSchemes
      InputSchemePreference.scheme = scheme
      CandidatePalette.defaults.set(followsDesktop, forKey: CandidatePalette.followsDesktopKey)
    }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .wubi
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
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
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
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
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

  /// 固定明暗的主题只在那一种明暗下提供；所选主题属于另一种明暗时按跟随系统画，所选值不改。
  @MainActor func testFixedModeThemesAreOfferedOnlyInTheirMode() throws {
    let dark = GlobalThemeCatalog.entries.filter { $0.appearance == .dark }.map(\.id)
    let light = GlobalThemeCatalog.entries.filter { $0.appearance == .light }.map(\.id)
    XCTAssertFalse(dark.isEmpty)
    XCTAssertFalse(light.isEmpty)
    for id in [GlobalThemeCatalog.systemId, GlobalThemeCatalog.customId] {
      XCTAssertTrue(GlobalThemeCatalog.fits(id, dark: true), id)
      XCTAssertTrue(GlobalThemeCatalog.fits(id, dark: false), id)
    }
    XCTAssertEqual(GlobalThemeCatalog.ids(offeredIn: true), GlobalThemeCatalog.ids.filter { !light.contains($0) })
    XCTAssertEqual(GlobalThemeCatalog.ids(offeredIn: false), GlobalThemeCatalog.ids.filter { !dark.contains($0) })
    for id in dark {
      XCTAssertEqual(GlobalThemeCatalog.effectiveId(id, dark: true), id)
      XCTAssertEqual(GlobalThemeCatalog.effectiveId(id, dark: false), GlobalThemeCatalog.systemId)
      XCTAssertEqual(KeyboardTheme.resolve(document: ["global_theme": id], dark: false).id, GlobalThemeCatalog.systemId)
      XCTAssertEqual(KeyboardTheme.resolve(document: ["global_theme": id], dark: true).id, id)
      XCTAssertEqual(KeyboardTheme.resolve(document: ["global_theme": id]).id, id)
    }
    for id in light {
      XCTAssertEqual(GlobalThemeCatalog.effectiveId(id, dark: false), id)
      XCTAssertEqual(GlobalThemeCatalog.effectiveId(id, dark: true), GlobalThemeCatalog.systemId)
    }
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    for mode in [false, true] {
      let picker = KeyboardSkinPickerView(selected: GlobalThemeCatalog.systemId, document: [:], dark: mode,
                                          onSelect: { _ in }, onSelectDesign: { _ in }, onClose: {})
      let cards = Set(descendants(picker).compactMap(\.accessibilityIdentifier).filter { $0.hasPrefix("skinCard-") })
      XCTAssertEqual(cards, Set(GlobalThemeCatalog.ids(offeredIn: mode).map { "skinCard-\($0)" }), "dark \(mode)")
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

  /// 磁贴、工具栏和回车使用当前主题的强调色，原生皮肤使用水杉绿。
  func testSwitchedOnTileUsesTheActiveThemeAccent() {
    XCTAssertTrue(GlobalThemeCatalog.ids.contains { KeyboardTheme.resolve($0, document: [:]).palette != nil })
    for id in GlobalThemeCatalog.ids {
      let skin = KeyboardTheme.resolve(id, document: [:])
      guard skin.design == nil else { continue }
      // 候选条的配色按相等比较，所以着色不能每次读取都生成新的颜色。
      if skin.appearance != nil || skin.palette == nil || skin.palette == SeasonKeyboardTokens.palette {
        XCTAssertEqual(skin.accentSoft, skin.accentSoft, id)
      }
      for style in [UIUserInterfaceStyle.light, .dark] {
        let traits = UITraitCollection(userInterfaceStyle: style)
        let accent = skin.accent.resolvedColor(with: traits)
        XCTAssertEqual(skin.toolbarActiveBackground.resolvedColor(with: traits), skin.accentSoft.resolvedColor(with: traits), "\(id) \(style.rawValue)")
        XCTAssertEqual(skin.toggleForeground.resolvedColor(with: traits), accent, "\(id) \(style.rawValue)")
        XCTAssertEqual(skin.actionBackground.resolvedColor(with: traits),
                       accent, "\(id) \(style.rawValue)")
        guard let palette = skin.palette else {
          // 经典兜底保留原生 token，原生皮肤使用水杉绿。
          let soft = skin.isNative ? SystemKeyboardTokens.accentSoft : NativeKeyboardTokens.accentSoft
          XCTAssertEqual(skin.accentSoft.resolvedColor(with: traits), soft.resolvedColor(with: traits), "\(id) \(style.rawValue)")
          continue
        }
        XCTAssertEqual(accent, palette.accent.resolvedColor(with: traits), "\(id) \(style.rawValue)")
        let drawsDark = (skin.appearance ?? style) == .dark
        XCTAssertEqual(skin.accentSoft.resolvedColor(with: traits), accent.withAlphaComponent(drawsDark ? 0x40 / 255 : 0x22 / 255), "\(id) \(style.rawValue)")
        XCTAssertEqual(skin.actionForeground.resolvedColor(with: traits), palette.onAccent.resolvedColor(with: traits), "\(id) \(style.rawValue)")
      }
    }
    var design = CustomKeyboardSkin()
    design.accent = 0xD4BBFF
    let designed = KeyboardTheme.designed(design)
    XCTAssertEqual(packed(designed.toggleForeground), 0xD4BBFF)
    XCTAssertEqual(packed(designed.toolbarActiveBackground), 0xD4BBFF)
  }

  /// 功能菜单的图块确实按 `toggleForeground` 画开启状态，按 `keyForeground` 画关闭状态，上面那条语义颜色的测试才守得住图块本身。
  @MainActor func testFunctionTileDrawsItsOnStateInToggleForeground() throws {
    var design = CustomKeyboardSkin()
    design.accent = 0xD4BBFF
    let skin = KeyboardTheme.designed(design)
    for selected in [true, false] {
      let tile = KeyboardFunctionTileView(
        tool: KeyboardTool(id: "fullwidth", title: "全角", face: .glyph("全"), isToggle: true, selected: selected) {}, skin: skin)
      let title = try XCTUnwrap(tile.subviews.compactMap { $0 as? UILabel }.first { $0.text == "全角" })
      let expected = selected ? skin.toggleForeground : skin.keyForeground
      XCTAssertEqual(title.textColor.resolvedColor(with: tile.traitCollection), expected.resolvedColor(with: tile.traitCollection), "selected: \(selected)")
    }
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

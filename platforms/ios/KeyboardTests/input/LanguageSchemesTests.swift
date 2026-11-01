import XCTest
import UIKit

/// 粤拼、注音、越南语、藏文和笔画：键盘提供其中哪些、它们怎样写进共享偏好、大千键位和笔画键，以及键盘经桥接层看到的就地组字。
@MainActor
final class LanguageSchemesTests: XCTestCase {
  private var state: URL!

  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-language-schemes-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func type(_ keys: String, into bridge: MetasequoiaInputSessionBridge) -> MetasequoiaInputSnapshot {
    var snapshot = MetasequoiaInputSnapshot()
    for key in keys { snapshot = bridge.handleCharacter(String(key)) }
    return snapshot
  }

  // MARK: - Availability

  func testTheLanguageSchemesStartDisabledOnAFreshInstall() throws {
    let defaults = try XCTUnwrap(UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier))
    let previous = defaults.object(forKey: InputSchemePreference.enabledSchemesKey)
    defer { defaults.set(previous, forKey: InputSchemePreference.enabledSchemesKey) }
    defaults.removeObject(forKey: InputSchemePreference.enabledSchemesKey)
    let enabled = InputSchemePreference.enabledSchemes
    // 全拼 14 键和 18 键同样要用户自己打开，与 client-core 的 `DEFAULT_ENABLED` 一致。
    XCTAssertEqual(enabled, ChineseInputScheme.allCases.filter { ![.cantonese, .zhuyin, .vietnamese, .tibetan, .stroke, .fourteenKey, .nineKeyShuangpin, .eighteenKey, .cantoneseShuangpin].contains($0) })
  }

  func testAMissingDictionaryHidesOnlyItsScheme() {
    let enabled: [ChineseInputScheme] = [.quanpin, .cantonese, .cantoneseShuangpin, .zhuyin, .vietnamese]
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: nil), enabled,
                   "a process that cannot tell hides nothing")
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: [.cantonese, .cantoneseShuangpin, .zhuyin]), enabled)
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: [.zhuyin]), [.quanpin, .zhuyin, .vietnamese])
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: []), [.quanpin, .vietnamese],
                   "Vietnamese needs no data")
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: [.cantonese], installed: []), [.quanpin])
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: [.quanpin, .tibetan], installed: []), [.quanpin, .tibetan],
                   "藏文不需要词库")
    let withStroke: [ChineseInputScheme] = [.quanpin, .zhuyin, .stroke]
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: withStroke, installed: [.zhuyin]), [.quanpin, .zhuyin])
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: withStroke, installed: [.stroke]), [.quanpin, .stroke])
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: [.stroke], installed: [.cantonese, .zhuyin]), [.quanpin])
  }

  func testInstalledSchemesAreReadFromTheDictionaryFiles() throws {
    XCTAssertNil(InputSchemePreference.installedLanguageSchemes(in: nil))
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [])
    try Data().write(to: state.appendingPathComponent("msime-cantonese.db"))
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [.cantonese, .cantoneseShuangpin])
    try Data().write(to: state.appendingPathComponent("msime-zhuyin.db"))
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [.cantonese, .cantoneseShuangpin, .zhuyin])
    try Data().write(to: state.appendingPathComponent("msime-stroke.db"))
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [.cantonese, .cantoneseShuangpin, .zhuyin, .stroke])
    try FileManager.default.removeItem(at: state.appendingPathComponent("msime-cantonese.db"))
    try FileManager.default.removeItem(at: state.appendingPathComponent("msime-zhuyin.db"))
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [.stroke], "msime-stroke.db alone offers Stroke")
  }

  /// The test host bundles EngineResources, so it can tell what is installed; the App bundle carries no Engine and cannot.
  func testTheTestHostLooksBesideItsEngineResources() throws {
    let directory = try XCTUnwrap(InputSchemePreference.languageDictionaryDirectory(in: .main))
    XCTAssertEqual(directory.lastPathComponent, "language-dictionaries")
    XCTAssertNotNil(InputSchemePreference.installedLanguageSchemes)
  }

  // MARK: - Shared preferences

  func testCantoneseZhuyinAndStrokeAreChineseSchemesAndVietnameseIsNot() {
    let enabled = ChineseInputScheme.allCases
    for (scheme, engine, remembered) in [(ChineseInputScheme.cantonese, "cantonese", true), (.cantoneseShuangpin, "cantonese", true), (.zhuyin, "zhuyin", true), (.vietnamese, "vietnamese", false), (.tibetan, "tibetan", false), (.stroke, "stroke", true)] {
      var document: [String: Any] = ["last_chinese_scheme": "wubi"]
      MetasequoiaInputSessionBridge.schemeMapping(scheme, enabledSchemes: enabled)?(&document)
      XCTAssertEqual(document["scheme"] as? String, engine)
      XCTAssertEqual(document["last_chinese_scheme"] as? String, remembered ? engine : "wubi", engine)
      XCTAssertEqual(document["touch_keyboard_layout"] as? String, "twenty_six_key", engine)
      XCTAssertEqual(ChineseInputScheme.scheme(sharedIdentifier: scheme.sharedIdentifier), scheme)
    }
    XCTAssertEqual(ChineseInputScheme.stroke.sharedIdentifier, "stroke")
    XCTAssertEqual(ChineseInputScheme(rawValue: "stroke"), .stroke, "the App Group value is the shared one")
    XCTAssertEqual(ChineseInputScheme.stroke.title, "笔画")
    XCTAssertEqual(ChineseInputScheme.allCases.suffix(3), [.nineKeyShuangpin, .eighteenKey, .cantoneseShuangpin], "the scheme order is persisted, so new schemes go last")
  }

  func testCantoneseShuangpinHasItsOwnSavedSelection() {
    let scheme = ChineseInputScheme.cantoneseShuangpin
    XCTAssertEqual(scheme.engineScheme, "cantonese")
    XCTAssertEqual(scheme.sharedIdentifier, "cantonese_shuangpin")
    XCTAssertEqual(scheme.title, "粤语双拼")
    XCTAssertTrue(ChineseInputScheme.optInSchemes.contains(scheme))
    XCTAssertNil(scheme.cloudSchema)
    XCTAssertEqual(KeyboardViewController.languageKeyValue(scheme), "粤语双拼输入")
    var document: [String: Any] = [:]
    MetasequoiaInputSessionBridge.schemeMapping(scheme, enabledSchemes: [.quanpin, .cantonese, scheme])?(&document)
    XCTAssertEqual(document["scheme"] as? String, "cantonese")
    XCTAssertEqual((document["touch_keyboard_schemes"] as? [String: Any])?["selected"] as? String, "cantonese_shuangpin")
    XCTAssertEqual(InputSchemePreference.selectedScheme(in: document), scheme)
    MetasequoiaInputSessionBridge.schemeMapping(.cantonese, enabledSchemes: [.quanpin, .cantonese, scheme])?(&document)
    XCTAssertEqual(InputSchemePreference.selectedScheme(in: document), .cantonese)
  }

  /// 粤语双拼和微软双拼一样在第二排 L 右边有 `;` 键（#10099）：引擎把 ing、ik 放在 `;` 上，没有这个键这些音节打不出来，所以选了方案就一直显示，没有开关；其他方案不加这个键（普通粤拼把 ing、ik 完整拼出来，全拼、小鹤等方案的 `;` 不是编码键）。
  func testOnlyMicrosoftAndCantoneseShuangpinShowTheSemicolonFinalKey() {
    XCTAssertEqual(ChineseInputScheme.allCases.filter(\.showsSemicolonFinalKey), [.microsoft, .cantoneseShuangpin])
    XCTAssertEqual(ChineseInputScheme.microsoft.semicolonFinalKeyAccessibilityLabel, "分号，微软双拼 ing")
    XCTAssertEqual(ChineseInputScheme.cantoneseShuangpin.semicolonFinalKeyAccessibilityLabel, "分号，粤语双拼 ing 和 ik")
    XCTAssertEqual(ChineseInputScheme.cantonese.semicolonFinalKeyAccessibilityLabel, "分号")
  }

  /// 手机第二排的缩进跟着 `;` 键是否真的显示走，不只看方案：微软双拼的中文模式里 `;` 填上右侧空位、第二排不缩进；切到英文 `;` 藏起来，第二排回到两侧缩进，A 到 L 与第一排同宽，不被拉宽到整排。粤语双拼走同一个条件，但它的用例要暂存粤语词库，这里用 CI 上就能跑的微软双拼守住。
  func testTheMiddleRowIndentsAgainWhenTheSemicolonKeyHides() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .microsoft
    XCTAssertEqual(InputSchemePreference.scheme, .microsoft)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    defer { controller.viewDidDisappear(false) }
    func layout() {
      controller.view.setNeedsLayout()
      controller.view.layoutIfNeeded()
      controller.view.layoutIfNeeded()
    }
    let shown = { (view: UIView) in sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden } }
    // 英文模式可能自动大写，按键面不分大小写找显示中的那个键。
    func frame(of face: String) throws -> CGRect {
      let key = try XCTUnwrap(nodes(controller.view).first { view in
        guard let button = view as? UIButton, button.configuration?.title?.lowercased() == face else { return false }
        return shown(button)
      }, "no visible key \(face)")
      return key.convert(key.bounds, to: controller.view)
    }
    let semicolon = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "microsoftFinalKey" })
    layout()

    XCTAssertTrue(shown(semicolon))
    var q = try frame(of: "q"), a = try frame(of: "a")
    XCTAssertEqual(a.minX, q.minX, accuracy: 0.5, "the semicolon key fills the row, so A lines up with Q")
    XCTAssertEqual(a.width, q.width, accuracy: 0.5)

    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "bottomLanguageKey" } as? UIButton)
      .sendActions(for: .primaryActionTriggered)
    layout()
    XCTAssertFalse(shown(semicolon), "English mode has no semicolon final")
    q = try frame(of: "q")
    a = try frame(of: "a")
    let l = try frame(of: "l"), p = try frame(of: "p")
    XCTAssertEqual(a.width, q.width, accuracy: 0.5, "A..L keep the top row's key width")
    XCTAssertGreaterThan(a.minX, q.minX + q.width / 4, "the middle row is indented again")
    XCTAssertLessThan(l.maxX, p.maxX - p.width / 4)
  }

  /// 粤语双拼的 `;` 与微软双拼排在同一个位置、同样宽：第二排十个键，两侧不缩进；读屏说出粤语双拼的韵母；M 加 `;` 组成 ming 或 mik。普通粤拼不显示这个键。
  func testCantoneseShuangpinKeyboardShowsTheSemicolonKeyRightOfL() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.cantoneseShuangpin) == true,
                      "msime-cantonese.db is not staged into the test host")
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let shown = { (view: UIView) in sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden } }
    func keyboard(_ scheme: ChineseInputScheme) throws -> KeyboardViewController {
      InputSchemePreference.scheme = scheme
      XCTAssertEqual(InputSchemePreference.scheme, scheme)
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
      controller.viewWillAppear(false)
      controller.view.layoutIfNeeded()
      return controller
    }
    func key(_ predicate: (UIView) -> Bool, in controller: KeyboardViewController) throws -> UIButton {
      try XCTUnwrap(nodes(controller.view).first(where: predicate) as? UIButton)
    }
    func semicolon(in controller: KeyboardViewController) throws -> UIButton {
      try key({ $0.accessibilityIdentifier == "microsoftFinalKey" }, in: controller)
    }
    func letter(_ face: String, in controller: KeyboardViewController) throws -> UIButton {
      try key({ $0.accessibilityLabel == "字母 \(face)" && shown($0) }, in: controller)
    }
    func frame(_ view: UIView, in controller: KeyboardViewController) -> CGRect {
      view.convert(view.bounds, to: controller.view)
    }

    let microsoft = try keyboard(.microsoft)
    defer { microsoft.viewDidDisappear(false) }
    let microsoftSemicolon = try semicolon(in: microsoft)
    XCTAssertTrue(shown(microsoftSemicolon))
    XCTAssertEqual(microsoftSemicolon.accessibilityLabel, "分号，微软双拼 ing")

    let cantonese = try keyboard(.cantoneseShuangpin)
    defer { cantonese.viewDidDisappear(false) }
    let cantoneseSemicolon = try semicolon(in: cantonese)
    XCTAssertTrue(shown(cantoneseSemicolon), "ing and ik live only on the semicolon key")
    XCTAssertEqual(cantoneseSemicolon.accessibilityLabel, "分号，粤语双拼 ing 和 ik")
    XCTAssertNil(cantoneseSemicolon.accessibilityValue)
    let l = frame(try letter("L", in: cantonese), in: cantonese)
    let semicolonFrame = frame(cantoneseSemicolon, in: cantonese)
    XCTAssertGreaterThan(semicolonFrame.minX, l.maxX, "the key sits right of L")
    XCTAssertEqual(semicolonFrame.midY, l.midY, accuracy: 0.5)
    XCTAssertEqual(semicolonFrame.width, l.width, accuracy: 0.5)
    // 与微软双拼同一个横向几何：第二排不缩进，A 和 `;` 的横向位置、宽度都一样（纵向随候选栏高度，不比）。
    let microsoftFrame = frame(microsoftSemicolon, in: microsoft)
    XCTAssertEqual(semicolonFrame.minX, microsoftFrame.minX, accuracy: 0.5)
    XCTAssertEqual(semicolonFrame.width, microsoftFrame.width, accuracy: 0.5)
    XCTAssertEqual(frame(try letter("A", in: cantonese), in: cantonese).minX,
                   frame(try letter("A", in: microsoft), in: microsoft).minX, accuracy: 0.5)

    // `;` 是韵母键：M 之后按它，组字变成 ming 或 mik 的候选，而不是上屏一个分号。排序由引擎和词库决定，这里只要求出现 ming（名、明）或 mik（覓、冪）里的任意一个字。
    func candidates() -> [String] {
      cantonese.view.layoutIfNeeded()
      return nodes(cantonese.view).compactMap { $0 as? UIButton }
        .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
        .compactMap { $0.configuration?.attributedTitle.map { String($0.characters) } ?? $0.configuration?.title }
    }
    try letter("M", in: cantonese).sendActions(for: .primaryActionTriggered)
    let initialOnly = candidates()
    cantoneseSemicolon.sendActions(for: .primaryActionTriggered)
    let final = candidates()
    XCTAssertNotEqual(final, initialOnly, "the semicolon joined the composition")
    XCTAssertTrue(final.contains { ["名", "明", "覓", "冪"].contains($0) }, "m; is ming or mik: \(final.prefix(8))")

    let plain = try keyboard(.cantonese)
    defer { plain.viewDidDisappear(false) }
    XCTAssertFalse(shown(try semicolon(in: plain)), "plain Jyutping has no semicolon final")
  }

  /// 「双拼键位提示」对粤语双拼也生效（#10438）：提示从引擎取，I 键写 `gw / i`、V 键写 `ng / ui yu`，读屏读同一行；开关关掉时不画。普通粤拼完整拼写，不画提示。
  func testCantoneseShuangpinLabelsItsKeysWhenTheHintSwitchIsOn() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.cantoneseShuangpin) == true,
                      "msime-cantonese.db is not staged into the test host")
    let previousScheme = InputSchemePreference.scheme
    let previousHints = KeyboardLayoutPreference.shuangpinKeyHints
    defer {
      InputSchemePreference.scheme = previousScheme
      KeyboardLayoutPreference.shuangpinKeyHints = previousHints
    }
    let shown = { (view: UIView) in sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden } }
    func keyboard(_ scheme: ChineseInputScheme, hints: Bool) -> KeyboardViewController {
      InputSchemePreference.scheme = scheme
      KeyboardLayoutPreference.shuangpinKeyHints = hints
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
      controller.viewWillAppear(false)
      controller.view.layoutIfNeeded()
      return controller
    }
    func letter(_ face: String, in controller: KeyboardViewController) throws -> UIButton {
      try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 \(face)" && shown($0) } as? UIButton)
    }
    // 提示行是直接加在按键上的 9pt 标签，见 `attachHintLabel`。
    func hintLabel(of button: UIButton) -> UILabel? {
      button.subviews.compactMap { $0 as? UILabel }.first { $0.font.pointSize == 9 }
    }

    let labelled = keyboard(.cantoneseShuangpin, hints: true)
    defer { labelled.viewDidDisappear(false) }
    let i = try letter("I", in: labelled)
    XCTAssertEqual(i.accessibilityValue, "gw / i")
    XCTAssertEqual(hintLabel(of: i)?.text, "gw / i")
    XCTAssertEqual(hintLabel(of: i)?.isHidden, false)
    XCTAssertEqual(i.configuration?.contentInsets.bottom, 11)
    XCTAssertEqual(try letter("V", in: labelled).accessibilityValue, "ng / ui yu")
    XCTAssertEqual(try letter("P", in: labelled).accessibilityValue, "eon un ung ut")

    let off = keyboard(.cantoneseShuangpin, hints: false)
    defer { off.viewDidDisappear(false) }
    XCTAssertNil(try letter("I", in: off).accessibilityValue, "the switch also covers Cantonese shuangpin")

    let plain = keyboard(.cantonese, hints: true)
    defer { plain.viewDidDisappear(false) }
    XCTAssertNil(try letter("I", in: plain).accessibilityValue, "plain Jyutping is spelled out in full")
  }

  /// 全拼 14 键：引擎方案是全拼，共享文档里的 id 和布局都是 `fourteen_key`，云端按全拼上传，默认不启用，空格键和方案简称写「全拼」，统计来源是 `fourteenKey`。
  func testTheFourteenKeySchemeIsQuanpinOnItsOwnLayout() {
    let scheme = ChineseInputScheme.fourteenKey
    XCTAssertEqual(scheme.engineScheme, "quanpin")
    XCTAssertEqual(scheme.sharedIdentifier, "fourteen_key")
    XCTAssertEqual(ChineseInputScheme.scheme(sharedIdentifier: "fourteen_key"), .fourteenKey)
    XCTAssertEqual(scheme.cloudSchema, "quanpin")
    XCTAssertEqual(scheme.title, "全拼 14 键")
    XCTAssertEqual(scheme.shortLabel, "全拼")
    XCTAssertTrue(ChineseInputScheme.optInSchemes.contains(.fourteenKey))
    XCTAssertTrue(scheme.writesChinese)
    XCTAssertFalse(scheme.editsBySyllable, "a run of group codes is as ambiguous as nine-key digits")
    XCTAssertNil(scheme.shuangpinProfile)
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(scheme), "中")
    XCTAssertEqual(TypingSource(rawValue: scheme.rawValue), .fourteenKey)
    var document: [String: Any] = ["last_chinese_scheme": "wubi"]
    MetasequoiaInputSessionBridge.schemeMapping(scheme, enabledSchemes: ChineseInputScheme.allCases)?(&document)
    XCTAssertEqual(document["scheme"] as? String, "quanpin")
    XCTAssertEqual(document["last_chinese_scheme"] as? String, "quanpin")
    XCTAssertEqual(document["touch_keyboard_layout"] as? String, "fourteen_key")
    XCTAssertEqual((document["touch_keyboard_schemes"] as? [String: Any])?["selected"] as? String, "fourteen_key")
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .returnKey), .finishComposition,
                   "the raw keys are group codes, so every boundary converts as nine-key does")
  }

  /// 全拼 18 键与 14 键一样：引擎方案是全拼，共享文档里的 id 和布局都是 `eighteen_key`，云端按全拼上传、布局写 `eighteen_key`，默认不启用，简称写「全拼」，统计来源是 `eighteenKey`，回车和切换模式先结束组字。
  func testTheEighteenKeySchemeIsQuanpinOnItsOwnLayout() {
    let scheme = ChineseInputScheme.eighteenKey
    XCTAssertEqual(scheme.engineScheme, "quanpin")
    XCTAssertEqual(scheme.sharedIdentifier, "eighteen_key")
    XCTAssertEqual(ChineseInputScheme.scheme(sharedIdentifier: "eighteen_key"), .eighteenKey)
    XCTAssertEqual(scheme.cloudSchema, "quanpin")
    XCTAssertEqual(scheme.cloudKeyboardLayout, "eighteen_key")
    XCTAssertEqual(scheme.title, "全拼 18 键")
    XCTAssertEqual(scheme.shortLabel, "全拼")
    XCTAssertTrue(ChineseInputScheme.optInSchemes.contains(.eighteenKey))
    XCTAssertFalse(scheme.editsBySyllable)
    XCTAssertNil(scheme.shuangpinProfile)
    XCTAssertEqual(LetterKeyGrid.of(scheme, shuangpinLayout: .twentySixKey, wubiLayout: .fourteenKey), .eighteenKey)
    XCTAssertEqual(TypingSource(rawValue: scheme.rawValue), .eighteenKey)
    var document: [String: Any] = ["last_chinese_scheme": "wubi"]
    MetasequoiaInputSessionBridge.schemeMapping(scheme, enabledSchemes: ChineseInputScheme.allCases)?(&document)
    XCTAssertEqual(document["scheme"] as? String, "quanpin")
    XCTAssertEqual(document["touch_keyboard_layout"] as? String, "eighteen_key")
    XCTAssertEqual((document["touch_keyboard_schemes"] as? [String: Any])?["selected"] as? String, "eighteen_key")
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .returnKey), .finishComposition)
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .modeSwitch), .finishComposition)
  }

  /// 双拼 14 键只改变字母键布局，不替换当前的小鹤、自然码或微软方案；布局字段缺省为 26 键。
  func testShuangpinLayoutIsIndependentFromTheSelectedProfile() {
    XCTAssertEqual(TouchShuangpinLayoutPreference.layout(in: nil), .twentySixKey)
    let document: [String: Any] = ["touch_shuangpin_layout": "fourteen_key", "shuangpin_profile": "microsoft"]
    XCTAssertEqual(TouchShuangpinLayoutPreference.layout(in: document), .fourteenKey)
    XCTAssertEqual(TouchShuangpinLayoutPreference.Layout(rawValue: "nine_key"), nil)
    XCTAssertEqual(TouchShuangpinLayoutPreference.layout(in: ["touch_shuangpin_layout": "eighteen_key"]), .eighteenKey)
    XCTAssertEqual(TouchShuangpinLayoutPreference.Layout.allCases.map(\.title), ["26 键", "14 键", "18 键"])
    let grids: [(TouchShuangpinLayoutPreference.Layout, LetterKeyGrid?)] = [(.twentySixKey, nil), (.fourteenKey, .fourteenKey), (.eighteenKey, .eighteenKey)]
    for (layout, grid) in grids {
      XCTAssertEqual(LetterKeyGrid.of(.microsoft, shuangpinLayout: layout, wubiLayout: .twentySixKey), grid, layout.rawValue)
      XCTAssertEqual(LetterKeyGrid.of(.microsoft, shuangpinLayout: layout, wubiLayout: .fourteenKey), grid,
                     "the wubi layout never applies to shuangpin")
      XCTAssertNil(LetterKeyGrid.of(.quanpin, shuangpinLayout: layout, wubiLayout: .fourteenKey),
                   "the shuangpin layout never applies to quanpin")
      XCTAssertNil(LetterKeyGrid.of(.wubi, shuangpinLayout: layout, wubiLayout: .twentySixKey),
                   "the shuangpin layout never applies to wubi")
    }

    var mapped = document
    MetasequoiaInputSessionBridge.schemeMapping(.microsoft, enabledSchemes: ChineseInputScheme.allCases)?(&mapped)
    XCTAssertEqual(mapped["scheme"] as? String, "shuangpin")
    XCTAssertEqual(mapped["shuangpin_profile"] as? String, "microsoft")
    XCTAssertEqual(mapped["touch_shuangpin_layout"] as? String, "fourteen_key")
  }

  /// 五笔 14 键是独立的 `touch_wubi_layout`：缺省 26 键，与双拼布局、86/98 码表互不影响；方案选择写文档时不碰它，所以切到别的方案再回到五笔仍是 14 键。
  func testWubiLayoutIsIndependentFromTheShuangpinLayoutAndTheProfile() throws {
    XCTAssertEqual(TouchWubiLayoutPreference.layout(in: nil), .twentySixKey)
    XCTAssertEqual(TouchWubiLayoutPreference.layout(in: [:]), .twentySixKey)
    XCTAssertEqual(TouchWubiLayoutPreference.layout(in: ["touch_wubi_layout": "eighteen_key"]), .twentySixKey,
                   "五笔没有 18 键")
    XCTAssertEqual(TouchWubiLayoutPreference.Layout.allCases.map(\.title), ["26 键", "14 键"])
    XCTAssertEqual(TouchWubiLayoutPreference.Layout.allCases.map(\.rawValue), ["twenty_six_key", "fourteen_key"])
    let document: [String: Any] = ["touch_wubi_layout": "fourteen_key", "touch_shuangpin_layout": "eighteen_key",
                                   "wubi_profile": "wubi98"]
    XCTAssertEqual(TouchWubiLayoutPreference.layout(in: document), .fourteenKey)
    XCTAssertEqual(TouchShuangpinLayoutPreference.layout(in: document), .eighteenKey)
    XCTAssertEqual(LetterKeyGrid.of(.wubi, shuangpinLayout: .eighteenKey, wubiLayout: .fourteenKey), .fourteenKey)
    XCTAssertNil(LetterKeyGrid.of(.wubi, shuangpinLayout: .fourteenKey, wubiLayout: .twentySixKey))
    for scheme in ChineseInputScheme.allCases where scheme != .wubi {
      XCTAssertEqual(LetterKeyGrid.of(scheme, shuangpinLayout: .twentySixKey, wubiLayout: .fourteenKey),
                     LetterKeyGrid.of(scheme, shuangpinLayout: .twentySixKey, wubiLayout: .twentySixKey),
                     "the wubi layout only applies to wubi: \(scheme)")
    }

    // 选别的方案、再选回五笔，都不改五笔布局和码表。
    var mapped = document
    for scheme in [ChineseInputScheme.quanpin, .microsoft, .wubi] {
      MetasequoiaInputSessionBridge.schemeMapping(scheme, enabledSchemes: ChineseInputScheme.allCases)?(&mapped)
      XCTAssertEqual(mapped["touch_wubi_layout"] as? String, "fourteen_key", scheme.rawValue)
      XCTAssertEqual(mapped["wubi_profile"] as? String, "wubi98", scheme.rawValue)
    }
    XCTAssertEqual(mapped["scheme"] as? String, "wubi")
    XCTAssertEqual(mapped["touch_keyboard_layout"] as? String, "twenty_six_key")

    // 存进共享文档：五笔布局与双拼布局、码表分开保存，换码表不丢 14 键。
    let previousWubi = TouchWubiLayoutPreference.layout
    let previousShuangpin = TouchShuangpinLayoutPreference.layout
    let previousProfile = WubiProfilePreference.profile
    defer {
      TouchWubiLayoutPreference.layout = previousWubi
      TouchShuangpinLayoutPreference.layout = previousShuangpin
      WubiProfilePreference.profile = previousProfile
    }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(TouchWubiLayoutPreference.layout(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)),
                   .twentySixKey, "a new document starts on 26 keys")
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.twentySixKey, stateRoot: state))
    XCTAssertTrue(TouchWubiLayoutPreference.save(.fourteenKey, stateRoot: state))
    XCTAssertEqual(TouchWubiLayoutPreference.layout, .fourteenKey, "the App Group mirror follows a saved document")
    for profile in ["wubi98", "wubi86"] {
      XCTAssertTrue(WubiProfilePreference.save(profile, stateRoot: state))
      let reloaded = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
      XCTAssertEqual(TouchWubiLayoutPreference.layout(in: reloaded), .fourteenKey, profile)
      XCTAssertEqual(TouchShuangpinLayoutPreference.layout(in: reloaded), .twentySixKey, profile)
    }
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.fourteenKey, stateRoot: state))
    XCTAssertEqual(TouchWubiLayoutPreference.layout(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)),
                   .fourteenKey)
    XCTAssertTrue(TouchWubiLayoutPreference.save(.twentySixKey, stateRoot: state))
    let back = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    XCTAssertEqual(TouchWubiLayoutPreference.layout(in: back), .twentySixKey)
    XCTAssertEqual(TouchShuangpinLayoutPreference.layout(in: back), .fourteenKey)
    // 镜像只认文档：没有文档时不动。
    TouchWubiLayoutPreference.mirror(nil)
    XCTAssertEqual(TouchWubiLayoutPreference.layout, .twentySixKey)
  }

  /// The cloud settings document knows quanpin, shuangpin, wubi, japanese and korean only; a device that uploaded any other `input.schema` would make every other device reject the whole document.
  func testTheCloudCarriesNoLanguageScheme() {
    for scheme in [ChineseInputScheme.cantonese, .zhuyin, .vietnamese, .tibetan, .stroke] {
      XCTAssertNil(scheme.cloudSchema, scheme.rawValue)
    }
    XCTAssertEqual(ChineseInputScheme.quanpin.cloudSchema, "quanpin")
    XCTAssertEqual(ChineseInputScheme.nineKey.cloudSchema, "quanpin")
    XCTAssertEqual(ChineseInputScheme.fourteenKey.cloudSchema, "quanpin")
    XCTAssertEqual(ChineseInputScheme.handwriting.cloudSchema, "quanpin")
    XCTAssertEqual(ChineseInputScheme.microsoft.cloudSchema, "shuangpin")
    XCTAssertEqual(ChineseInputScheme.wubi.cloudSchema, "wubi")
    XCTAssertEqual(ChineseInputScheme.japaneseNineKey.cloudSchema, "japanese")
    XCTAssertEqual(ChineseInputScheme.korean.cloudSchema, "korean")
  }

  func testSchemeTraits() {
    XCTAssertTrue(ChineseInputScheme.cantonese.hasSpellingCaret)
    XCTAssertFalse(ChineseInputScheme.cantonese.composesInPlace)
    XCTAssertFalse(ChineseInputScheme.cantonese.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.zhuyin.composesInPlace)
    XCTAssertFalse(ChineseInputScheme.zhuyin.hasSpellingCaret)
    XCTAssertFalse(ChineseInputScheme.zhuyin.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.vietnamese.composesInPlace)
    XCTAssertTrue(ChineseInputScheme.vietnamese.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.tibetan.composesInPlace)
    XCTAssertTrue(ChineseInputScheme.tibetan.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.tibetan.typesCasedLetters)
    XCTAssertFalse(ChineseInputScheme.tibetan.writesChinese)
    XCTAssertFalse(ChineseInputScheme.tibetan.hasSpellingCaret)
    XCTAssertFalse(ChineseInputScheme.tibetan.needsLanguageDictionary)
    XCTAssertTrue(ChineseInputScheme.optInSchemes.contains(.tibetan))
    XCTAssertEqual(ChineseInputScheme.allCases.filter(\.typesCasedLetters), [.vietnamese, .tibetan])
    XCTAssertFalse(ChineseInputScheme.japanese.writesAsciiPunctuation, "Japanese keeps its own marks")
    // 笔画在 Engine 里与粤拼同构（自己的只读词库，不用普通话那一套功能），但键面画成笔画字形，所以没有可移动光标的字母拼写。
    XCTAssertFalse(ChineseInputScheme.stroke.writesChinese)
    XCTAssertTrue(ChineseInputScheme.stroke.needsLanguageDictionary)
    XCTAssertFalse(ChineseInputScheme.stroke.hasSpellingCaret)
    XCTAssertFalse(ChineseInputScheme.stroke.composesInPlace)
    XCTAssertFalse(ChineseInputScheme.stroke.writesAsciiPunctuation)
    XCTAssertFalse(ChineseInputScheme.stroke.typesCasedLetters)
    XCTAssertTrue(ChineseInputScheme.stroke.drawsKeysAsGlyphs)
    XCTAssertNil(ChineseInputScheme.stroke.shuangpinProfile)
    XCTAssertFalse(ChineseInputScheme.stroke.editsBySyllable)
    for scheme in ChineseInputScheme.allCases where ![.cantonese, .cantoneseShuangpin, .zhuyin, .vietnamese, .tibetan, .stroke].contains(scheme) {
      XCTAssertFalse(scheme.needsLanguageDictionary, scheme.rawValue)
      XCTAssertEqual(scheme.composesInPlace, scheme.isKorean, scheme.rawValue)
      XCTAssertEqual(scheme.hasSpellingCaret, scheme.writesChinese, scheme.rawValue)
      XCTAssertFalse(scheme.drawsKeysAsGlyphs, scheme.rawValue)
    }
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.cantonese), "粤")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.zhuyin), "注")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.vietnamese), "越")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.quanpin), "中")
    XCTAssertEqual(KeyboardViewController.languageKeyValue(.vietnamese), "越南语输入")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.tibetan), "藏")
    XCTAssertEqual(KeyboardViewController.languageKeyValue(.tibetan), "藏文输入")
    XCTAssertEqual(ChineseInputScheme.tibetan.title, "藏文 26 键")
    XCTAssertEqual(ChineseInputScheme.tibetan.sharedIdentifier, "tibetan")
    XCTAssertEqual(TypingSource.tibetan.rawValue, "tibetan")
    XCTAssertEqual(TypingSource.tibetan.title, "藏文")
    XCTAssertEqual(KeyboardViewController.tibetanSpellingSymbols, ["'", "+", "-", ".", "/"])
    // 藏文的符号页把 `=` 键换成叠写用的 `+`，其他键和其他方案不变。
    XCTAssertEqual(KeyboardViewController.symbolRowKey("=", tibetan: true), "+")
    XCTAssertEqual(KeyboardViewController.symbolRowKey("=", tibetan: false), "=")
    XCTAssertEqual(KeyboardViewController.symbolRowKey("_", tibetan: true), "_")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.stroke), "笔")
    XCTAssertEqual(KeyboardViewController.languageKeyValue(.stroke), "笔画输入")
    XCTAssertEqual(TypingSource(rawValue: "stroke"), .stroke, "the keyboard counts by the scheme's raw value")
    XCTAssertEqual(TypingSource.stroke.title, "笔画")
    XCTAssertEqual(TypingSource.cantonese.rawValue, "cantonese")
    XCTAssertEqual(TypingSource.zhuyin.rawValue, "zhuyin")
    XCTAssertEqual(TypingSource.vietnamese.rawValue, "vietnamese")
  }

  // MARK: - Boundaries

  func testZhuyinAndVietnameseCommitAtEveryBoundary() {
    for scheme in [ChineseInputScheme.zhuyin, .vietnamese, .tibetan] {
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: false, scheme: scheme, boundary: .returnKey), .none)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .returnKey), .commitRaw)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .modeSwitch), .finishComposition)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .deactivate), .finishComposition)
    }
    for boundary in [CompositionBoundary.modeSwitch, .returnKey] {
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .cantonese, boundary: boundary), .commitRaw,
                     "Cantonese keeps its letters like pinyin")
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .stroke, boundary: boundary), .commitRaw,
                     "Return and the mode switch commit the typed stroke letters, as for Cantonese")
    }
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .stroke, boundary: .deactivate), .finishComposition)
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: false, scheme: .stroke, boundary: .returnKey), .none)
  }

  func testAnInPlaceCompositionIsMarkedWhateverThePreeditSetting() {
    XCTAssertEqual(InlineCompositionPolicy.markedText(
      inPlace: true, style: .off, phrasePrefix: "", preedit: "việt", editingText: "vieejt", japaneseReading: nil), "việt")
  }

  /// Stroke's `editing_text` is the letters its keys send, which the user never saw: 原始按键 marks the glyphs instead, and the other styles follow the setting as usual.
  func testAStrokeCompositionIsMarkedAsItsGlyphs() {
    let mark = { (style: InlinePreeditPreference.Style) in
      InlineCompositionPolicy.markedText(inPlace: false, style: style, drawsKeysAsGlyphs: true, phrasePrefix: "",
                                         preedit: "一丨＊", editingText: "hsx", japaneseReading: nil)
    }
    XCTAssertEqual(mark(.raw), "一丨＊")
    XCTAssertEqual(mark(.pinyin), "一丨＊")
    XCTAssertEqual(mark(.off), "")
    XCTAssertEqual(InlineCompositionPolicy.markedText(inPlace: false, style: .raw, phrasePrefix: "", preedit: "ni hao",
                                                      editingText: "nihao", japaneseReading: nil), "nihao",
                   "a spelled scheme still marks its keys")
  }

  // MARK: - Dachen keys

  func testEveryDachenKeyHasItsBopomofoOrTone() {
    let keys = ZhuyinKeyLayout.rows.flatMap { $0 }
    XCTAssertEqual(keys.count, 41)
    XCTAssertEqual(Set(keys).count, 41)
    let faces = keys.compactMap(ZhuyinKeyLayout.keycap(for:))
    XCTAssertEqual(faces.count, 41)
    XCTAssertEqual(Set(faces).count, 41)
    XCTAssertEqual(ZhuyinKeyLayout.keycap(for: "1"), "ㄅ")
    XCTAssertEqual(ZhuyinKeyLayout.keycap(for: "3"), "ˇ")
    XCTAssertEqual(ZhuyinKeyLayout.keycap(for: "/"), "ㄥ")
    XCTAssertNil(ZhuyinKeyLayout.keycap(for: "["))
    for digit in "123456789".map(String.init) { XCTAssertTrue(ZhuyinKeyLayout.selectsWhileListOpen(digit), digit) }
    for key in ["0", "-", "a", ";", ","] { XCTAssertFalse(ZhuyinKeyLayout.selectsWhileListOpen(key), key) }
  }

  func testTheDachenRowsReplaceTheLettersWhileZhuyinIsActive() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.zhuyin) == true,
                      "msime-zhuyin.db is not staged into the test host")
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .zhuyin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.viewWillAppear(false)
    let all = nodes(controller.view)
    let rows = all.filter { $0.accessibilityIdentifier == "zhuyinRow" }
    XCTAssertEqual(rows.count, 4)
    XCTAssertTrue(rows.allSatisfy { !$0.isHidden })
    let key = try XCTUnwrap(all.first { $0.accessibilityIdentifier == "zhuyinKeyq" } as? UIButton)
    XCTAssertEqual(key.accessibilityLabel, "注音 ㄆ")
  }

  func testTheZhuyinSymbolPanelFollowsTheChinesePunctuationSwitch() {
    XCTAssertEqual(KeyboardViewController.zhuyinSymbolText(",", chinesePunctuation: true), "，")
    XCTAssertEqual(KeyboardViewController.zhuyinSymbolText(".", chinesePunctuation: false), ".")
    XCTAssertEqual(KeyboardViewController.zhuyinSymbolText("@", chinesePunctuation: true), "@")
    XCTAssertTrue(KeyboardViewController.writesChinesePunctuation(switchOn: true, punctuationLock: nil))
    XCTAssertTrue(KeyboardViewController.writesChinesePunctuation(switchOn: true, punctuationLock: "follow"))
    XCTAssertFalse(KeyboardViewController.writesChinesePunctuation(switchOn: false, punctuationLock: "follow"))
    XCTAssertFalse(KeyboardViewController.writesChinesePunctuation(switchOn: true, punctuationLock: "english"))
    XCTAssertTrue(KeyboardViewController.writesChinesePunctuation(switchOn: false, punctuationLock: "chinese"))
  }

  // MARK: - Stroke keys

  func testTheStrokeKeysSendTheFiveStrokesAndTheWildcard() {
    let keys = StrokeKeyLayout.rows.flatMap { $0 }
    XCTAssertEqual(StrokeKeyLayout.rows.map(\.count), [3, 3], "a 2×3 grid")
    XCTAssertEqual(keys.map(\.ascii), ["h", "s", "p", "n", "z", "x"])
    XCTAssertEqual(keys.map(\.face), ["一", "丨", "丿", "丶", "乛", "＊"], "the glyphs the Engine draws in the preedit")
    XCTAssertEqual(keys.map(\.name), ["横", "竖", "撇", "点", "折", "通配"])
    XCTAssertEqual(StrokeKeyLayout.keycap(for: "z"), "乛")
    XCTAssertNil(StrokeKeyLayout.keycap(for: "a"))
    XCTAssertNil(StrokeKeyLayout.keycap(for: "*"), "the wildcard is a letter, never a punctuation mark")
    for key in ["h", "s", "p", "n", "z"] {
      XCTAssertTrue(StrokeKeyLayout.accepts(key, composing: false), key)
      XCTAssertTrue(StrokeKeyLayout.accepts(key, composing: true), key)
    }
    XCTAssertFalse(StrokeKeyLayout.accepts("x", composing: false), "an idle wildcard starts nothing")
    XCTAssertTrue(StrokeKeyLayout.accepts("x", composing: true))
    XCTAssertFalse(StrokeKeyLayout.accepts("a", composing: true))
  }

  func testTheStrokeKeypadDrawsTheGlyphsAndSendsTheLetters() throws {
    var sent: [String] = []
    let keypad = StrokeKeypadView(makeKey: { title, label, action in
      let button = UIButton(type: .system, primaryAction: UIAction { _ in action() })
      button.setTitle(title, for: .normal)
      button.accessibilityLabel = label
      return button
    })
    keypad.onStroke = { sent.append($0) }
    keypad.frame = CGRect(x: 0, y: 0, width: 280, height: 150)
    keypad.layoutIfNeeded()
    XCTAssertEqual(keypad.arrangedSubviews.count, 2)
    XCTAssertEqual(keypad.keyButtons.map { $0.title(for: .normal) }, ["一", "丨", "丿", "丶", "乛", "＊"])
    XCTAssertEqual(keypad.keyButtons.map(\.accessibilityIdentifier), ["strokeKeyh", "strokeKeys", "strokeKeyp", "strokeKeyn", "strokeKeyz", "strokeKeyx"])
    XCTAssertEqual(keypad.keyButtons.first?.accessibilityLabel, "笔画 横")
    let names = nodes(keypad).compactMap { $0 as? UILabel }.filter { $0.accessibilityIdentifier == "strokeKeyName" }
    XCTAssertEqual(names.map(\.text), ["横", "竖", "撇", "点", "折", "通配"])
    let heights = keypad.keyButtons.map(\.bounds.height)
    XCTAssertGreaterThan(heights.min() ?? 0, 0)
    XCTAssertEqual(heights.min() ?? 0, heights.max() ?? 0, accuracy: 1, "both rows share the height")
    for button in keypad.keyButtons { button.sendActions(for: .primaryActionTriggered) }
    XCTAssertEqual(sent, ["h", "s", "p", "n", "z", "x"])
    let wildcard = try XCTUnwrap(keypad.keyButtons.last)
    XCTAssertFalse(wildcard.isEnabled, "the wildcard waits for a stroke")
    keypad.setComposing(true)
    XCTAssertTrue(wildcard.isEnabled)
    XCTAssertTrue(keypad.keyButtons.dropLast().allSatisfy(\.isEnabled))
    keypad.setComposing(false)
    XCTAssertFalse(wildcard.isEnabled)
  }

  func testTheStrokeKeysReplaceTheLettersWhileStrokeIsActive() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.stroke) == true,
                      "msime-stroke.db is not staged into the test host")
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .stroke
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    let all = nodes(controller.view)
    let shown = { (view: UIView) in sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden } }
    let keypad = try XCTUnwrap(all.first { $0.accessibilityIdentifier == "strokeKeypad" })
    XCTAssertTrue(shown(keypad))
    XCTAssertFalse(all.contains { $0.accessibilityLabel == "字母 Q" && shown($0) }, "no letter keys")
    XCTAssertTrue(shown(try XCTUnwrap(all.first { $0.accessibilityIdentifier == "nineKeySidebar" })), "the nine-key punctuation sidebar stays")
    XCTAssertTrue(shown(try XCTUnwrap(all.first { $0.accessibilityIdentifier == "nineKeyDelete" })))
    let heng = try XCTUnwrap(all.first { $0.accessibilityIdentifier == "strokeKeyh" } as? UIButton)
    let wildcard = try XCTUnwrap(all.first { $0.accessibilityIdentifier == "strokeKeyx" } as? UIButton)
    XCTAssertFalse(wildcard.isEnabled)
    heng.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(wildcard.isEnabled, "a stroke opens the composition, so the wildcard can follow")
    // 右列中间的键是笔画键旁边的「重输」，与 Android 一致：它清掉已输入的笔画。
    let rewrite = try XCTUnwrap(all.first { $0.accessibilityIdentifier == "nineKeyMiddleKey" } as? UIButton)
    XCTAssertEqual(rewrite.configuration?.title, "重输")
    rewrite.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(wildcard.isEnabled, "nothing is composing after 重输")
  }

  // MARK: - Through the bridge

  func testAnIdleZhuyinToneKeyIsLeftToTheKeyboardToType() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.zhuyin) == true,
                      "msime-zhuyin.db is not staged into the test host")
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToZhuyin()
    for tone in ["6", "3", "4", "7"] {
      let snapshot = bridge.handleCharacter(tone)
      XCTAssertFalse(snapshot.isHandled, "the Engine leaves \(tone) to the keyboard, which inserts it")
      XCTAssertNil(snapshot.commitText, tone)
      XCTAssertEqual(snapshot.preedit, "", tone)
    }
    let pending = type("1", into: bridge)
    XCTAssertTrue(pending.isHandled)
    XCTAssertTrue(bridge.handleCharacter("6").isHandled, "a tone key on a pending syllable stays the Engine's")
  }


  func testZhuyinComposesInPlaceAndReturnCommitsTheConversion() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.zhuyin) == true,
                      "msime-zhuyin.db is not staged into the test host")
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToZhuyin()
    let composing = type("su3cl3", into: bridge)
    XCTAssertTrue(composing.isHandled)
    XCTAssertNil(composing.commitText)
    XCTAssertEqual(composing.preedit, "你好")
    let committed = bridge.commitRaw()
    XCTAssertTrue(committed.isHandled, "Return confirms the conversion, so no newline follows")
    XCTAssertEqual(committed.commitText, "你好")
  }

  /// Synthetic expectations that hold for any msime-stroke.db: 一 is the single stroke 横, and an idle wildcard is left to the host.
  func testStrokeLooksUpByStrokeOrderAndReturnCommitsTheLetters() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.stroke) == true,
                      "msime-stroke.db is not staged into the test host")
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToStroke()
    let idle = bridge.handleCharacter("x")
    XCTAssertFalse(idle.isHandled, "an idle wildcard starts nothing")
    XCTAssertEqual(idle.preedit, "")
    let one = bridge.handleCharacter("h")
    XCTAssertNil(one.diagnosticText)
    XCTAssertTrue(one.isHandled)
    XCTAssertEqual(one.preedit, "一")
    XCTAssertTrue(one.candidates.contains("一"), "\(one.candidates.prefix(5))")
    let wildcard = type("x", into: bridge)
    XCTAssertTrue(wildcard.isHandled)
    XCTAssertEqual(wildcard.preedit, "一＊")
    let swallowed = bridge.handleCharacter("a")
    XCTAssertTrue(swallowed.isHandled, "a letter that is no stroke is swallowed while composing")
    XCTAssertEqual(swallowed.preedit, "一＊")
    XCTAssertEqual(bridge.handleBackspace().preedit, "一")
    let committed = bridge.commitRaw()
    XCTAssertEqual(committed.commitText, "h", "Return commits the typed letters, as Cantonese does")
    XCTAssertEqual(committed.preedit, "")
  }

  func testVietnameseComposesTheWordAndTheFirstCancelRestoresTheKeys() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToVietnamese()
    let composing = type("vieejt", into: bridge)
    XCTAssertTrue(composing.isHandled)
    XCTAssertEqual(composing.preedit, "việt")
    XCTAssertTrue(composing.candidates.isEmpty)
    let restored = bridge.cancel()
    XCTAssertEqual(restored.preedit, "vieejt")
    XCTAssertEqual(bridge.cancel().preedit, "")
    _ = type("chaof", into: bridge)
    let committed = bridge.commitRaw()
    XCTAssertFalse(committed.isHandled, "Return still inserts its newline after the word")
    XCTAssertEqual(committed.commitText, "chào")
  }

  func testTibetanComposesWylieAndCommitsWithTshegOrShad() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToTibetan()
    let composing = type("bkra", into: bridge)
    XCTAssertTrue(composing.isHandled)
    XCTAssertNil(composing.commitText)
    XCTAssertEqual(composing.preedit, "བཀྲ")
    XCTAssertTrue(composing.candidates.isEmpty)
    // 空格（MSIME_COMMIT_CANDIDATE）上屏藏文并加音节点，以已处理返回，键盘不再插入空格。
    let tsheg = bridge.commitCandidate()
    XCTAssertTrue(tsheg.isHandled)
    XCTAssertEqual(tsheg.commitText, "བཀྲ་")
    _ = type("shis", into: bridge)
    let shad = bridge.handleCharacter("/")
    XCTAssertTrue(shad.isHandled)
    XCTAssertEqual(shad.commitText, "ཤིས།")
    let idleShad = bridge.handleCharacter("/")
    XCTAssertTrue(idleShad.isHandled, "没有组字时斜杠单独输入垂符")
    XCTAssertEqual(idleShad.commitText, "།")
  }

  func testTibetanReturnCommitsWithoutTshegAndUppercaseIsSpelling() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToTibetan()
    // 威利转写区分大小写：`Ta` 是反写的 ཊ，不是 ཏ。
    XCTAssertTrue(bridge.handleCharacter("T", shifted: true).isHandled)
    XCTAssertEqual(type("a", into: bridge).preedit, "ཊ")
    let committed = bridge.commitRaw()
    XCTAssertTrue(committed.isHandled, "回车只确认藏文，不换行")
    XCTAssertEqual(committed.commitText, "ཊ")
    // `'` 在没有组字时也能开头（achung），`+` 叠写。
    XCTAssertEqual(type("'od", into: bridge).preedit, "འོད")
    XCTAssertEqual(bridge.commitRaw().commitText, "འོད")
    XCTAssertEqual(type("pad+ma", into: bridge).preedit, "པདྨ")
    XCTAssertEqual(bridge.commitRaw().commitText, "པདྨ")
  }

  func testTibetanFirstCancelRestoresTheWylie() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToTibetan()
    _ = type("bkra", into: bridge)
    XCTAssertEqual(bridge.cancel().preedit, "bkra")
    XCTAssertEqual(bridge.cancel().preedit, "")
  }

  func testTibetanNeverReplacesTheRememberedChineseScheme() {
    var document: [String: Any] = ["last_chinese_scheme": "shuangpin"]
    MetasequoiaInputSessionBridge.schemeMapping(.tibetan, enabledSchemes: [.quanpin, .tibetan])?(&document)
    XCTAssertEqual(document["scheme"] as? String, "tibetan")
    XCTAssertEqual(document["last_chinese_scheme"] as? String, "shuangpin")
  }
}

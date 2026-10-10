import XCTest
import UIKit
import Darwin

@MainActor
final class NineKeyKeyboardTests: XCTestCase {
  func testSharedFrequencyPreferenceRejectsFractionalAndBooleanCounts() {
    XCTAssertNil(KeyboardViewController.sharedPreferenceInt(NSNumber(value: 2.5), range: 1...10))
    XCTAssertNil(KeyboardViewController.sharedPreferenceInt(NSNumber(value: true), range: 1...10))
    XCTAssertEqual(KeyboardViewController.sharedPreferenceInt(NSNumber(value: 4), range: 1...10), 4)
  }

  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to
  // whatever the app group was left holding. See InputSchemeTestSupport.
  private var savedKeyboardPreferences: [String: Any] = [:]
  private let preferenceKeys = [KeyboardLayoutPreference.keySpacingKey,
    KeyboardLayoutPreference.rowSpacingKey, KeyboardLayoutPreference.heightAdjustmentKey,
    KeyboardLayoutPreference.voiceShortcutKey, KeyboardLayoutPreference.numberKeypadOrderKey,
    KeyboardLayoutPreference.twentySixKeyNumberLayoutKey]
  override func tearDown() {
    for key in preferenceKeys {
      if let value = savedKeyboardPreferences[key] { KeyboardLayoutPreference.defaults.set(value, forKey: key) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: key) }
    }
    super.tearDown()
  }
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    savedKeyboardPreferences = [:]
    for key in preferenceKeys {
      savedKeyboardPreferences[key] = KeyboardLayoutPreference.defaults.object(forKey: key)
      KeyboardLayoutPreference.defaults.removeObject(forKey: key)
    }
  }

  func testLayoutKeepsKeysInBoundsAcrossBothKeyboards() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    for scheme in [ChineseInputScheme.quanpin, .nineKey] {
      InputSchemePreference.scheme = scheme
      for width in [320.0, 414.0] {
        let controller = KeyboardViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
        controller.view.layoutIfNeeded()
        let space = try button("spaceKey", in: controller)
        let enter = try button("returnKey", in: controller)
        XCTAssertGreaterThanOrEqual(space.bounds.width, 43.5, "\(scheme) / \(width)")
        XCTAssertEqual(
          controller.view.bounds.height, KeyboardViewController.defaultKeyboardHeight,
          accuracy: 0.5)
        XCTAssertLessThanOrEqual(enter.convert(enter.bounds, to: controller.view).maxX, width)
        let language = try button("bottomLanguageKey", in: controller)
        XCTAssertFalse(language.isHidden)
        if width == 414 {
          let renderer = UIGraphicsImageRenderer(bounds: controller.view.bounds)
          let screenshot = renderer.image { context in controller.view.layer.render(in: context.cgContext) }
          let attachment = XCTAttachment(image: screenshot)
          attachment.name = "Layout-\(scheme.rawValue)"
          attachment.lifetime = .keepAlways
          add(attachment)
        }
      }
    }
  }

  func testNineKeysCarryTheirLettersAndAHoldGesture() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)

    let expected = [2: "ABC", 3: "DEF", 4: "GHI", 5: "JKL", 6: "MNO", 7: "PQRS", 8: "TUV", 9: "WXYZ"]
    for (digit, letters) in expected {
      let key = try button("nineKey\(digit)", in: controller)
      XCTAssertEqual(key.configuration?.title, letters)
      XCTAssertEqual(key.tag, digit)
      XCTAssertEqual(
        (key.gestureRecognizers ?? []).compactMap { $0 as? UILongPressGestureRecognizer }.count, 1)
    }
    // 1 键是设计稿里的 @#，点开符号面板：没有长按菜单，角上也没有数字。
    let symbols = try button("nineKey1", in: controller)
    XCTAssertEqual(symbols.configuration?.title, "@#")
    XCTAssertEqual(symbols.accessibilityLabel, "符号")
    XCTAssertTrue((symbols.gestureRecognizers ?? []).compactMap { $0 as? UILongPressGestureRecognizer }.isEmpty)
    XCTAssertTrue(symbols.subviews.allSatisfy { $0.accessibilityIdentifier != "keyNumberHint" })
    // 分词键移到了右列，位于 ⌫ 和 ！ 之间，与 Android 一致。
    XCTAssertEqual(try button("nineKeyMiddleKey", in: controller).configuration?.title, "分词")
    XCTAssertEqual(try button("nineKeyMiddleKey", in: controller).accessibilityLabel, "拼音分词")
    XCTAssertEqual(try button("nineKeyClosingMark", in: controller).configuration?.title, "！")
  }

  /// ，。？ 竖排在网格左侧；⌫、分词 和 ！ 竖排在右侧，各自对齐网格的一行；0 在底行。
  func testNineKeyColumnsLineUpWithTheGridRows() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    func frame(_ view: UIView) -> CGRect { view.convert(view.bounds, to: controller.view) }
    let sidebar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "nineKeySidebar" })
    let marks = descendants(sidebar).compactMap { ($0 as? UIButton)?.configuration?.title }
    XCTAssertEqual(marks, KeyboardViewController.nineKeySidebarMarks)
    XCTAssertEqual(marks, ["，", "。", "？"])
    let right = try ["nineKeyDelete", "nineKeyMiddleKey", "nineKeyClosingMark"].map { try button($0, in: controller) }
    for (index, key) in right.enumerated() {
      let rowKey = try button("nineKey\(index * 3 + 3)", in: controller)
      XCTAssertEqual(frame(key).midY, frame(rowKey).midY, accuracy: 0.5, key.accessibilityIdentifier ?? "")
      XCTAssertGreaterThan(frame(key).minX, frame(rowKey).maxX)
    }
    let zero = try button("nineKeyZero", in: controller)
    XCTAssertFalse(zero.isHidden)
    XCTAssertEqual(zero.configuration?.title, "0")
    XCTAssertGreaterThan(frame(zero).minY, frame(try button("nineKey8", in: controller)).maxY)
  }

  func testNineKeyMiddleKeyFollowsTheSurface() {
    typealias Key = KeyboardViewController.NineKeyMiddleKey
    XCTAssertEqual(Key.resolve(stroke: false, digits: false), .separator)
    XCTAssertEqual(Key.resolve(stroke: false, digits: true), .period)
    XCTAssertEqual(Key.resolve(stroke: true, digits: false), .rewrite)
    XCTAssertEqual(Key.separator.face, "分词")
    XCTAssertEqual(Key.period.face, ".")
    XCTAssertEqual(Key.rewrite.face, "重输")
  }

  /// 右列的分词键仍能切分拼写：依次输入 94、分词、26，仍可选到 xi'an（西安）。
  func testNineKeySeparatorMarksASyllableBoundary() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    for digit in "94" { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
    try button("nineKeyMiddleKey", in: controller).sendActions(for: .primaryActionTriggered)
    for digit in "26" { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
    // 读音行显示 Engine 的拼音读音（`nine_key_reading`），不再是数字串，所以看读音：分词处断开，第一个音节正好是 94 的两个字母。
    let reading = try button("preeditButton", in: controller).configuration?.title ?? ""
    XCTAssertTrue(reading.contains("'"), reading)
    XCTAssertEqual(reading.split(separator: "'").first?.count, 2, "the separator reaches the composition: \(reading)")
  }

  func testNineKeyDigitLayerKeepsTheGridAndRestoresLetters() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)

    XCTAssertEqual(try button("nineKey2", in: controller).configuration?.title, "ABC")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    for digit in 1...9 {
      XCTAssertEqual(try button("nineKey\(digit)", in: controller).configuration?.title, String(digit))
    }
    XCTAssertTrue(descendants(controller.view).filter {
      $0.accessibilityLabel == "符号 1" && !($0.superview?.isHidden ?? true)
    }.isEmpty)
    XCTAssertTrue(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "symbolLayerRow0" }).isHidden,
                  "the grid keeps its digits instead of the 123 layer")
    // 数字层上分词键没有可切分的内容；该键在那里输入句号。
    XCTAssertEqual(try button("nineKeyMiddleKey", in: controller).configuration?.title, ".")
    XCTAssertFalse(try button("nineKeyZero", in: controller).isHidden)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("nineKey2", in: controller).configuration?.title, "ABC")
    XCTAssertEqual(try button("nineKey1", in: controller).configuration?.title, "@#")
    XCTAssertEqual(try button("nineKeyMiddleKey", in: controller).configuration?.title, "分词")
  }

  func testLayoutPreferencePreservesActiveComposition() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    key.sendActions(for: .primaryActionTriggered)
    let before = try button("preeditButton", in: controller).configuration?.title
    KeyboardLayoutPreference.keySpacing = 4
    controller.viewWillAppear(false)
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, before)
    XCTAssertFalse(try button("bottomLanguageKey", in: controller).isHidden)
  }

  func testTouchSchemeWritesCanonicalEngineAndPresentationMapping() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-touch-scheme-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.setTouchKeyboardScheme(
      .microsoft, enabledSchemes: [.quanpin, .microsoft, .japaneseNineKey]))
    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["scheme"] as? String, "shuangpin")
    XCTAssertEqual(preferences["last_chinese_scheme"] as? String, "shuangpin")
    XCTAssertEqual(preferences["shuangpin_profile"] as? String, "microsoft")
    XCTAssertEqual(preferences["touch_keyboard_layout"] as? String, "twenty_six_key")
    let schemes = try XCTUnwrap(preferences["touch_keyboard_schemes"] as? [String: Any])
    XCTAssertEqual(schemes["enabled"] as? [String], ["quanpin", "microsoft", "japanese_nine_key"])
    XCTAssertEqual(schemes["selected"] as? String, "microsoft")
  }

  func testKeyboardThemeWritesCanonicalPreference() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-touch-skin-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.updateTheme(GlobalThemePreference.selecting("night")))
    var preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["global_theme"] as? String, "night")
    // A design picked on the keyboard moves to the custom theme over the theme it replaces.
    let design = CustomKeyboardSkin.templates[2].1
    XCTAssertTrue(bridge.updateTheme(try XCTUnwrap(GlobalThemePreference.applyingDesign(design))))
    preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["global_theme"] as? String, "custom")
    XCTAssertEqual(GlobalThemePreference.base(in: preferences), "night")
    XCTAssertEqual(GlobalThemePreference.design(in: preferences), design.normalized)
    // An id the catalog does not list is refused and changes nothing.
    XCTAssertFalse(bridge.updateTheme(GlobalThemePreference.selecting("midnight")))
    XCTAssertEqual(try XCTUnwrap(bridge.sharedPreferences)["global_theme"] as? String, "custom")
  }

  func testTraditionalOutputWritesCanonicalPreference() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-traditional-output-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.setTraditionalChineseOutput(true))
    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["traditional_chinese_output"] as? Bool, true)
  }

  func testFuzzyPreferencesWaitForIdleAndSurviveSchemeRebuild() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-fuzzy-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setFuzzyPinyinRules(1))
    _ = bridge.handleCharacter("z")
    XCTAssertFalse(bridge.setFuzzyPinyinRules(0))
    _ = bridge.cancel()
    XCTAssertTrue(bridge.setFuzzyPinyinRules(0))
    XCTAssertTrue(bridge.setFuzzyPinyinRules(1))
    _ = bridge.switchToNineKey()
    _ = bridge.switch(toShuangpinProfile: "xiaohe")
    _ = bridge.handleCharacter("z")
    let view = bridge.handleCharacter("s")
    let rows = try XCTUnwrap(try bridge.allCandidates()["candidates"] as? [[String: Any]])
    XCTAssertTrue(rows.contains { $0["text"] as? String == "中" }, String(describing: view.candidates))
    XCTAssertFalse(bridge.setFuzzyPinyinRules(0))
    _ = bridge.cancel()
    XCTAssertTrue(bridge.setFuzzyPinyinRules(0))
  }

  func testThoughtfulReplyIsNoLongerAnInputScheme() throws {
    let defaults = try XCTUnwrap(UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier))
    let previousEnabled = defaults.object(forKey: InputSchemePreference.enabledSchemesKey)
    let previousScheme = defaults.object(forKey: "chineseInputScheme")
    defer {
      defaults.set(previousEnabled, forKey: InputSchemePreference.enabledSchemesKey)
      defaults.set(previousScheme, forKey: "chineseInputScheme")
    }
    XCTAssertNil(ChineseInputScheme(rawValue: "thoughtfulReply"))
    XCTAssertNil(ChineseInputScheme.scheme(sharedIdentifier: "thoughtful_reply"))
    XCTAssertFalse(ChineseInputScheme.allCases.map(\.title).contains("高情商回复"))
    // 旧版存下的选择和启用列表：选择回退到全拼 26 键，启用列表里直接忽略。
    defaults.set(["quanpin", "thoughtfulReply"], forKey: InputSchemePreference.enabledSchemesKey)
    defaults.set("thoughtfulReply", forKey: "chineseInputScheme")
    XCTAssertEqual(InputSchemePreference.enabledSchemes, [.quanpin])
    XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
    defaults.set(["thoughtfulReply"], forKey: InputSchemePreference.enabledSchemesKey)
    XCTAssertEqual(InputSchemePreference.enabledSchemes, [.quanpin])
  }

  func testReplyToolOpensReplyKeyboardOverAnySchemeAndTheBrandClosesIt() throws {
    let defaults = try XCTUnwrap(UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier))
    let previousEnabled = defaults.object(forKey: InputSchemePreference.enabledSchemesKey)
    let previous = InputSchemePreference.scheme
    defer {
      defaults.set(previousEnabled, forKey: InputSchemePreference.enabledSchemesKey)
      InputSchemePreference.scheme = previous
    }
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey]
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let hasReply = { self.descendants(controller.view).contains { $0.accessibilityIdentifier == "replyKeyboard" } }
    // 高情商回复 移进了功能菜单；旧的工具栏按钮仍参与排布，但从不显示。
    XCTAssertTrue(try button("replyShortcut", in: controller).isHidden)
    XCTAssertFalse(hasReply())
    let brand = try XCTUnwrap(try button("moreShortcut", in: controller) as? KeyboardBrandMarkButton)
    let openReply = {
      brand.sendActions(for: .primaryActionTriggered)
      try self.tile("moreCard-高情商回复", in: controller).sendActions(for: .primaryActionTriggered)
    }

    try openReply()
    XCTAssertTrue(hasReply())
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" },
                   "the menu closes before the reply panel opens")
    XCTAssertTrue(brand.isActive, "the brand is highlighted while the reply panel covers the keys")
    let reply = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "replyKeyboard" })
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    controller.view.layoutIfNeeded()
    XCTAssertGreaterThanOrEqual(reply.convert(reply.bounds, to: controller.view).minY,
                                strip.convert(strip.bounds, to: controller.view).maxY - 0.5)
    XCTAssertFalse(reply.accessibilityViewIsModal, "the toolbar above stays reachable")
    // 面板不是模态的，所以被它盖住的按键行要自己从 VoiceOver 里藏起来，否则能聚焦到看不见的按键。
    XCTAssertTrue(coveredByAPanel(try button("nineKey6", in: controller)), "the keys under the reply panel are hidden from VoiceOver and touch")
    // 面板只是盖在键区上，方案不变。
    XCTAssertEqual(InputSchemePreference.scheme, .nineKey)
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 9 键")

    // 点品牌标志关闭它，回到九键键盘。
    brand.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(hasReply())
    XCTAssertFalse(brand.isActive)
    XCTAssertFalse(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden)
    XCTAssertFalse(coveredByAPanel(try button("nineKey6", in: controller)), "closing the reply panel gives the keys back")

    // 键盘收起时面板一起关掉，下次出现是原来的键盘。
    try openReply()
    XCTAssertTrue(hasReply())
    controller.viewWillDisappear(false)
    controller.viewWillAppear(false)
    XCTAssertFalse(hasReply())
    XCTAssertFalse(coveredByAPanel(try button("nineKey6", in: controller)))

    // 键区面板替换回复面板时按键行仍然盖着，由新面板接手。
    try openReply()
    try button("emojiShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(hasReply())
    XCTAssertTrue(coveredByAPanel(try button("nineKey6", in: controller)), "the key-area panel that replaced the reply panel still covers the keys")
  }

  /// 工具栏的收起键在面板盖住按键时先回到键盘，与 Android 的收起键一致；它的读法随之在「返回键盘」和「收起键盘」之间切换。
  func testDismissKeyReturnsToTheKeysWhileAPanelCoversThem() throws {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let hasPicker = { self.descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardEmojiPicker" } }
    let dismiss = try button("dismissShortcut", in: controller)
    XCTAssertEqual(dismiss.accessibilityLabel, "收起键盘")

    try button("emojiShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(hasPicker())
    XCTAssertEqual(dismiss.accessibilityLabel, "返回键盘")
    dismiss.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(hasPicker(), "the first tap closes the panel instead of putting the keyboard away")
    XCTAssertFalse(coveredByAPanel(try button("spaceKey", in: controller)), "the keys are live again")
    XCTAssertEqual(dismiss.accessibilityLabel, "收起键盘")

    // 回复面板同样先回到键盘。
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    try tile("moreCard-高情商回复", in: controller).sendActions(for: .primaryActionTriggered)
    let hasReply = { self.descendants(controller.view).contains { $0.accessibilityIdentifier == "replyKeyboard" } }
    XCTAssertTrue(hasReply())
    XCTAssertEqual(dismiss.accessibilityLabel, "返回键盘")
    dismiss.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(hasReply())
    XCTAssertEqual(dismiss.accessibilityLabel, "收起键盘")
  }

  func testDisabledSchemesAreHiddenAndCurrentSchemeFallsBack() throws {
    let defaults = try XCTUnwrap(UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier))
    let previousEnabled = defaults.object(forKey: InputSchemePreference.enabledSchemesKey)
    let previousScheme = InputSchemePreference.scheme
    defer {
      defaults.set(previousEnabled, forKey: InputSchemePreference.enabledSchemesKey)
      InputSchemePreference.scheme = previousScheme
    }
    defaults.removeObject(forKey: InputSchemePreference.enabledSchemesKey)
    XCTAssertEqual(InputSchemePreference.enabledSchemes, ChineseInputScheme.allCases.filter { !ChineseInputScheme.optInSchemes.contains($0) })
    InputSchemePreference.scheme = .japanese
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    InputSchemePreference.enabledSchemes = [.nineKey, .wubi]
    XCTAssertEqual(InputSchemePreference.scheme, .nineKey)
    controller.viewWillAppear(false)
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 9 键")
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    let cards = descendants(controller.view).compactMap(\.accessibilityIdentifier).filter { $0.hasPrefix("schemeCard-") }
    XCTAssertEqual(Set(cards), ["schemeCard-nineKey", "schemeCard-wubi"])
    try button("schemeCard-wubi", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(InputSchemePreference.scheme, .wubi)
    InputSchemePreference.enabledSchemes = []
    XCTAssertEqual(InputSchemePreference.enabledSchemes, [.quanpin])
    XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
  }

  func testFieldLanguagePreferenceIsTemporaryAndRespectsManualChoice() {
    var context = KeyboardInputContext()
    let ordinary = UUID(), code = UUID(), url = UUID()
    XCTAssertNil(context.languageOverride(for: .default, document: ordinary, isChinese: true))
    XCTAssertEqual(context.languageOverride(for: .asciiCapable, document: code, isChinese: true), false)
    // A user explicitly switched to Chinese in this field; callbacks must not force English again.
    XCTAssertNil(context.languageOverride(for: .asciiCapable, document: code, isChinese: true))
    XCTAssertEqual(context.languageOverride(for: .URL, document: url, isChinese: true), false)
    XCTAssertEqual(context.languageOverride(for: .default, document: ordinary, isChinese: false), true)
    XCTAssertEqual(context.languageOverride(for: .emailAddress, document: code, isChinese: false), false)
    XCTAssertEqual(context.languageOverride(for: .default, document: ordinary, isChinese: false), false)
    XCTAssertNil(context.languageOverride(for: .webSearch, document: UUID(), isChinese: true))
    XCTAssertNil(context.languageOverride(for: .default, document: UUID(), isChinese: true))
  }

  func testLatinFieldsUseFullKeyboardAndRestoreNineKeyHeight() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      let ordinary = UUID()
      controller.applyInputContext(keyboardType: .default, documentIdentifier: ordinary)
      for type in [UIKeyboardType.asciiCapable, .emailAddress, .URL] {
        // Any preedit left after a missed focus callback must not enter the new field.
        try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
        let field = UUID()
        controller.applyInputContext(keyboardType: type, documentIdentifier: field)
        controller.view.layoutIfNeeded()
        XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "英文输入")
        XCTAssertTrue(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden)
        XCTAssertFalse(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" }).isHidden)
        let q = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" } as? UIButton)
        XCTAssertFalse(try XCTUnwrap(q.superview).isHidden)
        XCTAssertEqual(q.bounds.height, try button("returnKey", in: controller).bounds.height, accuracy: 0.5)
        XCTAssertGreaterThanOrEqual(q.bounds.height, KeyboardHeightPercent.portraitKeyHeight(tablet: false) - 0.5)
        if type != .asciiCapable { XCTAssertEqual(q.configuration?.title, "q") }
        XCTAssertEqual(try button("quickPunctuationKey", in: controller).configuration?.title, ",")
        q.sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "英文输入")
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        for symbol in ["@", "/", "_", "="] {
          // @ 和 / 在 123 层，_ 和 = 在 #+= 层。
          if symbol == "_" {
            try button("symbolLayerToggle", in: controller).sendActions(for: .primaryActionTriggered)
            controller.view.layoutIfNeeded()
          }
          let key = try XCTUnwrap(shownSymbolKey(symbol, in: controller), symbol)
          XCTAssertGreaterThan(key.bounds.width, 20)
        }
        controller.applyInputContext(keyboardType: type, documentIdentifier: field)
        controller.applyInputContext(keyboardType: .default, documentIdentifier: ordinary)
        controller.view.layoutIfNeeded()
        XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "中文输入")
        XCTAssertFalse(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden)
        XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 9 键")
        XCTAssertEqual(controller.view.bounds.height, KeyboardViewController.defaultKeyboardHeight)
      }
    }
  }

  func testCursorDragCannotResumeAfterDocumentChangeOrCancellation() {
    var movement = SpaceCursorMovement()
    let first = UUID(), second = UUID()
    movement.begin(at: 0, document: first)
    XCTAssertEqual(movement.advance(to: 24, document: first), 2)
    XCTAssertEqual(movement.advance(to: 48, document: second), 0)
    XCTAssertFalse(movement.isActive)
    XCTAssertEqual(movement.advance(to: 60, document: first), 0)
    movement.begin(at: 0, document: second)
    movement.cancel()
    XCTAssertEqual(movement.advance(to: 24, document: second), 0)
    movement.begin(at: 0, document: second)
    XCTAssertEqual(movement.advance(to: .greatestFiniteMagnitude, document: second), 0)
    XCTAssertFalse(movement.isActive)
    movement.begin(at: .nan, document: first)
    XCTAssertFalse(movement.isActive)
  }

  func testSpaceCursorMovementAccumulatesDistanceAndReverses() throws {
    var movement = SpaceCursorMovement()
    let document = UUID()
    movement.begin(at: 10, document: document)
    XCTAssertEqual(movement.advance(to: 15, document: document), 0)
    XCTAssertEqual(movement.advance(to: 34, document: document), 2)
    XCTAssertEqual(movement.advance(to: 30, document: document), 0)
    XCTAssertEqual(movement.advance(to: 22, document: document), -1)
    movement.begin(at: -20, document: document)
    XCTAssertEqual(movement.advance(to: -31, document: document), 0)
    XCTAssertEqual(movement.advance(to: -44, document: document), -2)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let space = try button("spaceKey", in: controller)
    XCTAssertEqual(space.accessibilityCustomActions?.map(\.name), ["光标左移", "光标右移", "语音输入"])
    let pan = try XCTUnwrap(space.gestureRecognizers?.first { $0.name == "spaceCursorPan" } as? UIPanGestureRecognizer)
    XCTAssertTrue(pan.cancelsTouchesInView)
    XCTAssertEqual(pan.maximumNumberOfTouches, 1)
  }

  func testChangingHapticStrengthReplacesTheViewGenerator() throws {
    guard #available(iOS 17.5, *) else { throw XCTSkip("View-bound haptics require iOS 17.5") }
    let defaults = KeyboardFeedbackPreference.defaults
    let keys = [KeyboardFeedbackPreference.hapticsKey, KeyboardFeedbackPreference.strengthKey]
    let previous = keys.map { defaults.object(forKey: $0) }
    defer {
      for (key, value) in zip(keys, previous) {
        if let value { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
      }
    }
    defaults.set(true, forKey: KeyboardFeedbackPreference.hapticsKey)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    var last: UIImpactFeedbackGenerator?
    for strength in KeyboardHapticStrength.allCases {
      defaults.set(strength.rawValue, forKey: KeyboardFeedbackPreference.strengthKey)
      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      let generators = controller.view.interactions.compactMap { $0 as? UIImpactFeedbackGenerator }
      XCTAssertEqual(generators.count, 1)
      let current = try XCTUnwrap(generators.first)
      if let last { XCTAssertFalse(last === current) }
      last = current
    }
  }

  /// 三档之间要拉得开：原先中、强两档的力度都是 1.0，单次轻敲分不出来。「跟随系统」不套力度，存了认不得的值按「中」。
  func testHapticStrengthLevelsAreDistinctAndSystemKeepsTheDefaultImpact() {
    XCTAssertEqual(KeyboardHapticStrength.allCases.map(\.title), ["跟随系统", "轻", "中", "强"])
    XCTAssertNil(KeyboardHapticStrength.system.intensity)
    let levels: [KeyboardHapticStrength] = [.light, .medium, .strong]
    XCTAssertEqual(levels.map(\.style), [.light, .medium, .heavy])
    let intensities = levels.compactMap(\.intensity)
    XCTAssertEqual(intensities.count, 3)
    for (lower, higher) in zip(intensities, intensities.dropFirst()) {
      XCTAssertGreaterThan(higher - lower, 0.15)
    }
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.object(forKey: KeyboardFeedbackPreference.strengthKey)
    defer {
      if let previous { defaults.set(previous, forKey: KeyboardFeedbackPreference.strengthKey) }
      else { defaults.removeObject(forKey: KeyboardFeedbackPreference.strengthKey) }
    }
    defaults.set("thunderous", forKey: KeyboardFeedbackPreference.strengthKey)
    XCTAssertEqual(KeyboardFeedbackPreference.hapticStrength, .medium)
    defaults.removeObject(forKey: KeyboardFeedbackPreference.strengthKey)
    XCTAssertEqual(KeyboardFeedbackPreference.hapticStrength, .medium)
    defaults.set("system", forKey: KeyboardFeedbackPreference.strengthKey)
    XCTAssertEqual(KeyboardFeedbackPreference.hapticStrength, .system)
  }

  func testCandidateManagementMenuUsesEngineSupportedLayouts() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.quanpin, .nineKey] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      for character in (scheme == .nineKey ? "64426" : "nihao") {
        if scheme == .nineKey {
          try button("nineKey\(character)", in: controller).sendActions(for: .primaryActionTriggered)
        } else {
          let key = try XCTUnwrap(descendants(controller.view).first {
            $0.accessibilityLabel == "字母 \(String(character).uppercased())"
          } as? UIButton)
          key.sendActions(for: .primaryActionTriggered)
        }
      }
      let candidate = try button("candidate-1", in: controller)
      XCTAssertTrue(candidate.menu?.children.first is UIDeferredMenuElement)
      // 你好 has two characters, so 以词定字 leads the menu while the shared `word_character.enabled` default is on. It is not pinned, so there is no 取消固定 and no slot is checked.
      XCTAssertEqual(controller.candidateMenuElements(at: 0).map(\.title),
                     ["以词定字", "优先显示", "固定排位", "删除词条…"])
      let slots = try XCTUnwrap((controller.candidateMenuElements(at: 0)[2] as? UIMenu)?.children as? [UIAction])
      XCTAssertEqual(slots.map(\.title), ["第 1 位", "第 2 位", "第 3 位", "第 4 位", "第 5 位"])
      XCTAssertTrue(slots.allSatisfy { $0.state == .off })
      XCTAssertEqual((controller.candidateMenuElements(at: 0).last as? UIMenu)?.children.first?.title,
                     "确认删除此词条")
    }
  }

  func testSkinCardsOpenInTheKeyAreaAndApplyWithoutChangingKeyboardHeight() throws {
    preserveSharedTheme()
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      let skinShortcut = try XCTUnwrap(try button("skinShortcut", in: controller) as? KeyboardToolbarButton)
      skinShortcut.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let picker = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSkinPicker" })
      // 选择器替换工具栏下方的按键，工具栏保持可见且 皮肤 高亮；选择器自身没有标题栏。
      try assertInKeyArea(picker, of: controller)
      XCTAssertTrue(skinShortcut.isActive)
      XCTAssertNil(descendants(picker).first { $0.accessibilityIdentifier == "closeSkinPicker" && !$0.isHidden })
      for id in GlobalThemeCatalog.ids {
        let card = try button("skinCard-\(id)", in: controller)
        XCTAssertNotNil(descendants(card).compactMap { $0 as? KeyboardSkinMiniature }.first, id)
      }
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Skin cards \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      try button("skinCard-night", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(GlobalThemePreference.selected, "night")
      XCTAssertEqual(MetasequoiaInputSessionBridge.loadSharedPreferences()?["global_theme"] as? String, "night")
      XCTAssertNil(picker.superview)
      XCTAssertFalse(skinShortcut.isActive)
      XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, KeyboardViewController.defaultKeyboardHeight)
      skinShortcut.sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(try button("skinCard-night", in: controller).accessibilityValue, "已选中")
      // 再点一次高亮图标会关闭选择器，按键回来。
      skinShortcut.sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardSkinPicker" })
      XCTAssertEqual(try button("spaceKey", in: controller).superview?.alpha, 1)
    }
  }

  func testLayoutSettingsDefaultIndependently() {
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 6)
    XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, 7)
    XCTAssertFalse(KeyboardLayoutPreference.voiceShortcutEnabled)
    XCTAssertEqual(KeyboardLayoutPreference.geometry.sidebarRatio, 0.14)
    XCTAssertFalse(KeyboardLayoutPreference.geometry.showsFullKeyboardSymbols)
    KeyboardLayoutPreference.keySpacing = 5
    KeyboardLayoutPreference.voiceShortcutEnabled = true
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 5)
    XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, 7)
    XCTAssertTrue(KeyboardLayoutPreference.voiceShortcutEnabled)
    KeyboardLayoutPreference.keySpacing = 99
    KeyboardLayoutPreference.rowSpacing = -99
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 6)
    XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, 4)
  }

  func testKeyboardHeightFollowsTheSetting() throws {
    let previous = KeyboardLayoutPreference.heightAdjustment
    defer { KeyboardLayoutPreference.heightAdjustment = previous }

    func height(for adjustment: Double) -> CGFloat {
      KeyboardLayoutPreference.heightAdjustment = adjustment
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)
      controller.view.layoutIfNeeded()
      return controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant ?? 0
    }

    let standard = height(for: 0)
    XCTAssertGreaterThan(standard, 0)
    XCTAssertEqual(height(for: 24), standard + 24, accuracy: 0.5)
    XCTAssertEqual(height(for: -12), standard - 12, accuracy: 0.5)

    KeyboardLayoutPreference.heightAdjustment = 500
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 48)
    KeyboardLayoutPreference.heightAdjustment = -500
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, -12)
  }

  func testSymbolKeysShowThePunctuationTheyInsert() throws {
    XCTAssertEqual(KeyboardViewController.chineseSymbolFaces, [
      ",": "，", ".": "。", "?": "？", "!": "！", ";": "；", ":": "：",
      "(": "（", ")": "）", "[": "【", "]": "】", "\\": "、",
      "<": "《", ">": "》", "'": "‘", "\"": "“", "_": "——",
    ])
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()

    func faces(_ row: Int) -> [String] {
      // 按这一行排布按键的顺序，而不是切换键加入该行子视图的顺序。
      let rowView = descendants(controller.view).first { $0.accessibilityIdentifier == "symbolLayerRow\(row)" } as? UIStackView
      return rowView?.arrangedSubviews.compactMap { ($0 as? UIButton)?.configuration?.title } ?? []
    }

    // 中文 123 层，与设计稿和 Android 的画法一致。
    XCTAssertEqual(faces(0), ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"])
    XCTAssertEqual(faces(1), ["-", "/", "：", "；", "（", "）", "¥", "@", "“", "”"])
    XCTAssertEqual(faces(2), ["#+=", "。", "，", "、", "？", "！"])
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "拼音")
    XCTAssertFalse(try button("layerEmojiKey", in: controller).isHidden)
    XCTAssertTrue(try button("bottomLanguageKey", in: controller).isHidden)
    XCTAssertNotNil(shownSymbolKey("、", in: controller))
    XCTAssertNil(shownSymbolKey("\\", in: controller))

    // #+= 换掉前两行和切换键；底行的 emoji 键变成 符号。
    try button("symbolLayerToggle", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(faces(0), ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="])
    XCTAssertEqual(faces(1), ["_", "\\", "|", "~", "《", "》", "€", "&", "·", "…"])
    XCTAssertEqual(faces(2), ["123", "。", "，", "、", "？", "！"])
    XCTAssertTrue(try button("layerEmojiKey", in: controller).isHidden)
    XCTAssertFalse(try button("symbolPanelKey", in: controller).isHidden)
    XCTAssertEqual(try button("symbolPanelKey", in: controller).configuration?.title, "符号")
    XCTAssertNotNil(shownSymbolKey("\\", in: controller), "the Chinese #+= layer draws the backslash itself")

    // 关闭该层会丢掉 #+= 状态，所以再次打开时回到 123，字母行的 中 键也回来了。
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(try button("bottomLanguageKey", in: controller).isHidden)
    XCTAssertTrue(try button("symbolPanelKey", in: controller).isHidden)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(faces(0), ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"])
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)

    // 英文画 ASCII 变体，返回键标为 ABC。
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(faces(1), ["-", "/", ":", ";", "(", ")", "$", "@", "\"", "'"])
    XCTAssertEqual(faces(2), ["#+=", ".", ",", "?", "!", "…"])
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "ABC")
    XCTAssertNil(shownSymbolKey("：", in: controller))
  }

  /// 123 / #+= 层：第三行为 #+= 1.4 | 五个符号 | ⌫ 1.4，底行按相对空格键的比例为 拼音 1.25 | emoji 1.05 | space 6 | return 1.9，没有 中 键。
  func testSymbolLayerFollowsTheDesignsWeights() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let letterKey = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" })
    let letterHeight = letterKey.bounds.height
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let row = try XCTUnwrap(try button("spaceKey", in: controller).superview as? UIStackView)
    let visible = row.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier)
    let globe = controller.needsInputModeSwitchKey ? ["inputModeSwitchButton"] : []
    XCTAssertEqual(visible, ["layoutToggleButton", "layerEmojiKey"] + globe + ["spaceKey", "returnKey"])
    let unit = try button("spaceKey", in: controller).bounds.width / SymbolLayerLayout.spaceWeight
    XCTAssertEqual(try button("layoutToggleButton", in: controller).bounds.width, unit * 1.25, accuracy: 0.5)
    XCTAssertEqual(try button("layerEmojiKey", in: controller).bounds.width, unit * 1.05, accuracy: 0.5)
    XCTAssertEqual(try button("returnKey", in: controller).bounds.width, unit * 1.9, accuracy: 0.5)
    XCTAssertEqual(try button("spaceKey", in: controller).configuration?.title, "空格")

    let mark = try button("symbolLayerKey2_0", in: controller)
    for id in ["symbolLayerToggle", "symbolLayerDeleteKey"] {
      XCTAssertEqual(try button(id, in: controller).bounds.width, mark.bounds.width * SymbolLayerLayout.edgeWeight, accuracy: 0.5, id)
    }
    XCTAssertEqual(try button("symbolLayerKey0_0", in: controller).bounds.height, letterHeight, accuracy: 0.5)
    XCTAssertEqual(try button("returnKey", in: controller).bounds.height, letterHeight, accuracy: 0.5)
    XCTAssertTrue(try button("symbolDeleteKey", in: controller).isHidden, "⌫ is in the layer's third row")
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant,
                   KeyboardViewController.defaultKeyboardHeight, "the layer is as tall as the letters")
    for (name, more) in [("123", false), ("#+=", true)] {
      if more { try button("symbolLayerToggle", in: controller).sendActions(for: .primaryActionTriggered) }
      controller.view.layoutIfNeeded()
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Symbol layer \(name)"
      attachment.lifetime = .keepAlways
      add(attachment)
    }
    XCTAssertFalse(try button("symbolPanelKey", in: controller).isHidden)
    XCTAssertEqual(try button("symbolPanelKey", in: controller).bounds.width, unit * 1.05, accuracy: 0.5)
  }

  func testSymbolLayerKeysRouteThroughThePunctuationPathOrInsertAsDrawn() {
    typealias Input = SymbolLayerLayout.Input
    // 两种变体上的数字都走符号键路径，所以组字时 1-9 仍能选候选。
    XCTAssertEqual(SymbolLayerLayout.input(for: "1", chinese: true, chinesePunctuation: true), Input.symbol("1"))
    XCTAssertEqual(SymbolLayerLayout.input(for: "0", chinese: false, chinesePunctuation: true), Input.symbol("0"))
    // Engine 一对一输出的中文符号发送对应的 ASCII 键，这样智能标点和配对才会生效。
    let oneToOne = [("，", ","), ("。", "."), ("？", "?"), ("！", "!"), ("：", ":"), ("；", ";"), ("（", "("), ("）", ")"), ("、", "\\")]
    for (face, key) in oneToOne {
      XCTAssertEqual(SymbolLayerLayout.input(for: face, chinese: true, chinesePunctuation: true), Input.symbol(key), face)
      // 中文标点关闭或锁定为英文时 Engine 会把这个 ASCII 键原样写出（、 会变成反斜线），所以按键面原样插入，与 Android 设计层一样写出键上的标点。
      XCTAssertEqual(SymbolLayerLayout.input(for: face, chinese: true, chinesePunctuation: false), Input.literal(face), face)
    }
    // 数字和 Engine 原样放行的 ASCII 不受中文标点开关影响：组字时数字仍能选候选，网址符号仍能进入组字。
    for face in ["1", "-", "/", "@"] {
      XCTAssertEqual(SymbolLayerLayout.input(for: face, chinese: true, chinesePunctuation: false), Input.symbol(face), face)
    }
    // 引号要交替、《》要嵌套、￥ 在 Engine 里不是 ¥，而且 Engine 会把 [ \ ^ _ 转成中文符号：这些按键面所画直接插入。
    for face in ["“", "”", "《", "》", "¥", "…", "·", "€", "[", "]", "\\", "^", "_"] {
      XCTAssertEqual(SymbolLayerLayout.input(for: face, chinese: true, chinesePunctuation: true), Input.literal(face), face)
    }
    // Engine 原样透传的 ASCII 走符号键路径，所以网址和拼写用的符号仍能进入组字。
    for face in ["-", "/", "@", "{", "}", "#", "%", "*", "+", "=", "|", "~", "&"] {
      XCTAssertEqual(SymbolLayerLayout.input(for: face, chinese: true, chinesePunctuation: true), Input.symbol(face), face)
    }
    // 英文层发送所有 ASCII 符号，其余的直接插入。
    for face in [",", ".", "(", "\"", "'", "$", "[", "\\", "<", "_"] {
      XCTAssertEqual(SymbolLayerLayout.input(for: face, chinese: false, chinesePunctuation: true), Input.symbol(face), face)
    }
    for face in ["…", "€", "·", "£"] {
      XCTAssertEqual(SymbolLayerLayout.input(for: face, chinese: false, chinesePunctuation: true), Input.literal(face), face)
    }
  }

  func testOnlyTheNineKeyKanaAndDachenKeepTheirOwnSymbolLayers() {
    XCTAssertTrue(KeyboardViewController.drawsSymbolLayer(symbols: true, nineKey: false, kana: false, dachen: false))
    XCTAssertFalse(KeyboardViewController.drawsSymbolLayer(symbols: false, nineKey: false, kana: false, dachen: false))
    XCTAssertFalse(KeyboardViewController.drawsSymbolLayer(symbols: true, nineKey: true, kana: false, dachen: false))
    XCTAssertFalse(KeyboardViewController.drawsSymbolLayer(symbols: true, nineKey: false, kana: true, dachen: false))
    XCTAssertFalse(KeyboardViewController.drawsSymbolLayer(symbols: true, nineKey: false, kana: false, dachen: true))
    for more in [false, true] {
      for chinese in [false, true] {
        XCTAssertEqual(SymbolLayerLayout.characterRows(more: more, chinese: chinese).map(\.count), [10, 10, 5])
      }
    }
    XCTAssertEqual(SymbolLayerLayout.characterRows(more: true, chinese: false)[1], ["_", "\\", "|", "~", "<", ">", "€", "&", "·", "£"])
    XCTAssertEqual(SymbolLayerLayout.lettersTitle(chinese: true), "拼音")
    XCTAssertEqual(SymbolLayerLayout.lettersTitle(chinese: false), "ABC")
  }

  func testSpacingChangesKeepDefaultKeyPlacementAndComposition() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    for scheme in [ChineseInputScheme.quanpin, .nineKey] {
      InputSchemePreference.scheme = scheme
      for width in [320.0, 440.0] {
        let controller = KeyboardViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: width, height: 292)
        for gap in [3.0, 6.0] {
          // The spacing used to come from sliders inside the keyboard and now comes from a drag on
          // the keys. A pan cannot be synthesised here, and what this checks is that the keys follow
          // the spacing, so it writes the preference and lets the keyboard lay out again.
          KeyboardLayoutPreference.keySpacing = gap
          KeyboardLayoutPreference.rowSpacing = gap == 3 ? 4 : 10
          controller.viewWillAppear(false)
          controller.view.layoutIfNeeded()
          XCTAssertEqual(KeyboardLayoutPreference.geometry.keySpacing, gap)
          let enter = try button("returnKey", in: controller)
          let space = try button("spaceKey", in: controller)
          XCTAssertGreaterThanOrEqual(space.bounds.width, 43.5)
          XCTAssertLessThanOrEqual(enter.convert(enter.bounds, to: controller.view).maxX, width)
          XCTAssertEqual(controller.view.bounds.height, 292)
          XCTAssertFalse(try button("bottomLanguageKey", in: controller).isHidden)
        }
      }
    }
  }

  /// 语音入口只由它自己的偏好决定露不露面，键距和行距不牵连它。
  func testVoiceShortcutFollowsOnlyItsOwnPreference() throws {
    KeyboardLayoutPreference.keySpacing = 5
    KeyboardLayoutPreference.rowSpacing = 8
    KeyboardLayoutPreference.voiceShortcutEnabled = true
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 440, height: 292)
    controller.viewWillAppear(false)
    XCTAssertFalse(try button("layoutVoiceShortcut", in: controller).isHidden)
    KeyboardLayoutPreference.voiceShortcutEnabled = false
    controller.viewWillAppear(false)
    XCTAssertTrue(try button("layoutVoiceShortcut", in: controller).isHidden)
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 5)
    XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, 8)
  }

  func testTouchGeometryWritesAndResetsCanonicalPreferences() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-touch-geometry-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.setTouchKeyboardGeometry(
      keySpacing: 99, rowSpacing: -1, heightAdjustment: 99, voiceEnabled: true))
    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual((preferences["touch_key_spacing_tenths"] as? NSNumber)?.intValue, 60)
    XCTAssertEqual((preferences["touch_row_spacing_tenths"] as? NSNumber)?.intValue, 40)
    XCTAssertEqual((preferences["touch_keyboard_height_adjustment"] as? NSNumber)?.intValue, 48)
    XCTAssertEqual(preferences["touch_voice_shortcut"] as? Bool, true)

    XCTAssertTrue(bridge.resetTouchKeyboardGeometry())
    let reset = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertNil(reset["touch_key_spacing_tenths"])
    XCTAssertNil(reset["touch_row_spacing_tenths"])
    XCTAssertNil(reset["touch_keyboard_height_adjustment"])
    XCTAssertNil(reset["touch_voice_shortcut"])
  }

  func testToolbarIsTheDesignsEqualGridOfTools() throws {
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      let toolbar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" } as? UIStackView)
      let more = try XCTUnwrap(try button("moreShortcut", in: controller) as? KeyboardBrandMarkButton)
      XCTAssertTrue(toolbar.arrangedSubviews.first === more)
      XCTAssertEqual(toolbar.distribution, .fillEqually)
      // 表情、剪贴板、皮肤 和 iOS 额外按钮跟随共享的 工具栏按钮 设置，常用语 和 输入方式 跟随本机自己的开关，收起 用于关闭这一行。
      let pinned = TouchToolbarPreference(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
      var expected = ["moreShortcut"]
      if pinned.emoji { expected.append("emojiShortcut") }
      if TouchToolbarLocalPreference.phrases { expected.append("phrasesShortcut") }
      if pinned.clipboard { expected.append("clipboardShortcut") }
      if pinned.skin { expected.append("skinShortcut") }
      if TouchToolbarLocalPreference.scheme { expected.append("schemeButton") }
      if pinned.ai { expected.append("aiShortcut") }
      if pinned.characterSet { expected.append("characterSetShortcut") }
      if pinned.fullwidth { expected.append("fullwidthShortcut") }
      if pinned.punctuation { expected.append("punctuationShortcut") }
      if KeyboardLayoutPreference.voiceShortcutEnabled { expected.append("layoutVoiceShortcut") }
      expected.append("dismissShortcut")
      let visible = toolbar.arrangedSubviews.filter { !$0.isHidden }
      XCTAssertEqual(visible.compactMap(\.accessibilityIdentifier), expected)
      for item in visible {
        XCTAssertEqual(item.bounds.width, visible[0].bounds.width, accuracy: 0.5, item.accessibilityIdentifier ?? "")
        XCTAssertLessThanOrEqual(item.frame.maxX, toolbar.bounds.width + 0.5)
      }
      XCTAssertNil(more.menu)
      XCTAssertNotNil(descendants(more).first { $0.accessibilityIdentifier == "keyboardBrandIcon" })
      // 设计稿的线框图标自己绘制：没有一个带着会被皮肤处理画成键帽的 configuration。
      for id in ["emojiShortcut", "phrasesShortcut", "schemeButton", "dismissShortcut"] {
        let item = try XCTUnwrap(try button(id, in: controller) as? KeyboardToolbarButton, id)
        XCTAssertNil(item.configuration, id)
      }
    }
  }

  func testBrandTogglesTheFunctionMenuInTheKeyArea() throws {
    let previous = KeyboardPrivacyPreference.incognito
    defer { KeyboardPrivacyPreference.incognito = previous }
    for width in [320.0, 414.0] {
      KeyboardPrivacyPreference.incognito = false
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      let height = controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant
      let more = try XCTUnwrap(try button("moreShortcut", in: controller) as? KeyboardBrandMarkButton)
      more.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" } as? KeyboardMorePickerView)
      try assertInKeyArea(panel, of: controller)
      XCTAssertTrue(more.isActive)
      XCTAssertFalse(panel.accessibilityViewIsModal, "the toolbar closes the menu, so VoiceOver must reach it")
      XCTAssertEqual(panel.currentPage, 0)
      XCTAssertNil(descendants(panel).first { $0.accessibilityIdentifier == "closeMorePicker" }, "the toolbar row is the menu's only header")

      // 手机上每页 4 × 2 个 52pt 图块，每页一个圆点，当前页圆点宽 16pt。
      let tiles = descendants(panel).compactMap { $0 as? KeyboardFunctionTileView }
      for tile in tiles { XCTAssertEqual(tile.bounds.height, 52, accuracy: 0.5, tile.tool.title) }
      let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
      XCTAssertEqual(tiles.filter { page(of: $0, in: scroll) == 0 }.count, 8)
      XCTAssertEqual(panel.pageCount, (tiles.count + 7) / 8)
      let dots = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagerDotsView }.first)
      XCTAssertEqual(dots.count, panel.pageCount)
      XCTAssertEqual(dots.subviews[dots.active].bounds.width, KeyboardPagerDotsView.activeWidth)
      XCTAssertEqual(KeyboardPagerDotsView.activeWidth, 16)

      // 改名后的图块在 VoiceOver 里保留完整的长名称。
      XCTAssertEqual(try tile("moreCard-全角", in: controller).accessibilityLabel, "全角输入")
      XCTAssertEqual(try tile("moreCard-繁体", in: controller).accessibilityLabel, "繁体输出")
      if KeyboardFeedbackPreference.hapticsAvailable {
        XCTAssertEqual(try tile("moreCard-振动", in: controller).accessibilityLabel, "按键振动")
      }

      // 拨动开关后菜单保持打开，停在原来那一页。
      let grid = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagedGridView }.first)
      grid.scrollToPage(1, animated: false)
      XCTAssertEqual(panel.currentPage, 1)
      let privacy = try tile("moreCard-隐私模式", in: controller)
      XCTAssertEqual(page(of: privacy, in: scroll), 1)
      XCTAssertEqual(privacy.accessibilityValue, "已关闭")
      privacy.sendActions(for: .primaryActionTriggered)
      XCTAssertTrue(KeyboardPrivacyPreference.incognito)
      XCTAssertEqual(try tile("moreCard-隐私模式", in: controller).accessibilityValue, "已开启")
      XCTAssertNotNil(panel.superview, "a switch leaves the menu open")
      XCTAssertEqual(panel.currentPage, 1, "a switch keeps the page")

      // 本地输入 把菜单换成各本地模式，最前面是一个返回图块。
      let local = try tile("moreCard-本地输入", in: controller)
      if local.isEnabled {
        local.sendActions(for: .primaryActionTriggered)
        for title in ["返回工具", "日期时间", "Unicode 码点"] { XCTAssertNoThrow(try tile("moreCard-" + title, in: controller)) }
        try tile("moreCard-返回工具", in: controller).sendActions(for: .primaryActionTriggered)
        XCTAssertNoThrow(try tile("moreCard-全角", in: controller))
      }

      // 点品牌标志再次关闭菜单，按键回来。
      more.sendActions(for: .primaryActionTriggered)
      XCTAssertNil(panel.superview)
      XCTAssertFalse(more.isActive)
      XCTAssertEqual(try button("spaceKey", in: controller).superview?.alpha, 1)
      more.sendActions(for: .primaryActionTriggered)
      let reopened = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" } as? KeyboardMorePickerView)
      XCTAssertEqual(reopened.currentPage, 0, "the menu opens on its first page")
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Function menu \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      // 跳转到别处的图块会先关闭菜单。
      try tile("moreCard-AI 润色", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" })
      XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, height)
    }
  }

  func testFullWidthInputConvertsOnlyDirectPrintableASCIIAndPreservesComposition() throws {
    XCTAssertEqual(FullWidthInputPolicy.output(" A!~9", enabled: true), "　Ａ！～９")
    XCTAssertEqual(FullWidthInputPolicy.output("中文，🙂\n", enabled: true), "中文，🙂\n")
    XCTAssertEqual(FullWidthInputPolicy.output(" A!~9", enabled: false), " A!~9")

    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let letter = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    letter.sendActions(for: .primaryActionTriggered)
    let preedit = try button("preeditButton", in: controller).configuration?.title
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try tile("moreCard-全角", in: controller).accessibilityValue, "已关闭")
    try tile("moreCard-全角", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try tile("moreCard-全角", in: controller).accessibilityValue, "已开启")
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, preedit)
  }

  func testSchemePickerUsesCurrentSkinPalette() throws {
    preserveSharedTheme()
    for id in ["system", "paper", "night"] {
      XCTAssertTrue(GlobalThemePreference.save(id), id)
      let skin = KeyboardTheme.current
      XCTAssertEqual(skin.id, id)
      for style in [UIUserInterfaceStyle.light, .dark] {
        let traits = UITraitCollection(userInterfaceStyle: style)
        let picker = KeyboardSchemePickerView(selected: .nineKey, onSelect: { _ in }, onClose: {})
        picker.overrideUserInterfaceStyle = style
        picker.frame = CGRect(x: 0, y: 0, width: 414, height: 292)
        picker.layoutIfNeeded()
        func assertColor(_ actual: UIColor?, _ expected: UIColor, file: StaticString = #filePath, line: UInt = #line) {
          XCTAssertEqual(actual?.resolvedColor(with: traits), expected.resolvedColor(with: traits), file: file, line: line)
        }
        let buttons = descendants(picker).compactMap { $0 as? UIButton }
        XCTAssertFalse(buttons.contains { $0.accessibilityIdentifier == "schemePickerKeyboardTab" })
        XCTAssertFalse(buttons.contains { $0.accessibilityIdentifier == "schemePickerThemeTab" })
        let back = try XCTUnwrap(buttons.first { $0.accessibilityIdentifier == "closeSchemePicker" })
        let selected = try XCTUnwrap(buttons.first { $0.accessibilityIdentifier == "schemeCard-nineKey" })
        assertColor(picker.backgroundColor, skin.background)
        assertColor(back.tintColor, skin.accent)
        // 设计稿的图块没有填充：当前输入方式由强调色图标、标签和勾选角标标出。
        XCTAssertEqual(selected.backgroundColor?.cgColor.alpha ?? 0, 0)
        XCTAssertTrue(selected.isSelected)
        let labels = selected.subviews.compactMap { $0 as? UILabel }.filter { $0.text?.isEmpty == false }
        XCTAssertEqual(labels.count, 3)
        for label in labels {
          assertColor(label.textColor, skin.accent)
        }
      }
    }
  }

  func testSchemeGridHasFourColumnsAtNarrowAndWideSizes() throws {
    for width in [320.0, 414.0, 812.0] {
      let picker = KeyboardSchemePickerView(selected: .nineKey, onSelect: { _ in }, onClose: {})
      picker.frame = CGRect(x: 0, y: 0, width: width, height: width > 500 ? 216 : 260)
      picker.layoutIfNeeded()
      let cards = descendants(picker).filter { $0.accessibilityIdentifier?.hasPrefix("schemeCard-") == true }
      XCTAssertGreaterThanOrEqual(cards.count, 4)
      let firstRow = cards.prefix(4).map { $0.convert($0.bounds, to: picker) }
      for frame in firstRow {
        XCTAssertEqual(frame.minY, firstRow[0].minY, accuracy: 0.1)
        XCTAssertEqual(frame.width, firstRow[0].width, accuracy: 0.5)
        XCTAssertGreaterThanOrEqual(frame.width, 60)
        XCTAssertGreaterThanOrEqual(frame.height, 44)
        // 手机网格沿用功能菜单 4pt 的页内边距。
        XCTAssertGreaterThanOrEqual(frame.minX, 4)
        XCTAssertLessThanOrEqual(frame.maxX, width - 4)
      }
      XCTAssertEqual(Set(firstRow.map { $0.minX }).count, 4)
      XCTAssertTrue(descendants(picker).compactMap { $0 as? KeyboardSkinMiniature }.isEmpty)
    }
  }

  /// 面板里的双拼只列一种（#6450）：选中的双拼优先，其次是文档里的 `shuangpin_profile`，都不在列表里时取列表里第一种双拼；其余方案原样保留，与 Android、鸿蒙相同。
  func testSchemePickerListsOnlyTheConfiguredShuangpin() {
    let all = ChineseInputScheme.allCases
    func only(_ kept: ChineseInputScheme) -> [ChineseInputScheme] { all.filter { $0.shuangpinProfile == nil || $0 == kept } }
    XCTAssertEqual(InputSchemePreference.pickerSchemes(all, selected: .quanpin, shuangpinProfile: nil), only(.shuangpin))
    XCTAssertEqual(InputSchemePreference.pickerSchemes(all, selected: .quanpin, shuangpinProfile: "future"), only(.shuangpin))
    XCTAssertEqual(InputSchemePreference.pickerSchemes(all, selected: .wubi, shuangpinProfile: "microsoft"), only(.microsoft))
    XCTAssertEqual(InputSchemePreference.pickerSchemes(all, selected: .shoudao, shuangpinProfile: "ziranma"), only(.shoudao))
    let partial: [ChineseInputScheme] = [.quanpin, .ziranma, .shoudao, .handwriting]
    XCTAssertEqual(InputSchemePreference.pickerSchemes(partial, selected: .quanpin, shuangpinProfile: "xiaohe"), [.quanpin, .ziranma, .handwriting])
    XCTAssertEqual(InputSchemePreference.pickerSchemes(partial, selected: .shuangpin, shuangpinProfile: "xiaohe"), [.quanpin, .ziranma, .handwriting])
    XCTAssertEqual(InputSchemePreference.pickerSchemes([.quanpin, .wubi], selected: nil, shuangpinProfile: "microsoft"), [.quanpin, .wubi])

    // 默认启用的方案下只剩五张方案卡片，键盘里再加英文和「添加语言」共七格，手机一页 4 × 2 放得下，手写不再被挤到第二页。
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled }
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey, .shuangpin, .ziranma, .microsoft, .shoudao, .wubi, .handwriting]
    let picker = KeyboardSchemePickerView(selected: .quanpin, shuangpinProfile: "ziranma", onSelect: { _ in }, onClose: {})
    let cards = descendants(picker).compactMap(\.accessibilityIdentifier).filter { $0.hasPrefix("schemeCard-") }
    XCTAssertEqual(Set(cards), ["schemeCard-quanpin", "schemeCard-nineKey", "schemeCard-ziranma", "schemeCard-wubi", "schemeCard-handwriting"])
    XCTAssertLessThanOrEqual(cards.count + 2, 8)
  }

  func testSchemeCardsSelectAndKeepKeyboardHeight() throws {
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    // 粤拼和注音只在测试宿主带了对应词库时才有卡片（#2704）；词库是可选资源，CI 不暂存，所以按实际提供的方案逐张检查，缺词库的方案必须不出卡片。
    let offered = InputSchemePreference.offeredSchemes
    let withheld = ChineseInputScheme.allCases.filter { !offered.contains($0) }
    XCTAssertTrue(withheld.allSatisfy(\.needsLanguageDictionary), "only a scheme whose dictionary is missing may be left off: \(withheld)")
    // 双拼只列一种（#6450），由上面的 `testSchemePickerListsOnlyTheConfiguredShuangpin` 检查是哪一种；这里只数张数。
    let shuangpin = offered.filter { $0.shuangpinProfile != nil }
    let listed = offered.filter { $0.shuangpinProfile == nil }
    for width in [320.0, 414.0] {
      // 上一轮点全拼卡片时键盘把选择记进了共享文档，新建的键盘按文档行事，所以从 9 键开始要像设置页那样写进文档，只改镜像不够。
      XCTAssertTrue(InputSchemePreference.select(.nineKey))
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let picker = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSchemePicker" } as? KeyboardSchemePickerView)
      try assertInKeyArea(picker, of: controller)
      XCTAssertTrue(try XCTUnwrap(try button("schemeButton", in: controller) as? KeyboardToolbarButton).isActive)
      let shownShuangpin = shuangpin.filter { scheme in
        descendants(controller.view).contains { $0.accessibilityIdentifier == "schemeCard-\(scheme.rawValue)" }
      }
      XCTAssertEqual(shownShuangpin.count, shuangpin.isEmpty ? 0 : 1)
      for scheme in listed + shownShuangpin {
        let card = try button("schemeCard-\(scheme.rawValue)", in: controller)
        XCTAssertGreaterThanOrEqual(card.bounds.width, 60)
        XCTAssertEqual(card.bounds.height, KeyboardSchemeTileView.height, accuracy: 0.5)
      }
      for scheme in withheld {
        XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "schemeCard-\(scheme.rawValue)" }, scheme.rawValue)
      }
      // 每张卡片都在选择器的高度之内，落在其中某一页上。
      for scheme in listed + shownShuangpin {
        let card = try button("schemeCard-\(scheme.rawValue)", in: controller)
        XCTAssertLessThanOrEqual(card.convert(card.bounds, to: picker).maxY, picker.bounds.height + 0.5, scheme.rawValue)
      }
      XCTAssertGreaterThanOrEqual(picker.pageCount, 1)
      XCTAssertEqual(try button("schemeCard-nineKey", in: controller).accessibilityValue, "已选中")
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Input scheme cards \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      try button("schemeCard-quanpin", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertNil(picker.superview)
      XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
      XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, ChineseInputScheme.quanpin.title)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, KeyboardViewController.defaultKeyboardHeight)
      try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(try button("schemeCard-quanpin", in: controller).accessibilityValue, "已选中")
      // 高亮的 输入方式 图标关闭它自己的选择器。
      try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardSchemePicker" })
      XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
    }
  }

  func testSchemePickerSwitchesEnglishAndTheToolbarMovesBetweenPanels() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 393, height: 260)
    controller.view.layoutIfNeeded()
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    try button("schemeEnglishCard", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "英文输入")
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("schemeEnglishCard", in: controller).accessibilityValue, "已选中")
    XCTAssertEqual(try button("schemeCard-quanpin", in: controller).accessibilityValue, "")
    try button("schemeCard-quanpin", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "中文输入")
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "schemePickerThemeTab" })
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "schemePickerKeyboardTab" })
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardSkinPickerView })
    XCTAssertEqual(try button("schemeCard-quanpin", in: controller).accessibilityValue, "已选中")
    // 键区里的选择器没有标题栏：靠工具栏离开。点品牌标志会关闭它，而不是在它上面打开菜单。
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "schemePickerSettings" })
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardSchemePickerView })
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" })
    // 点另一个工具栏图标会在同一位置把一个面板换成另一个。
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    try button("skinShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardSchemePickerView })
    XCTAssertTrue(descendants(controller.view).contains { $0 is KeyboardSkinPickerView })
    try button("skinShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardSkinPickerView })
  }

  func testVisibleInputViewReflectsSoundPreference() throws {
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.object(forKey: KeyboardFeedbackPreference.soundKey)
    defer {
      if let previous { defaults.set(previous, forKey: KeyboardFeedbackPreference.soundKey) }
      else { defaults.removeObject(forKey: KeyboardFeedbackPreference.soundKey) }
    }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let inputView = try XCTUnwrap(controller.inputView as? KeyboardInputView)
    XCTAssertTrue(controller.view === inputView)
    defaults.set(true, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertTrue(inputView.enableInputClicksWhenVisible)
    defaults.set(false, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertFalse(inputView.enableInputClicksWhenVisible)
  }

  func testShiftIsDiscoverableAndSwitchesToEnglishCapitalization() throws {
    let previous = InputSchemePreference.scheme
    InputSchemePreference.scheme = .quanpin
    defer { InputSchemePreference.scheme = previous }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let shift = try button("shiftButton", in: controller)
    XCTAssertFalse(shift.isHidden)
    XCTAssertEqual(shift.accessibilityLabel, "切换到英文大写")
    XCTAssertEqual(try button("inputModeSwitchButton", in: controller).isHidden,
                   !controller.needsInputModeSwitchKey)
    // 同 Android 的 styleShiftKey：关闭时为功能键底色配按键文字色，开启（单次或锁定）时为字母键底色配强调色图标。
    let light = UITraitCollection(userInterfaceStyle: .light)
    func colours() -> (fill: UIColor?, icon: UIColor?) {
      (shift.configuration?.background.backgroundColor?.resolvedColor(with: light),
       shift.configuration?.baseForegroundColor?.resolvedColor(with: light))
    }
    let skin = KeyboardTheme.current
    let off = (skin.functionKeyBackground.resolvedColor(with: light), skin.keyForeground.resolvedColor(with: light))
    let on = (skin.keyBackground.resolvedColor(with: light), skin.accent.resolvedColor(with: light))
    if skin.design == nil {
      XCTAssertEqual(colours().fill, off.0)
      XCTAssertEqual(colours().icon, off.1)
    }
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(shift.accessibilityValue, "下一字母")
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "英文输入")
    XCTAssertEqual(colours().icon, on.1)
    if skin.design == nil { XCTAssertEqual(colours().fill, on.0) }
    let shiftedIcon = shift.configuration?.image
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(shift.accessibilityValue, "开启")
    XCTAssertEqual(colours().icon, on.1)
    XCTAssertFalse(shift.configuration?.image === shiftedIcon, "Caps Lock draws the underlined icon")
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(shift.accessibilityValue, "关闭")
    XCTAssertEqual(colours().icon, off.1)
  }

  func testShiftEntersHelpcodeWhileComposingShuangpin() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .shuangpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: 390,
      height: KeyboardViewController.defaultKeyboardHeight)

    func letterKey(_ letter: String) throws -> UIButton {
      try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityLabel == "字母 \(letter)" || $0.accessibilityLabel == "大写 \(letter)"
      } as? UIButton)
    }

    try letterKey("N").sendActions(for: .primaryActionTriggered)
    try letterKey("I").sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try button("preeditButton", in: controller).configuration?.title?.isEmpty ?? true)

    let shift = try button("shiftButton", in: controller)
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "中文输入")
    XCTAssertEqual(try letterKey("H").accessibilityLabel, "大写 H")

    try letterKey("H").sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try letterKey("H").accessibilityLabel, "字母 H")
    XCTAssertEqual(shift.accessibilityValue, "关闭")
  }

  func testPressFeedbackPreservesLayoutAndResetsAfterInterruption() {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 414, height: 260))
    let controller = UIViewController()
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    let key = KeyboardKeyButton(frame: CGRect(x: 20, y: 20, width: 44, height: 48))
    controller.view.addSubview(key)
    let bounds = key.bounds
    let center = key.center
    for _ in 0..<10 {
      key.isHighlighted = true
      XCTAssertEqual(key.bounds, bounds)
      XCTAssertEqual(key.center, center)
      XCTAssertEqual(key.transform.isIdentity, UIAccessibility.isReduceMotionEnabled)
      key.isHighlighted = false
      XCTAssertTrue(key.transform.isIdentity)
    }
    key.isHighlighted = true
    key.isEnabled = false
    XCTAssertTrue(key.transform.isIdentity)
    key.isEnabled = true
    key.isHighlighted = false
    key.isHighlighted = true
    key.removeFromSuperview()
    XCTAssertTrue(key.transform.isIdentity)
    XCTAssertTrue(key.layer.animationKeys()?.isEmpty ?? true)
  }

  func testPressPreviewFollowsTheHighlightOnlyForKeysThatAskForIt() {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 300))
    let controller = UIViewController()
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    func key(_ title: String, x: CGFloat, preview: Bool) -> KeyboardKeyButton {
      var configuration = UIButton.Configuration.plain()
      configuration.title = title
      let key = KeyboardKeyButton(configuration: configuration)
      key.frame = CGRect(x: x, y: 120, width: 36, height: 44)
      key.showsPressPreview = preview
      controller.view.addSubview(key)
      return key
    }
    func previews() -> [KeyPressPreviewView] { descendants(controller.view).compactMap { $0 as? KeyPressPreviewView } }
    let letter = key("q", x: 20, preview: true)
    let function = key("中", x: 80, preview: false)

    letter.isHighlighted = true
    XCTAssertEqual(previews().map(\.text), ["q"])
    letter.isHighlighted = false
    XCTAssertTrue(previews().isEmpty)

    function.isHighlighted = true
    XCTAssertTrue(previews().isEmpty)
    function.isHighlighted = false

    // Shift changes the title between presses; the callout shows whatever the key says now.
    letter.configuration?.title = "Q"
    letter.isHighlighted = true
    XCTAssertEqual(previews().map(\.text), ["Q"])
    letter.isEnabled = false
    XCTAssertTrue(previews().isEmpty)
    letter.isEnabled = true
    letter.isHighlighted = false
    letter.isHighlighted = true
    XCTAssertEqual(previews().count, 1)
    letter.removeFromSuperview()
    XCTAssertTrue(previews().isEmpty)
  }

  func testPressPreviewHeadStaysInsideTheKeyboard() {
    let bounds = CGRect(x: 0, y: 0, width: 390, height: 300)
    let middle = KeyPressPreviewView.geometry(key: CGRect(x: 180, y: 120, width: 36, height: 44), in: bounds)
    XCTAssertGreaterThan(middle.head.width, 36)
    XCTAssertEqual(middle.head.midX, 198, accuracy: 0.01)
    // 设计稿的气泡向下盖过按键上沿，而不是停在上沿之上。
    XCTAssertEqual(middle.head.maxY, 120 + KeyPressPreviewView.overlap, accuracy: 0.01)
    XCTAssertGreaterThanOrEqual(middle.head.height, 44)

    let left = KeyPressPreviewView.geometry(key: CGRect(x: 3, y: 120, width: 36, height: 44), in: bounds)
    XCTAssertEqual(left.head.minX, 0, accuracy: 0.01)
    XCTAssertLessThanOrEqual(left.head.minX, left.key.minX)
    let right = KeyPressPreviewView.geometry(key: CGRect(x: 351, y: 120, width: 36, height: 44), in: bounds)
    XCTAssertEqual(right.head.maxX, 390, accuracy: 0.01)
    XCTAssertGreaterThanOrEqual(right.head.maxX, right.key.maxX)

    // A row with little room above it gets a shorter head rather than one the system would clip.
    let top = KeyPressPreviewView.geometry(key: CGRect(x: 180, y: 30, width: 36, height: 44), in: bounds)
    XCTAssertEqual(top.head.minY, 0, accuracy: 0.01)
    XCTAssertEqual(top.head.height, 30 + KeyPressPreviewView.overlap, accuracy: 0.01)
  }

  func testKeyShadowsFollowTheKeysWhenTheKeyboardShrinksIntoPlace() throws {
    // 按钮层的阴影只属于内置主题；自定义设计（例如薄荷晨光）的阴影由 `SkinKeySurfaceView` 自己画，按钮的 `shadowOpacity` 为 0，所以这里先选内置的 paper。
    preserveSharedTheme()
    XCTAssertTrue(GlobalThemePreference.save("paper"))
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    // The system shows a keyboard at a taller window first and walks it down to the real height (874 -> 444 -> 292 measured on iOS 26.3).
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 874)
    controller.view.layoutIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let shadowed = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { $0.layer.shadowOpacity > 0 && $0.window == nil && !$0.isHidden && $0.bounds.height > 0 }
    XCTAssertFalse(shadowed.isEmpty)
    for button in shadowed {
      let path = try XCTUnwrap(button.layer.shadowPath, button.accessibilityLabel ?? "")
      XCTAssertEqual(path.boundingBoxOfPath.height, button.bounds.height, accuracy: 0.5, button.accessibilityLabel ?? "")
      XCTAssertEqual(path.boundingBoxOfPath.width, button.bounds.width, accuracy: 0.5, button.accessibilityLabel ?? "")
    }
  }

  func testOnlyCharacterKeysOfTheRealKeyboardShowAPressPreview() throws {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let keys = descendants(controller.view).compactMap { $0 as? KeyboardKeyButton }
    let letter = try XCTUnwrap(keys.first { ["q", "Q"].contains($0.configuration?.title ?? "") })
    XCTAssertTrue(letter.showsPressPreview)
    for id in ["shiftButton", "returnKey"] {
      let key = try XCTUnwrap(keys.first { $0.accessibilityIdentifier == id }, id)
      XCTAssertFalse(key.showsPressPreview, id)
    }
    let space = try XCTUnwrap(keys.first { $0.accessibilityLabel == "空格" })
    XCTAssertFalse(space.showsPressPreview)
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  /// `view` 是否被面板盖住：它所在的按键行被藏起来，既不绘制、不接收触摸，也不出现在 VoiceOver 里。
  private func coveredByAPanel(_ view: UIView) -> Bool {
    sequence(first: view, next: \.superview).contains { $0.accessibilityElementsHidden && $0.alpha == 0 && !$0.isUserInteractionEnabled }
  }

  /// 屏幕上标为 `face` 的符号键：隐藏的 Dachen 符号行里有很多同样的标签。
  private func shownSymbolKey(_ face: String, in controller: KeyboardViewController) -> UIButton? {
    descendants(controller.view).first { view in
      view.accessibilityLabel == "符号 \(face)" && sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden }
    } as? UIButton
  }

  /// Clear the whole composition the way the keyboard does it: holding delete past the repeat
  /// threshold wipes the composition instead of deleting one more character. There is no separate
  /// clear key to press -- the tests used to reach for a `nineKeyClear` that no keyboard has ever
  /// built, so they failed looking it up before asserting anything.
  private func clearComposition(in controller: KeyboardViewController) throws {
    let delete = try button("nineKeyDelete", in: controller)
    let begin = try XCTUnwrap(delete.actions(forTarget: controller, forControlEvent: .touchDown)?.first)
    controller.perform(NSSelectorFromString(begin))
    controller.perform(NSSelectorFromString("repeatBackspace"))
  }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    // Name the identifier: a bare "expected non-nil value of type UIButton" from a test that
    // looks up a dozen keys says nothing about which one went missing.
    try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == identifier
    } as? UIButton, "No button with accessibility identifier \(identifier).")
  }

  /// 功能菜单的一个图块：是普通 control 而不是按钮，所以皮肤处理不会把它画成键帽。
  private func tile(_ identifier: String, in controller: KeyboardViewController) throws -> UIControl {
    try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == identifier
    } as? UIControl, "No control with accessibility identifier \(identifier).")
  }

  /// 分页面板里 `view` 所在的页，按它在面板滚动内容中的布局位置推算。
  private func page(of view: UIView, in scroll: UIScrollView) -> Int {
    guard scroll.bounds.width > 0 else { return 0 }
    return Int((view.convert(view.bounds, to: scroll).midX / scroll.bounds.width).rounded(.down))
  }

  /// 键区里的面板：从顶行底部到键区底部，工具栏仍显示在其上方，按键藏在其下方。
  private func assertInKeyArea(_ panel: UIView, of controller: KeyboardViewController,
                               file: StaticString = #filePath, line: UInt = #line) throws {
    controller.view.layoutIfNeeded()
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" }, file: file, line: line)
    let space = try button("spaceKey", in: controller)
    let frame = panel.convert(panel.bounds, to: controller.view)
    XCTAssertEqual(frame.minY, strip.convert(strip.bounds, to: controller.view).maxY, accuracy: 0.5, "the panel starts under the top row", file: file, line: line)
    XCTAssertEqual(frame.maxY, space.convert(space.bounds, to: controller.view).maxY, accuracy: 0.5, "the panel ends with the key area", file: file, line: line)
    XCTAssertFalse(strip.isHidden, file: file, line: line)
    let toolbar = try XCTUnwrap(descendants(strip).first { $0.accessibilityIdentifier == "keyboardShortcutBar" }, file: file, line: line)
    XCTAssertFalse(toolbar.isHidden, "the toolbar row stays visible above the panel", file: file, line: line)
    XCTAssertEqual(space.superview?.alpha, 0, "the keys are hidden under the panel", file: file, line: line)
  }

  func testLetterFacesAreLowercaseUntilShiftUppercasesThem() throws {
    // 设计稿在所有模式下都画小写键面；只有下一键会输入大写字母时键面才变成大写。无论哪种情况，引擎收到的都是小写输入。
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.quanpin, .japanese] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(
        x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()

      let a = try XCTUnwrap(
        descendants(controller.view).first { $0.accessibilityLabel == "字母 A" } as? UIButton)
      XCTAssertEqual(a.configuration?.title, "a", "\(scheme) draws lowercase letter faces")
      XCTAssertEqual(a.accessibilityLabel, "字母 A", "The pinyin face is not an uppercase keystroke")
      // 设计稿在手机上用固定的 22pt 键面字号，不用 Dynamic Type 样式。
      let transformer = try XCTUnwrap(a.configuration?.titleTextAttributesTransformer)
      let font = try XCTUnwrap(transformer.callAsFunction(AttributeContainer()).uiKit.font)
      XCTAssertEqual(font.pointSize, KeyboardViewController.letterFontSize(.phone))
      XCTAssertEqual(KeyboardViewController.letterFontSize(.phone), 22)
      XCTAssertEqual(KeyboardViewController.letterFontSize(.tablet), 20)

      a.sendActions(for: .primaryActionTriggered)
      XCTAssertNotNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidate-1" })
      XCTAssertEqual(a.configuration?.title, "a", "composing keeps lowercase faces")

      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(a.configuration?.title, "a", "English starts lowercase")

      try button("shiftButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(a.configuration?.title, "A", "English Shift should uppercase the face")
      XCTAssertEqual(a.accessibilityLabel, "大写 A", "English Shift should be announced as uppercase")
    }
  }

  func testShortcutsYieldToCandidatesWithoutMovingKeys() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousScript = ChineseOutputPreference.usesTraditional
    // The chip text is the subject here -- no ordinal, no stray whitespace. A gloss adds a second
    // line to that text and would fail the assertion for a reason this test is not about.
    let previousGloss = CandidateGlossPreference.enabled
    InputSchemePreference.scheme = .nineKey
    ChineseOutputPreference.usesTraditional = false
    CandidateGlossPreference.enabled = false
    defer {
      InputSchemePreference.scheme = previousScheme
      ChineseOutputPreference.usesTraditional = previousScript
      CandidateGlossPreference.enabled = previousGloss
    }
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      let toolbar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" })
      XCTAssertFalse(toolbar.isHidden)
      // 品牌标志是放在 30pt 圆底上的官方标志，在自己的槽位里居中，排在这一行最前面。
      let brand = try XCTUnwrap(descendants(toolbar).first { $0.accessibilityIdentifier == "keyboardBrandIcon" })
      XCTAssertEqual(brand.bounds.width, KeyboardBrandMarkButton.discSide, accuracy: 0.1)
      XCTAssertEqual(brand.bounds.height, KeyboardBrandMarkButton.discSide, accuracy: 0.1)
      let brandSlot = try XCTUnwrap(brand.superview)
      XCTAssertEqual(brand.center.x, brandSlot.bounds.midX, accuracy: 0.5)
      XCTAssertLessThan(brand.convert(brand.bounds, to: toolbar).maxX,
                        try button("schemeButton", in: controller).convert(try button("schemeButton", in: controller).bounds, to: toolbar).minX)
      for id in ["schemeButton", "emojiShortcut", "phrasesShortcut",
                 "skinShortcut", "moreShortcut", "dismissShortcut"] {
        let control = try button(id, in: controller)
        XCTAssertGreaterThanOrEqual(control.bounds.width, 40, id)
        XCTAssertGreaterThanOrEqual(control.bounds.height, 38, id)
      }
      XCTAssertTrue(try button("replyShortcut", in: controller).isHidden)
      XCTAssertNil(try button("skinShortcut", in: controller).menu)
      XCTAssertTrue(try button("layoutVoiceShortcut", in: controller).isHidden)
      try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
      try tile("moreCard-繁体", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertTrue(ChineseOutputPreference.usesTraditional)
      try tile("moreCard-繁体", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(ChineseOutputPreference.usesTraditional)
      try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" })
      let key = try button("nineKey6", in: controller)
      let frame = key.convert(key.bounds, to: controller.view)
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Keyboard shortcuts \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      for digit in "64426" { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
      controller.view.layoutIfNeeded()
      XCTAssertTrue(toolbar.isHidden)
      // The chip carries the candidate and nothing else: a touch keyboard has no number row, so a
      // leading ordinal is noise that reads as part of the word.
      let chip = try XCTUnwrap(button("candidate-1", in: controller).configuration?.title)
      XCTAssertTrue(chip.contains("你好"))
      XCTAssertFalse(chip.contains(where: \.isNumber), chip)
      XCTAssertEqual(chip, chip.trimmingCharacters(in: .whitespaces), chip)
      XCTAssertEqual(key.convert(key.bounds, to: controller.view), frame)
      try clearComposition(in: controller)
      controller.view.layoutIfNeeded()
      XCTAssertFalse(toolbar.isHidden)
      XCTAssertEqual(key.convert(key.bounds, to: controller.view), frame)
    }
  }

  func testNewCandidatesAndPagesReturnToLeadingCandidate() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.nineKey, .quanpin] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 320, height: KeyboardViewController.defaultKeyboardHeight)
      func type(_ text: String) throws {
        for character in text {
          let key: UIButton
          if scheme == .nineKey {
            key = try button("nineKey\(character)", in: controller)
          } else {
            key = try XCTUnwrap(descendants(controller.view).first {
              $0.accessibilityLabel == "字母 \(String(character).uppercased())"
            } as? UIButton)
          }
          key.sendActions(for: .primaryActionTriggered)
        }
        controller.view.layoutIfNeeded()
      }
      try type(scheme == .nineKey ? "6" : "n")
      let candidate = try button("candidate-1", in: controller)
      var ancestor = candidate.superview
      while ancestor != nil && !(ancestor is UIScrollView) { ancestor = ancestor?.superview }
      let scroll = try XCTUnwrap(ancestor as? UIScrollView)
      XCTAssertFalse(scroll.delaysContentTouches)
      XCTAssertTrue(scroll.canCancelContentTouches)
      XCTAssertTrue(scroll.touchesShouldCancel(in: candidate))
      XCTAssertGreaterThan(scroll.contentSize.width, scroll.bounds.width + 40)
      scroll.setContentOffset(CGPoint(x: 40, y: 0), animated: false)
      try type(scheme == .nineKey ? "4" : "i")
      XCTAssertEqual(scroll.contentOffset.x, 0, accuracy: 0.5)
      let leading = try button("candidate-1", in: controller)
      XCTAssertGreaterThanOrEqual(leading.convert(leading.bounds, to: scroll).minX, 0)

      // The strip shows a page (nine by default); everything past them is reached by expanding rather than by paging
      // nine at a time, which for a query answering with hundreds left the tail unreachable.
      let expand = try button("expandCandidates", in: controller)
      XCTAssertFalse(expand.isHidden)
      expand.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let panel = try XCTUnwrap(
        descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })
      let chips = descendants(panel).compactMap { $0.accessibilityIdentifier }
        .filter { $0.hasPrefix("panelCandidate-") }
      XCTAssertGreaterThan(chips.count, CandidatePageSizePreference.defaultSize)
      // 全拼九键的展开面板由右栏的「返回」收起，其他方案是标题行的收起按钮。
      let closeIdentifier = scheme == .nineKey ? "candidatePanelBack" : "closeCandidatePanel"
      let close = try XCTUnwrap(
        descendants(panel).first { $0.accessibilityIdentifier == closeIdentifier } as? UIButton)
      close.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })
    }
  }

  func testSymbolKeyOpensAPanelInsteadOfAMenu() throws {
    let previous = InputSchemePreference.scheme
    // The panel leads with 最近 once anything was picked, which shifts every category index; this test itself records `@`, so a second run on the same device would otherwise start from its own leftovers.
    let defaults = KeyboardFeedbackPreference.defaults
    let previousRecents = defaults.stringArray(forKey: KeyboardSymbolRecents.key)
    defer {
      InputSchemePreference.scheme = previous
      if let previousRecents { defaults.set(previousRecents, forKey: KeyboardSymbolRecents.key) }
      else { defaults.removeObject(forKey: KeyboardSymbolRecents.key) }
    }
    defaults.removeObject(forKey: KeyboardSymbolRecents.key)
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 393, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()

    // 拼音网格的 @# 键是面板入口；九键底行不再有 符 键。
    let key = try button("nineKey1", in: controller)
    XCTAssertEqual(key.accessibilityLabel, "符号")
    XCTAssertTrue(try button("symbolPanelKey", in: controller).isHidden)
    XCTAssertNil(key.menu, "这颗键不再弹菜单")
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })

    key.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })
    XCTAssertEqual(panel.bounds.size, controller.view.bounds.size, "面板整块盖住键盘区")
    let last = KeyboardSymbolPanelView.categories.count - 1
    for identifier in ["symbolCategory_0", "symbolCategory_\(last)", "closeSymbolPanel", "symbolLockKey", "symbolDeleteKey"] {
      XCTAssertNotNil(descendants(panel).first { $0.accessibilityIdentifier == identifier }, identifier)
    }
    let grid = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "symbolGrid" } as? UIScrollView)
    XCTAssertGreaterThan(grid.contentSize.height, grid.bounds.height, "符号多到一屏放不下时要能往下滚")

    let network = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "symbolCategory_\(last)" } as? UIButton)
    network.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotNil(descendants(panel).first { $0.accessibilityIdentifier == "symbolKey_http://" })

    let symbol = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "symbolKey_@" } as? UIButton)
    symbol.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })
  }

  func testKeyLayoutsKeepNineKeyHeight() throws {
    let previous = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previous
    }
    // 粤拼、注音、笔画等是要用户自己启用的方案，全新模拟器上默认不在启用列表里；不先全部启用，下面选不上它们，各自的布局分支就永远走不到。
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    for width in [320.0, 414.0] {
      InputSchemePreference.scheme = .nineKey
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      let reference = try button("nineKey6", in: controller).bounds.height
      // Handwriting has a taller canvas, covered by HandwritingTests. The kana nine-key panel
      // brings its own ⌫ / 空白 / 改行 and the action row is collapsed underneath it, so the
      // shared return key has no height to keep there; JapaneseNineKeyTests covers that layout.
      for scheme in ChineseInputScheme.allCases
      where scheme != .handwriting && scheme != .japaneseNineKey {
        InputSchemePreference.scheme = scheme
        // 笔画和注音只在测试宿主带了 msime-stroke.db、msime-zhuyin.db 时才能选上（CI 不带）；没带时上面的赋值落到别的方案，那个方案已经单独测过。
        if [.stroke, .zhuyin].contains(scheme) && InputSchemePreference.scheme != scheme { continue }
        // 韩语方案在候选栏里常留一行训音（2758a0ebc，#2615），键盘为这一行长高而不是从按键里扣，所以视图要按方案自己要的高度给，按键才保持九键高度。
        let keyboardHeight = KeyboardViewController.keyboardHeight(
          glossLines: KeyboardViewController.stripGlossLines(scheme: scheme, fullAccess: false, onlineRoute: false))
        controller.view.frame.size.height = keyboardHeight
        controller.viewWillAppear(false)
        for symbols in [false, true] {
          if symbols { try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered) }
          controller.view.layoutIfNeeded()
          if scheme == .zhuyin && !symbols {
            // The Dachen layout fits four rows of keys above the action row into the same keyboard height, as the system Zhuyin keyboard does, so its rows are shorter than the three letter rows and all of them stay the same height.
            let returnHeight = try button("returnKey", in: controller).bounds.height
            XCTAssertLessThan(returnHeight, reference)
            XCTAssertGreaterThanOrEqual(returnHeight, 30)
            XCTAssertEqual(try button("zhuyinKeyq", in: controller).bounds.height, returnHeight, accuracy: 0.5)
          } else {
            XCTAssertEqual(try button("returnKey", in: controller).bounds.height, reference, accuracy: 0.5)
          }
          if scheme == .stroke {
            // 笔画键在九键外框里占九键网格的位置：两行键填满三行的高度。符号层和 Android 一样是新设计的 123 / #+= 层，不再回到九键网格。
            let shown = { (view: UIView) in sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden } }
            let stroke = try button("strokeKeyh", in: controller)
            XCTAssertEqual(shown(stroke), !symbols)
            XCTAssertFalse(shown(try button("nineKey6", in: controller)))
            XCTAssertEqual(shown(try button("symbolLayerKey0_0", in: controller)), symbols)
            if !symbols { XCTAssertGreaterThan(stroke.bounds.height, reference) }
            XCTAssertEqual(shown(try button("nineKeyDelete", in: controller)), !symbols)
            if !symbols { XCTAssertEqual(try button("nineKeyMiddleKey", in: controller).configuration?.title, "重输") }
            XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, symbols ? "拼音" : "123")
          }
          if symbols && ![.nineKey, .japaneseNineKey, .zhuyin].contains(scheme) {
            // 其他所有界面都画设计稿的 123 层，其按键与字母键等高。
            XCTAssertEqual(try button("symbolLayerKey0_0", in: controller).bounds.height, reference, accuracy: 0.5, "\(scheme)")
            XCTAssertFalse(try button("symbolLayerDeleteKey", in: controller).isHidden)
          }
          XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, keyboardHeight, "\(scheme)")
          if !symbols && [.nineKey, .quanpin].contains(scheme) {
            // 输入方式 自己绘制设计稿的线框图标，不带会被皮肤处理画成键帽的 configuration。
            let selector = try XCTUnwrap(try button("schemeButton", in: controller) as? KeyboardToolbarButton)
            XCTAssertGreaterThanOrEqual(selector.bounds.width, 40)
            XCTAssertNil(selector.configuration)
            XCTAssertEqual(selector.icon, .toolbarScheme)
            XCTAssertEqual(selector.accessibilityLabel, "选择输入方案")
            for id in ["emojiShortcut", "skinShortcut", "moreShortcut", "dismissShortcut"] {
              XCTAssertGreaterThanOrEqual(try button(id, in: controller).bounds.width, 40, id)
            }
            if width == 320 {
              let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
                controller.view.layer.render(in: context.cgContext)
              })
              attachment.name = "Narrow \(scheme.rawValue) keyboard and toolbar"
              attachment.lifetime = .keepAlways
              add(attachment)
            }
          }
          let punctuation = try button("quickPunctuationKey", in: controller)
          // 手写板沿用手机 26 键的底行，与 Android 的 designEntries 一致，所以它的 ， 显示在空格键旁边。
          XCTAssertEqual(punctuation.isHidden, symbols || [.nineKey, .japaneseNineKey, .stroke].contains(scheme))
          if !punctuation.isHidden {
            XCTAssertEqual(punctuation.configuration?.title, scheme.isJapanese ? "、" : scheme.writesAsciiPunctuation ? "," : "，")
            // 手机 26 键底行按相对空格键的比例分配宽度：， 占一份，空格占四份。
            let space = try button("spaceKey", in: controller)
            XCTAssertEqual(punctuation.bounds.width, space.bounds.width / KeyboardViewController.phoneSpaceWeight, accuracy: 0.5)
            XCTAssertGreaterThanOrEqual(space.bounds.width, 79.2)
            XCTAssertEqual(punctuation.menu?.children.count, 7)
          }
          // 底行的 ⌫ 只属于 Dachen 符号行：123 / #+= 层的 ⌫ 在第三行，九键网格的在右列，假名网格的在它自己的网格里。
          XCTAssertEqual(try button("symbolDeleteKey", in: controller).isHidden, !(symbols && scheme == .zhuyin))
          // 全拼 14 键画自己的三排键，键位与等高由 FourteenKeyKeyboardTests 覆盖。
          if !symbols && ![.nineKey, .japaneseNineKey, .handwriting, .zhuyin, .stroke, .fourteenKey].contains(scheme) {
            let delete = try button("letterDeleteKey", in: controller)
            let shift = try button("shiftButton", in: controller)
            // Korean keys are named by the jamo they type.
            let letterLabel = { (letter: String) in
              "字母 \(scheme.isKorean ? DubeolsikKeyLayout.keycap(for: letter, shifted: false) ?? letter : letter)"
            }
            let m = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == letterLabel("M") } as? UIButton)
            XCTAssertEqual(delete.superview, m.superview)
            XCTAssertGreaterThan(delete.frame.minX, m.frame.maxX)
            XCTAssertEqual(delete.bounds.width, 44, accuracy: 0.5)
            XCTAssertEqual(shift.bounds.width, 44, accuracy: 0.5)
            XCTAssertEqual(delete.bounds.height, reference, accuracy: 0.5)
            XCTAssertLessThanOrEqual(delete.convert(delete.bounds, to: controller.view).maxX, width - KeyboardFormFactor.phone.padding.trailing + 0.5)
            for label in ["Q", "A", "Z", "P", "L", "M"] {
              let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == letterLabel(label) } as? UIButton)
              XCTAssertEqual(key.bounds.height, reference, accuracy: 0.5)
              let frame = key.convert(key.bounds, to: controller.view)
              XCTAssertGreaterThanOrEqual(frame.minX, KeyboardFormFactor.phone.padding.leading - 0.5)
              XCTAssertLessThanOrEqual(frame.maxX, controller.view.bounds.width - KeyboardFormFactor.phone.padding.trailing + 0.5, "\(scheme) \(label) frame \(frame)")
            }
          }
          if !symbols && scheme == .japaneseNineKey {
            XCTAssertEqual(try button("japaneseKana0", in: controller).bounds.height, reference, accuracy: 0.5)
          }
          if symbols { try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered) }
        }
      }
    }
  }

  func testLocalModeLayouts() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.nineKey, .shuangpin] {
      InputSchemePreference.scheme = scheme
      for trigger in ["U", "T", "J"] {
        let controller = KeyboardViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
        controller.openLocalInputMode(trigger)
        controller.view.layoutIfNeeded()
        let returnKey = try button("returnKey", in: controller)
        for label in ["Q", "A", "Z", "P", "L", "M"] {
          let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(label)" } as? UIButton)
          XCTAssertEqual(key.bounds.height, returnKey.bounds.height, accuracy: 0.5)
          XCTAssertGreaterThanOrEqual(key.bounds.height, KeyboardHeightPercent.portraitKeyHeight(tablet: false) - 0.5)
          XCTAssertTrue(key.subviews.compactMap { $0 as? UILabel }.filter { $0.font.pointSize == 9 }.allSatisfy { $0.isHidden })
        }
        XCTAssertFalse(try button("exitLocalModeButton", in: controller).isHidden)
        let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
          controller.view.layer.render(in: context.cgContext)
        })
        attachment.name = "Local mode \(scheme) \(trigger)"
        attachment.lifetime = .keepAlways
        add(attachment)
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        // 本地工具输出 ASCII，所以它们用英文层，九键键盘上也一样。
        XCTAssertNotNil(shownSymbolKey(":", in: controller))
        XCTAssertNil(shownSymbolKey("：", in: controller))
        XCTAssertNil(shownSymbolKey("、", in: controller))
        XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "ABC")
        try button("exitLocalModeButton", in: controller).sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        XCTAssertTrue(try button("exitLocalModeButton", in: controller).isHidden)
        XCTAssertFalse(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" }).isHidden)
        XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, scheme.title)
        XCTAssertEqual(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden, scheme != .nineKey)
      }
    }
  }

  func testJapaneseSentenceConversionAndMemory() throws {
    func residentBytes() -> UInt64 {
      var info = mach_task_basic_info_data_t()
      var count = mach_msg_type_number_t(MemoryLayout.size(ofValue: info) / MemoryLayout<integer_t>.size)
      let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
          task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
        }
      }
      return result == KERN_SUCCESS ? info.resident_size : 0
    }
    func footprint() -> UInt64 {
      var info = task_vm_info_data_t()
      var count = mach_msg_type_number_t(MemoryLayout.size(ofValue: info) / MemoryLayout<integer_t>.size)
      let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
          task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
        }
      }
      return result == KERN_SUCCESS ? info.phys_footprint : 0
    }
    let footprintBefore = footprint()
    let before = residentBytes()
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToJapanese()
    var snapshot = bridge.cancel()
    for letter in "watashihanihonjindesu" { snapshot = bridge.handleCharacter(String(letter)) }
    print("Japanese sentence candidates: \(snapshot.candidates.prefix(5))")
    XCTAssertTrue(snapshot.candidates.contains { $0.contains("日本人") && $0.contains("です") })
    XCTAssertNil(snapshot.diagnosticText)
    let after = residentBytes()
    let footprintAfter = footprint()
    print("Japanese physical footprint: before=\(footprintBefore), after=\(footprintAfter) bytes")
    XCTAssertGreaterThan(footprintBefore, 0)
    XCTAssertLessThan(footprintAfter > footprintBefore ? footprintAfter - footprintBefore : 0, 16 * 1024 * 1024,
      "The keyboard must not copy the full sentence model into private memory")
    var usage = rusage()
    getrusage(RUSAGE_SELF, &usage)
    print("Japanese model memory: before=\(before), after=\(after), peak=\(usage.ru_maxrss) bytes")
    let index = try XCTUnwrap(snapshot.candidates.firstIndex { $0.contains("日本人") && $0.contains("です") })
    XCTAssertEqual(bridge.selectCandidate(at: UInt(index)).commitText, snapshot.candidates[index])
    _ = bridge.switch(toShuangpin: false)
    _ = bridge.openLocalMode("R")
    for letter in "nihon" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("日本"))
    _ = bridge.cancel()
    XCTAssertFalse(bridge.isInLocalMode)
  }

  func testJapaneseSentenceCandidateStrip() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousScript = ChineseOutputPreference.usesTraditional
    InputSchemePreference.scheme = .japanese
    ChineseOutputPreference.usesTraditional = true
    defer {
      InputSchemePreference.scheme = previousScheme
      ChineseOutputPreference.usesTraditional = previousScript
    }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
    for letter in "watashihanihonjindesu" {
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(String(letter).uppercased())" } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("私は日本人です"))
    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    })
    attachment.name = "Japanese full sentence keyboard"
    attachment.lifetime = .keepAlways
    add(attachment)
  }

  // Putting the keyboard away must hand the engine's dictionary access back: iOS terminates an
  // extension suspended while holding a lock in the App Group container and reports it as
  // 0xdead10cc. Resuming has to rebuild a session that still converts.
  // The presentation cycle is what the device actually does: show, type, put away, show again.
  // Releasing the dictionary access on dismissal must not leave the next presentation unable to
  // convert, and a dropped keystroke would still have played its click.
  /// 工具栏和候选共用一个顶行（dc.html：一行 50pt）：空闲时只有工具栏，图标在整行里居中；组字时由 12pt 拼写行和候选行在原位替换它，顶行下方的内容不移动。
  func testComposingSwapsTheToolbarForTheCandidatesInTheSameRow() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.applyInputContext(keyboardType: .default, documentIdentifier: UUID())
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    let toolbar = try XCTUnwrap(descendants(strip).first { $0.accessibilityIdentifier == "keyboardShortcutBar" })
    let reading = try XCTUnwrap(descendants(strip).first { $0.accessibilityIdentifier == "compositionRow" })
    let space = try button("spaceKey", in: controller)
    let idleStrip = strip.frame
    let idleSpace = space.convert(space.bounds, to: controller.view)
    XCTAssertEqual(idleStrip.height, KeyboardViewController.topRowHeight(
      glossLines: KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: false)))
    XCTAssertGreaterThanOrEqual(idleStrip.height, KeyboardViewController.topRowMinimumHeight)
    XCTAssertFalse(toolbar.isHidden)
    XCTAssertTrue(reading.isHidden, "no empty composition line is reserved over the toolbar")
    XCTAssertEqual(toolbar.frame.minY, 0, accuracy: 0.5, "the toolbar takes the whole row")
    XCTAssertEqual(toolbar.frame.height, idleStrip.height, accuracy: 0.5)
    XCTAssertEqual(strip.layer.cornerRadius, 0)

    for letter in "nihao" {
      let key = try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityLabel == "字母 \(String(letter).uppercased())"
      } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    XCTAssertTrue(toolbar.isHidden)
    XCTAssertFalse(reading.isHidden)
    XCTAssertEqual(reading.frame.minY, 0, accuracy: 0.5)
    XCTAssertEqual(reading.frame.height, KeyboardViewController.readingRowHeight, accuracy: 0.5)
    let preedit = try button("preeditButton", in: controller)
    XCTAssertEqual(preedit.configuration?.title, "nihao")
    let transformer = try XCTUnwrap(preedit.configuration?.titleTextAttributesTransformer)
    let attributes = transformer.callAsFunction(AttributeContainer())
    XCTAssertEqual(attributes.uiKit.font?.pointSize, KeyboardViewController.preeditFontSize)
    XCTAssertEqual(try XCTUnwrap(attributes.uiKit.kern), KeyboardViewController.preeditFontSize * 0.02, accuracy: 0.001)
    let candidate = try button("candidate-1", in: controller)
    XCTAssertGreaterThanOrEqual(candidate.convert(candidate.bounds, to: strip).minY, reading.frame.maxY - 0.5)
    XCTAssertEqual(strip.frame, idleStrip, "the row keeps its height while composing")
    XCTAssertEqual(space.convert(space.bounds, to: controller.view), idleSpace, "the keys do not move")
  }

  /// 显示方式 隐藏 以及 常用语 / 输入方式 两个开关，对应 Android 的 `toolbar_hidden`、`toolbar_phrase` 和 `toolbar_scheme`：隐藏时空闲顶行及其下方的间隙消失，按键上移；组字时读音行和候选行以顶行的常规高度回来，但不带工具栏，上屏后顶行再次消失。两个开关会把各自的按钮从工具栏上去掉。
  func testHiddenToolbarCollapsesTheIdleRowAndTheSwitchesDropTheirButtons() throws {
    let defaults = TouchToolbarLocalPreference.defaults
    let keys = TouchToolbarLocalPreference.keys
    let saved = keys.map { defaults.object(forKey: $0) }
    let previous = InputSchemePreference.scheme
    defer {
      InputSchemePreference.scheme = previous
      for (key, value) in zip(keys, saved) {
        if let value { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
      }
    }
    InputSchemePreference.scheme = .quanpin
    keys.forEach(defaults.removeObject(forKey:))
    TouchToolbarLocalPreference.hidden = true
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.applyInputContext(keyboardType: .default, documentIdentifier: UUID())
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    let toolbar = try XCTUnwrap(descendants(strip).first { $0.accessibilityIdentifier == "keyboardShortcutBar" })
    let reading = try XCTUnwrap(descendants(strip).first { $0.accessibilityIdentifier == "compositionRow" })
    let space = try button("spaceKey", in: controller)
    let rowHeight = KeyboardViewController.topRowHeight(
      glossLines: KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: false))
    XCTAssertEqual(strip.frame.height, 0, accuracy: 0.5)
    XCTAssertTrue(toolbar.isHidden)
    XCTAssertTrue(reading.isHidden)
    let idleSpace = space.convert(space.bounds, to: controller.view)
    let keyboardHeight = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let idleHeight = keyboardHeight.constant

    for letter in "ni" {
      let key = try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityLabel == "字母 \(String(letter).uppercased())"
      } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    XCTAssertEqual(strip.frame.height, rowHeight, accuracy: 0.5)
    XCTAssertTrue(toolbar.isHidden, "the bar stays hidden while composing")
    XCTAssertFalse(reading.isHidden)
    XCTAssertFalse(try button("candidate-1", in: controller).isHidden)
    XCTAssertGreaterThan(space.convert(space.bounds, to: controller.view).minY, idleSpace.minY,
                         "the row comes back above the keys")
    // 收起时顶栏和它下面的间隔一起去掉，键盘高度的差正好是这两样，按键高度不跟着组字跳。
    XCTAssertEqual(keyboardHeight.constant - idleHeight, rowHeight + KeyboardFormFactor.phone.topRowGap, accuracy: 0.5)

    try button("candidate-1", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(strip.frame.height, 0, accuracy: 0.5, "committing leaves nothing on the row")
    XCTAssertEqual(space.convert(space.bounds, to: controller.view).minY, idleSpace.minY, accuracy: 0.5)

    TouchToolbarLocalPreference.hidden = false
    TouchToolbarLocalPreference.phrases = false
    TouchToolbarLocalPreference.scheme = false
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(strip.frame.height, rowHeight, accuracy: 0.5)
    XCTAssertFalse(toolbar.isHidden)
    XCTAssertTrue(try button("phrasesShortcut", in: controller).isHidden)
    XCTAssertTrue(try button("schemeButton", in: controller).isHidden)
    XCTAssertFalse(try button("moreShortcut", in: controller).isHidden)
    XCTAssertFalse(try button("dismissShortcut", in: controller).isHidden)
  }

  func testKeyboardStillConvertsAfterBeingPutAwayAndShownAgain() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
    controller.applyInputContext(keyboardType: .default, documentIdentifier: UUID())
    for round in 0..<2 {
      controller.viewWillAppear(false)
      controller.view.layoutIfNeeded()
      for letter in "nihao" {
        let key = try XCTUnwrap(descendants(controller.view).first {
          $0.accessibilityLabel == "字母 \(String(letter).uppercased())"
        } as? UIButton)
        key.sendActions(for: .primaryActionTriggered)
      }
      controller.view.layoutIfNeeded()
      XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "nihao",
                     "round \(round): the keystrokes never reached the engine")
      XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"),
                    "round \(round): no Chinese candidate")
      controller.viewWillDisappear(false)
    }
  }

  func testSuspendReleasesDictionaryAccessAndResumeStillConverts() throws {
    // 用自己的状态目录。默认目录是模拟器里各用例共用的偏好文档，前面的用例经由键盘选过的方案（比如五笔）会留在里面，这里的全拼输入就得不到「你好」。
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-suspend-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    var snapshot = bridge.cancel()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"))
    XCTAssertFalse(bridge.suspendDictionarySession(), "an open composition still owns the session")
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    XCTAssertTrue(bridge.suspendDictionarySession(), "suspending twice is not an error")
    // Pausing learning would leave the session, and its dictionary access, very much alive. Only a
    // destroyed session hands the access back, and a destroyed session cannot answer a keystroke.
    let whileSuspended = bridge.handleCharacter("n")
    XCTAssertNotNil(whileSuspended.diagnosticText, "the session is paused, not released")
    XCTAssertTrue(whileSuspended.candidates.isEmpty)
    try bridge.resumeDictionarySession()
    snapshot = bridge.cancel()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertNil(snapshot.diagnosticText)
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session lost its dictionaries")
  }

  // Nine-key is engine-session state, not one of the preferences the prepared options carry, so a
  // session rebuilt after a dismissal starts back on the 26-key layout. The keyboard keeps sending
  // the digits its layout produces and the engine, expecting letters, answers with nothing at all:
  // no preedit, no candidates, no diagnostic.
  func testNineKeySurvivesTheSessionBeingReleasedAndRebuilt() throws {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToNineKey()
    var snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "nine-key never reached the engine")
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertFalse(snapshot.preedit.isEmpty, "the rebuilt session ignored the digits entirely")
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session forgot nine-key mode")
  }

  // A dismissal is not the only rebuild. Reading the personal dictionary hands the shared
  // dictionary lease back the same way, and the keyboard refreshes that list on every appearance:
  // the nine-key layout the host kept drawing was sitting on a 26-key engine from the first
  // refresh onwards, which is why typing only started working after a trip through 26 keys.
  func testNineKeySurvivesTheSessionRebuiltForDictionaryMaintenance() throws {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToNineKey()
    var snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "nine-key never reached the engine")
    _ = bridge.cancel()
    _ = try bridge.personalEntries(atOffset: 0)
    snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertFalse(snapshot.preedit.isEmpty, "the rebuilt session ignored the digits entirely")
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session forgot nine-key mode")
  }

  // A session is created from the shared document, so a scheme picked in an earlier session has to
  // reach it. Nothing on iOS ever wrote the touch layout there, so a cold keyboard created its
  // session on 26 keys no matter what the user had picked, and only the live session knew better.
  func testTouchSchemeReachesTheDocumentTheNextSessionIsCreatedFrom() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-scheme-persist-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    var first: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(try XCTUnwrap(first).setTouchKeyboardScheme(
      .nineKey, enabledSchemes: [.quanpin, .nineKey]))
    first = nil

    let next = MetasequoiaInputSessionBridge(stateRoot: state)
    var snapshot = next.cancel()
    for digit in "64426" { snapshot = next.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the new session did not start on nine-key")
    XCTAssertEqual(try XCTUnwrap(next.sharedPreferences)["touch_keyboard_layout"] as? String,
                   "nine_key")
  }

  // The scheme is not the only thing the keyboard itself can change. Every one of these used to
  // live on the session alone, so reloading the settings app's document put its older value back
  // over the choice the user had just made, and a new session never saw the choice at all.
  func testKeyboardSideSelectionsReachTheSharedDocument() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-selection-persist-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    var first: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(stateRoot: state)
    let bridge = try XCTUnwrap(first)
    XCTAssertTrue(bridge.updateTheme(GlobalThemePreference.selecting("night")))
    XCTAssertTrue(bridge.setTraditionalChineseOutput(true))
    XCTAssertTrue(bridge.persistTouchKeyboardGeometry(
      keySpacing: 5, rowSpacing: 9, heightAdjustment: 12, voiceEnabled: true))
    // A drag reports on every gesture frame and only previews on the live session; taking a file
    // lock that often is what the separate commit above is for.
    XCTAssertTrue(bridge.setTouchKeyboardGeometry(
      keySpacing: 3, rowSpacing: 4, heightAdjustment: -5, voiceEnabled: false))
    first = nil

    let next = MetasequoiaInputSessionBridge(stateRoot: state)
    let preferences = try XCTUnwrap(next.sharedPreferences)
    XCTAssertEqual(preferences["global_theme"] as? String, "night")
    XCTAssertEqual(preferences["traditional_chinese_output"] as? Bool, true)
    XCTAssertEqual(preferences["touch_key_spacing_tenths"] as? Int, 50)
    XCTAssertEqual(preferences["touch_row_spacing_tenths"] as? Int, 90)
    XCTAssertEqual(preferences["touch_keyboard_height_adjustment"] as? Int, 12)
    XCTAssertEqual(preferences["touch_voice_shortcut"] as? Bool, true)
  }

  // Reloading the settings app's document replaced the session's preferences wholesale, dropping
  // the two values this host sets for itself along with them.
  func testSharedPreferenceReloadKeepsTheHostSessionContract() async throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-reload-overrides-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setTouchKeyboardScheme(.nineKey, enabledSchemes: [.quanpin, .nineKey]))
    let reloaded = expectation(description: "shared preferences reloaded")
    bridge.reloadSharedPreferences { accepted in
      XCTAssertTrue(accepted)
      reloaded.fulfill()
    }
    await fulfillment(of: [reloaded], timeout: 15)

    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["candidate_page_size"] as? Int, 9)
    XCTAssertEqual(preferences["default_ime_mode"] as? String, "chinese")
    XCTAssertEqual(preferences["touch_keyboard_layout"] as? String, "nine_key")
    var snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the reloaded document lost nine-key")
  }

  /// The key face has to name what the keys actually produce.
  ///
  /// The hints used to come from a copy of the keymap kept in this target, and that copy
  /// had lost Xiaohe's `uai` from K - the key that types 乖 carried no sign of it. They now
  /// come from the Engine's own profile through the shared ABI, so this drives a real
  /// session per profile and checks the face against the key that produced candidates.
  func testShuangpinKeyHintsComeFromTheProfileTheSessionRuns() throws {
    let bridge = MetasequoiaInputSessionBridge()
    // The key each profile puts `uai` on, reached with the `g` initial.
    for (profile, key) in [("xiaohe", "K"), ("ziranma", "Y"), ("shoudao", "G"), ("microsoft", "Y")] {
      _ = bridge.switch(toShuangpinProfile: profile)
      _ = bridge.cancel()
      let hints = bridge.shuangpinKeyHints()
      XCTAssertGreaterThanOrEqual(hints.count, 26, "\(profile) labelled only \(hints.count) keys")
      XCTAssertEqual(hints[key]?.contains("uai"), true,
                     "\(profile) key \(key) reads \(hints[key] ?? "nothing") but types uai")
      _ = bridge.handleCharacter("g")
      XCTAssertFalse(bridge.handleCharacter(key.lowercased()).candidates.isEmpty,
                     "\(profile) g\(key.lowercased()) produced no candidates")
      _ = bridge.cancel()
    }
    // Both units K carries, not just the first one.
    _ = bridge.switch(toShuangpinProfile: "xiaohe")
    XCTAssertEqual(bridge.shuangpinKeyHints()["K"], "ing uai")
    // Initials and finals stay on their own side of the separator.
    XCTAssertEqual(bridge.shuangpinKeyHints()["V"], "zh / ui ü")
    // A session that is not running double pinyin gets no face rather than the default one's.
    _ = bridge.switch(toShuangpin: false)
    XCTAssertTrue(bridge.shuangpinKeyHints().isEmpty, "Quanpin was labelled with a double-pinyin face")
    _ = bridge.cancel()
  }

  /// 「双拼键位提示」关掉后，双拼 26 键的字母键不画提示：提示行隐藏、字母不再为它让出下边距，读屏也不再读它。
  func testTheShuangpinKeyHintSwitchDropsTheHintLineAndItsReading() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousHints = KeyboardLayoutPreference.shuangpinKeyHints
    defer {
      InputSchemePreference.scheme = previousScheme
      KeyboardLayoutPreference.shuangpinKeyHints = previousHints
    }
    InputSchemePreference.scheme = .shuangpin

    func letterU(hints: Bool) throws -> (button: UIButton, controller: KeyboardViewController) {
      KeyboardLayoutPreference.shuangpinKeyHints = hints
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      let button = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 U" } as? UIButton)
      return (button, controller)
    }
    // 提示行是直接加在按键上的 9pt 标签，见 `attachHintLabel`。
    func hintLabel(of button: UIButton) throws -> UILabel {
      try XCTUnwrap(button.subviews.compactMap { $0 as? UILabel }.first { $0.font.pointSize == 9 })
    }

    let shown = try letterU(hints: true)
    let reading = try XCTUnwrap(shown.button.accessibilityValue, "shuangpin labels U by default")
    XCTAssertFalse(reading.isEmpty)
    XCTAssertEqual(try hintLabel(of: shown.button).text, reading)
    XCTAssertFalse(try hintLabel(of: shown.button).isHidden)
    XCTAssertEqual(shown.button.configuration?.contentInsets.bottom, 11)

    let hidden = try letterU(hints: false)
    XCTAssertNil(hidden.button.accessibilityValue, "VoiceOver no longer reads the hint")
    XCTAssertTrue(try hintLabel(of: hidden.button).isHidden)
    XCTAssertEqual(hidden.button.configuration?.contentInsets.bottom, 0, "the letter drops back to the centre")
    XCTAssertEqual(hidden.button.accessibilityLabel, "字母 U")
  }

  /// 键盘开着时在设置里切换「双拼键位提示」：键盘再次出现时从共享文档同步，已经画好的字母键当场跟着收回或画回提示，不必重建键盘。这里只改共享文档、不动 App Group 镜像，走的是 `synchronizeSharedTouchPreferences` 这条路。
  func testTheShuangpinKeyHintSwitchReachesAKeyboardThatIsAlreadyShown() throws {
    let key = KeyboardLayoutPreference.shuangpinKeyHintsDocumentKey
    let previousScheme = InputSchemePreference.scheme
    let previousHints = KeyboardLayoutPreference.shuangpinKeyHints
    let previousDocument = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    defer {
      InputSchemePreference.scheme = previousScheme
      MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = previousDocument }
      KeyboardLayoutPreference.shuangpinKeyHints = previousHints
    }
    InputSchemePreference.scheme = .shuangpin
    KeyboardLayoutPreference.shuangpinKeyHints = true
    // 文档里的方案要和键盘一致：会话重读文档后按 Engine 视图里的方案取提示表，文档还写着全拼时表是空的，开关打开也画不出提示。真实使用中用户选了双拼，文档里就是双拼。
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { document in
      document["scheme"] = "shuangpin"
      document["last_chinese_scheme"] = "shuangpin"
      document["shuangpin_profile"] = "xiaohe"
      document[key] = true
    })

    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let letterU = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 U" } as? UIButton)
    // 提示行是直接加在按键上的 9pt 标签，见 `attachHintLabel`。
    let hintLine = try XCTUnwrap(letterU.subviews.compactMap { $0 as? UILabel }.first { $0.font.pointSize == 9 })
    XCTAssertFalse(hintLine.isHidden)
    XCTAssertNotNil(letterU.accessibilityValue)

    // 共享文档在后台线程读入、回到主线程应用，所以按条件轮询，不按固定时长等待。
    func appear(until done: () -> Bool) {
      controller.viewWillAppear(false)
      let deadline = Date().addingTimeInterval(15)
      while !done() && Date() < deadline {
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
      }
    }

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = false })
    appear { hintLine.isHidden }
    XCTAssertTrue(hintLine.isHidden, "the keyboard kept drawing hints after the switch was turned off")
    XCTAssertNil(letterU.accessibilityValue, "VoiceOver still reads the hint")
    XCTAssertEqual(letterU.configuration?.contentInsets.bottom, 0)
    XCTAssertFalse(KeyboardLayoutPreference.shuangpinKeyHints, "the App Group mirror follows the document")

    // 再打开：同一个键盘画回提示。这一步也确认上面收回提示不是因为方案被同步成了全拼。
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = true })
    appear { !hintLine.isHidden }
    XCTAssertFalse(hintLine.isHidden, "the keyboard did not draw the hints again after the switch was turned on")
    XCTAssertNotNil(letterU.accessibilityValue)
    XCTAssertEqual(letterU.configuration?.contentInsets.bottom, 11)
  }

  func testAdditionalShuangpinProfilesAndKeyHints() throws {
    let bridge = MetasequoiaInputSessionBridge()
    for (profile, input) in [("ziranma", "nihk"), ("microsoft", "nihk"), ("shoudao", "nihd"), ("xiaohe", "nihc")] {
      _ = bridge.switch(toShuangpinProfile: profile)
      var snapshot = bridge.cancel()
      for letter in input { snapshot = bridge.handleCharacter(String(letter)) }
      XCTAssertTrue(snapshot.candidates.contains("你好"), profile)
      _ = bridge.cancel()
    }
    _ = bridge.switch(toShuangpinProfile: "microsoft")
    XCTAssertTrue(bridge.shuangpinKeyHints()[";"]?.contains("ing") == true)
    _ = bridge.handleCharacter("n")
    XCTAssertFalse(bridge.handleCharacter(";").candidates.isEmpty)
    _ = bridge.cancel()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.wubi, .japanese, .microsoft] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(try button("microsoftFinalKey", in: controller).isHidden, scheme != .microsoft)
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Keyboard scheme \(scheme)"
      attachment.lifetime = .keepAlways
      add(attachment)
    }
  }

  func testAdditionalEngineSchemesAndLocalProviders() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-schemes-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToWubi()
    var snapshot = bridge.handleCharacter("a")
    XCTAssertTrue(snapshot.candidates.contains("工"))
    XCTAssertEqual(bridge.selectCandidate(at: UInt(snapshot.candidates.firstIndex(of: "工")!)).commitText, "工")
    _ = bridge.switchToJapanese()
    for letter in "nihon" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("にほん"))
    let japaneseRows = try XCTUnwrap(try bridge.allCandidates()["candidates"] as? [[String: Any]])
    XCTAssertTrue(japaneseRows.contains { $0["text"] as? String == "ニホン" })
    _ = bridge.cancel()
    snapshot = bridge.handleCharacter("a")
    XCTAssertTrue(snapshot.candidates.contains("亜"))
    _ = bridge.cancel()
    _ = bridge.switch(toShuangpin: false)
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"))
    _ = bridge.cancel()
    for (trigger, input) in [("K", "yyds"), ("Y", "hello"), ("E", "smile"), ("M", "kaixin"), ("R", "nihon")] {
      _ = bridge.openLocalMode(trigger)
      XCTAssertTrue(bridge.isInLocalMode)
      for letter in input { snapshot = bridge.handleCharacter(String(letter)) }
      XCTAssertNil(snapshot.diagnosticText, "Provider \(trigger)")
      XCTAssertFalse(snapshot.candidates.isEmpty, "Provider \(trigger)")
      // Temporary English completes what was typed. This used to ask for more than one answer,
      // which counted rows in the pinned dictionary rather than describing the product: the
      // release `msime-english.db` now holds exactly one word beginning with "hello", so the count
      // moved while the behaviour did not.
      if trigger == "Y" {
        XCTAssertTrue(snapshot.candidates.contains { $0.lowercased().hasPrefix(input) },
                      "Provider Y answered \(snapshot.candidates) for \(input)")
      }
      if trigger == "K" { XCTAssertTrue(snapshot.candidates.contains("永远滴神")) }
      _ = bridge.cancel()
    }
  }

  func testSpellingStripReusesItsButtonsBetweenKeystrokes() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey]
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()

    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    try button("nineKey4", in: controller).sendActions(for: .primaryActionTriggered)
    let strip = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "nineKeySpellingStrip"
    } as? UIScrollView)
    let first = descendants(strip).compactMap { $0 as? UIButton }.filter { !$0.isHidden }
    XCTAssertFalse(first.isEmpty)

    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    let second = descendants(strip).compactMap { $0 as? UIButton }.filter { !$0.isHidden }
    XCTAssertFalse(second.isEmpty)
    for (before, after) in zip(first, second) {
      XCTAssertTrue(before === after)
    }
    XCTAssertTrue(second.contains { ($0.accessibilityIdentifier ?? "").hasPrefix("nineKeySpelling_") })
  }

  /// Nine-key digits reach the English candidates too.
  ///
  /// From a report: typing 65 on nine-key wanted `ok` and the strip had nothing. The digits are letter groups, so the mixed-English path has to see them the same way the Chinese one does; nothing in the host mapped them, and the failure looked like the word being missing from the dictionary rather than like the layout never asking.
  ///
  /// The shared `mixed_input.minimum_prefix` default is now 5, matching Windows, so a two-digit `ok` no longer reaches English at all; the check types the five digits of `hello` instead.
  func testNineKeyOffersEnglishForTheDigitsTyped() throws {
    let previousScheme = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = enabled
      InputSchemePreference.scheme = previousScheme
    }
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey]
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    for key in ["nineKey4", "nineKey3", "nineKey5", "nineKey5", "nineKey6"] {
      try button(key, in: controller).sendActions(for: .primaryActionTriggered)
    }
    let chips = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
      .compactMap { chip -> String? in
        chip.configuration?.attributedTitle.map { String($0.characters) } ?? chip.configuration?.title
      }
    XCTAssertTrue(chips.contains { $0.hasPrefix("hello") }, "nine-key 43556 offered no hello: \(chips)")
  }

  func testNineKeyInputAndLayoutSwitches() throws {
    let previous = InputSchemePreference.scheme
    InputSchemePreference.scheme = .nineKey
    defer { InputSchemePreference.scheme = previous }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 320, height: KeyboardViewController.keyboardHeight(landscape: true))
    controller.view.layoutIfNeeded()

    let nine = try button("nineKey6", in: controller)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    // 双拼只列一种（#6450）。
    let offered = InputSchemePreference.offeredSchemes
    let extraShuangpin = max(0, offered.filter { $0.shuangpinProfile != nil }.count - 1)
    XCTAssertEqual(descendants(controller.view).filter { $0.accessibilityIdentifier?.hasPrefix("schemeCard-") == true }.count, offered.count - extraShuangpin)
    // 高亮的 输入方式 图标关闭它自己的选择器。
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    for digit in "64426" {
      try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
    }
    XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
    try button("nineKeySpelling_ni", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
    controller.view.layoutIfNeeded()
    XCTAssertGreaterThanOrEqual(nine.bounds.height, 30)
    XCTAssertGreaterThan(nine.bounds.width, 60)
    let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    }
    let attachment = XCTAttachment(image: image)
    attachment.name = "Nine-key pinyin keyboard"
    attachment.lifetime = .keepAlways
    add(attachment)

    // The digit layer keeps the three-column grid and only swaps the legends, which is what
    // testNineKeyDigitLayerKeepsTheGridAndRestoresLetters pins down and what the Apple client
    // asserts here too. Switching language is the one that puts the grid away.
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try XCTUnwrap(nine.superview).isHidden)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 9 键")

    InputSchemePreference.scheme = .shuangpin
    controller.viewWillAppear(false)
    XCTAssertTrue(try XCTUnwrap(nine.superview).isHidden)
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "小鹤双拼")
  }

  // 候选条能横向滚，所以放不下的候选应该滚出去，不是在 chip 里折成两行。
  func testCandidateChipsNeverWrapToASecondLine() throws {
    let previous = InputSchemePreference.scheme
    // Glosses are on by default and reserve a line of their own under the candidate, which is a
    // different question from the one this test asks: whether a chip too wide for the row breaks
    // instead of truncating. Pin the preference so the measurement is of wrapping alone.
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = false
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: 320, height: KeyboardViewController.defaultKeyboardHeight)
    for digit in "926439" {
      try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    let chips = descendants(controller.view).compactMap { view -> UIButton? in
      guard let button = view as? UIButton,
            let identifier = button.accessibilityIdentifier,
            identifier.hasPrefix("candidate-") else { return nil }
      return button
    }
    XCTAssertFalse(chips.isEmpty, "The strip offered no candidate to measure.")
    for chip in chips {
      let label = try XCTUnwrap(chip.titleLabel)
      XCTAssertEqual(label.numberOfLines, 1, "A candidate chip may take more than one line.")
      XCTAssertLessThan(
        label.bounds.height, label.font.lineHeight * 1.5,
        "Candidate \(chip.accessibilityIdentifier ?? "?") wrapped instead of keeping its width.")
    }
  }

  func testCandidateChipsReuseTheirSlotsBetweenKeystrokes() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390,
                                   height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)

    for digit in "64426" {
      try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
    }
    let first = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
    XCTAssertFalse(first.isEmpty)

    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    let second = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
    XCTAssertFalse(second.isEmpty)
    for (before, after) in zip(first, second) {
      XCTAssertTrue(before === after)
    }
  }

  func testKeyPositionsStayFixedWhileComposingAndClearing() throws {
    let previous = InputSchemePreference.scheme
    InputSchemePreference.scheme = .nineKey
    defer { InputSchemePreference.scheme = previous }
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
      controller.view.layoutIfNeeded()
      let keys = try (1...9).map { try button("nineKey\($0)", in: controller) }
      let frames = keys.map { $0.convert($0.bounds, to: controller.view) }
      let delete = try button("nineKeyDelete", in: controller)
      let deleteFrame = delete.convert(delete.bounds, to: controller.view)
      XCTAssertGreaterThan(deleteFrame.minX, frames[2].maxX)
      XCTAssertEqual(deleteFrame.minY, frames[2].minY, accuracy: 0.5)
      XCTAssertGreaterThanOrEqual(frames[0].height, KeyboardHeightPercent.portraitKeyHeight(tablet: false) - 0.5)
      let sidebar = try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityIdentifier == "nineKeySidebar"
      })
      XCTAssertLessThan(sidebar.convert(sidebar.bounds, to: controller.view).maxX, frames[0].minX)

      for phase in ["idle", "composing", "cleared"] {
        if phase == "composing" {
          for digit in "64426" {
            try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
          }
          XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
          // The hostless XCTest runner does not route target/action through UIApplication.
          // Invoke the registered release action to exercise the same callback as a key tap.
          let releaseAction = try XCTUnwrap(delete.actions(forTarget: controller, forControlEvent: .touchUpInside)?.first)
          controller.perform(NSSelectorFromString(releaseAction))
          try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
          XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
        } else if phase == "cleared" {
          try clearComposition(in: controller)
          XCTAssertTrue(descendants(controller.view).allSatisfy {
            $0.accessibilityIdentifier?.hasPrefix("candidate-") != true || $0.isHidden
          })
        }
        controller.view.layoutIfNeeded()
        for (key, expected) in zip(keys, frames) {
          XCTAssertEqual(key.convert(key.bounds, to: controller.view), expected,
                         "Key moved during \(phase) at width \(width)")
        }
        let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
          controller.view.layer.render(in: context.cgContext)
        }
        let attachment = XCTAttachment(image: image)
        attachment.name = "Nine-key \(Int(width))pt \(phase)"
        attachment.lifetime = .keepAlways
        add(attachment)
      }
    }
  }

  /// 手机 26 键底行，按设计稿相对空格键的 flex 权重排列：123 1.25 | ， 1 | space 4 | 。 1 | 中 1.05 | return 1.9，中/英 紧挨回车左边，其中 ，。 经标点路径输入逗号和句号。
  func testPhoneBottomRowFollowsTheDesignsWeights() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let row = try XCTUnwrap(try button("spaceKey", in: controller).superview as? UIStackView)
    let visible = row.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier)
    let globe = controller.needsInputModeSwitchKey ? ["inputModeSwitchButton"] : []
    XCTAssertEqual(visible, ["layoutToggleButton"] + globe
                   + ["quickPunctuationKey", "spaceKey", "bottomPeriodKey", "bottomLanguageKey", "returnKey"])
    let space = try button("spaceKey", in: controller).bounds.width
    let unit = space / KeyboardViewController.phoneSpaceWeight
    XCTAssertEqual(try button("layoutToggleButton", in: controller).bounds.width, unit * 1.25, accuracy: 0.5)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).bounds.width, unit * 1.05, accuracy: 0.5)
    XCTAssertEqual(try button("quickPunctuationKey", in: controller).bounds.width, unit, accuracy: 0.5)
    XCTAssertEqual(try button("bottomPeriodKey", in: controller).bounds.width, unit, accuracy: 0.5)
    XCTAssertEqual(try button("returnKey", in: controller).bounds.width, unit * 1.9, accuracy: 0.5)
    XCTAssertEqual(try button("quickPunctuationKey", in: controller).configuration?.title, "，")
    XCTAssertEqual(try button("bottomPeriodKey", in: controller).configuration?.title, "。")
    // 123 和 中 用 15pt medium 字重。
    for id in ["layoutToggleButton", "bottomLanguageKey"] {
      let transformer = try XCTUnwrap(try button(id, in: controller).configuration?.titleTextAttributesTransformer, id)
      XCTAssertEqual(transformer.callAsFunction(AttributeContainer()).uiKit.font?.pointSize, 15, id)
    }
    // ， 长按仍弹出快捷符号。
    XCTAssertEqual(try button("quickPunctuationKey", in: controller).menu?.children.count, 7)

    // 英文下输出 ASCII 符号，键面也这样显示。
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("quickPunctuationKey", in: controller).configuration?.title, ",")
    XCTAssertEqual(try button("bottomPeriodKey", in: controller).configuration?.title, ".")

    // 123 层自带底行：ABC | emoji | space | return，⌫ 上移到该层第三行。
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(try button("quickPunctuationKey", in: controller).isHidden)
    XCTAssertTrue(try button("bottomPeriodKey", in: controller).isHidden)
    XCTAssertTrue(try button("bottomLanguageKey", in: controller).isHidden)
    XCTAssertTrue(try button("symbolDeleteKey", in: controller).isHidden)
    XCTAssertFalse(try button("symbolLayerDeleteKey", in: controller).isHidden)
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "ABC")
  }

  /// 九键保留自己的底行，即设计稿的 123 1.25 | space 4.2 | 0 1.05 | 中 1.05 | return 1.6，中/英 紧挨回车左边，没有 符、， 或 。 键。
  func testNineKeyKeepsItsOwnBottomRow() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let row = try XCTUnwrap(try button("spaceKey", in: controller).superview as? UIStackView)
    let ids = row.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier)
    let globe = controller.needsInputModeSwitchKey ? ["inputModeSwitchButton"] : []
    XCTAssertEqual(ids, ["layoutToggleButton"] + globe + ["spaceKey", "nineKeyZero", "bottomLanguageKey", "returnKey"])
    let unit = try button("spaceKey", in: controller).bounds.width / KeyboardViewController.nineKeySpaceWeight
    XCTAssertEqual(try button("layoutToggleButton", in: controller).bounds.width, unit * 1.25, accuracy: 0.5)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).bounds.width, unit * 1.05, accuracy: 0.5)
    XCTAssertEqual(try button("nineKeyZero", in: controller).bounds.width, unit * 1.05, accuracy: 0.5)
    XCTAssertEqual(try button("returnKey", in: controller).bounds.width, unit * 1.6, accuracy: 0.5)
    // 数字层保持同一底行；网格提供 1-9，这个键提供 0。
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(row.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier), ids)
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "九键")
  }

  /// 回车键始终是强调色按键：普通输入框上显示回车图标，输入框指定了动作时显示该动作的词，组字未结束时显示 确认。
  func testReturnIsTheAccentKeyWithTheGlyphAtRest() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let skin = KeyboardTheme.current
    let traits = controller.traitCollection
    let enter = try button("returnKey", in: controller)
    func assertAccent(_ phase: String) {
      guard skin.design == nil else { return }
      XCTAssertEqual(enter.configuration?.background.backgroundColor?.resolvedColor(with: traits),
                     skin.actionBackground.resolvedColor(with: traits), phase)
      XCTAssertEqual(enter.configuration?.baseForegroundColor?.resolvedColor(with: traits),
                     skin.actionForeground.resolvedColor(with: traits), phase)
    }
    XCTAssertNil(enter.configuration?.title)
    XCTAssertNotNil(enter.configuration?.image)
    XCTAssertEqual(enter.accessibilityLabel, "换行")
    assertAccent("idle")
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
      .sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(enter.configuration?.title, "确认")
    XCTAssertNil(enter.configuration?.image)
    let font = try XCTUnwrap(enter.configuration?.titleTextAttributesTransformer).callAsFunction(AttributeContainer()).uiKit.font
    XCTAssertEqual(font?.pointSize, 15)
    assertAccent("composing")
  }

  /// 空格键带麦克风图标和方案简称，英文下为 `space`，符号层上为 空格；VoiceOver 仍读作 空格。
  func testSpaceFaceNamesTheScheme() throws {
    XCTAssertEqual(KeyboardViewController.spaceKeyFace(scheme: .quanpin, chinese: true, symbols: false, composing: false),
                   .init(label: "全拼", mic: true))
    XCTAssertEqual(KeyboardViewController.spaceKeyFace(scheme: .shuangpin, chinese: true, symbols: false, composing: false).label,
                   ChineseInputScheme.shuangpin.shortLabel)
    XCTAssertEqual(KeyboardViewController.spaceKeyFace(scheme: .quanpin, chinese: false, symbols: false, composing: false),
                   .init(label: "space", mic: true))
    XCTAssertEqual(KeyboardViewController.spaceKeyFace(scheme: .quanpin, chinese: true, symbols: true, composing: false),
                   .init(label: "空格", mic: true))
    // 手写板的空格键同样显示方案名，与 Android 的 SpaceKeyFace.schemeLabel 给 HANDWRITING 的 手写 一致。
    XCTAssertEqual(KeyboardViewController.spaceKeyFace(scheme: .handwriting, chinese: true, symbols: false, composing: false),
                   .init(label: "手写", mic: true))
    XCTAssertEqual(KeyboardViewController.spaceKeyFace(scheme: .japanese, chinese: true, symbols: false, composing: true),
                   .init(label: "変換", mic: false))

    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let space = try button("spaceKey", in: controller)
    XCTAssertEqual(space.configuration?.title, "全拼")
    XCTAssertNotNil(space.configuration?.image)
    XCTAssertEqual(space.accessibilityLabel, "空格")
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(space.configuration?.title, "space")
    XCTAssertEqual(space.accessibilityLabel, "空格")
  }

  /// 第一个标签放在按键色的底块上，其余直接放在键盘上；按住时标签变淡。细分隔线后的展开箭头在键区切换完整列表，列表打开时箭头翻转。
  func testCandidateChipsAndTheExpandChevron() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let expand = try button("expandCandidates", in: controller)
    XCTAssertTrue(expand.isHidden, "nothing to expand while idle")
    for letter in ["N", "I"] {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(letter)" } as? UIButton)
        .sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    let first = try XCTUnwrap(try button("candidate-1", in: controller) as? KeyboardKeyButton)
    let second = try XCTUnwrap(try button("candidate-2", in: controller) as? KeyboardKeyButton)
    XCTAssertEqual(first.pressFeedback, .opacity(0.6))
    XCTAssertEqual(first.configuration?.contentInsets.leading, 11)
    XCTAssertEqual(first.configuration?.background.cornerRadius, 9)
    if CandidatePalette.active(in: MetasequoiaInputSessionBridge.loadSharedPreferences(), systemDark: false) == nil {
      XCTAssertGreaterThan(first.configuration?.background.backgroundColor?.cgColor.alpha ?? 0, 0, "the leading chip is filled")
      XCTAssertEqual(second.configuration?.background.backgroundColor?.cgColor.alpha ?? 0, 0, "the others sit on the keyboard")
    }
    XCTAssertGreaterThanOrEqual(first.bounds.width, KeyboardViewController.candidateChipMinimumSize.width - 0.5)

    XCTAssertFalse(expand.isHidden)
    let divider = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "expandCandidatesDivider" })
    XCTAssertFalse(divider.isHidden)
    XCTAssertEqual(divider.bounds.width, 1, accuracy: 0.1)
    XCTAssertEqual(divider.bounds.height, 22, accuracy: 0.1)
    XCTAssertEqual(expand.bounds.width, KeyboardViewController.expandButtonSide, accuracy: 0.5)
    XCTAssertEqual(expand.transform, .identity)
    expand.sendActions(for: .primaryActionTriggered)
    let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })
    controller.view.layoutIfNeeded()
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    XCTAssertEqual(panel.convert(panel.bounds, to: controller.view).minY,
                   strip.convert(strip.bounds, to: controller.view).maxY, accuracy: 0.5, "the grid opens under the strip")
    XCTAssertEqual(try button("spaceKey", in: controller).superview?.alpha, 0)
    XCTAssertEqual(atan2(expand.transform.b, expand.transform.a), .pi, accuracy: 0.01)
    XCTAssertTrue(try XCTUnwrap(try button("moreShortcut", in: controller) as? KeyboardBrandMarkButton).isActive)
    expand.sendActions(for: .primaryActionTriggered)
    XCTAssertNil(panel.superview)
    XCTAssertEqual(expand.transform, .identity)
    XCTAssertEqual(try button("spaceKey", in: controller).superview?.alpha, 1)
  }

  /// 常用语 在主线程之外读取短语并显示在键区，图标高亮；再点图标即关闭。
  func testPhrasesOpenInTheKeyAreaAndCloseFromTheirIcon() throws {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let phrases = try XCTUnwrap(try button("phrasesShortcut", in: controller) as? KeyboardToolbarButton)
    XCTAssertFalse(phrases.isHidden, "常用语 is on the toolbar by default")
    phrases.sendActions(for: .primaryActionTriggered)
    let shown = expectation(description: "phrases panel")
    func poll() {
      if descendants(controller.view).contains(where: { $0.accessibilityIdentifier == "keyboardPhrasesPanel" }) {
        shown.fulfill()
      } else {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { poll() }
      }
    }
    poll()
    wait(for: [shown], timeout: 10)
    let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardPhrasesPanel" })
    try assertInKeyArea(panel, of: controller)
    XCTAssertTrue(phrases.isActive)
    phrases.sendActions(for: .primaryActionTriggered)
    XCTAssertNil(panel.superview)
    XCTAssertFalse(phrases.isActive)
  }

  // MARK: - 九键展开面板

  /// 打开一个全拼九键键盘并打出 `digits`。
  private func nineKeyController(typing digits: String) throws -> KeyboardViewController {
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey]
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    for digit in digits {
      try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
    }
    return controller
  }

  private func candidatePanel(in controller: KeyboardViewController) -> UIView? {
    descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" }
  }

  private func panelCandidates(in controller: KeyboardViewController) -> [String] {
    descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("panelCandidate-") }
      .compactMap { chip in chip.configuration?.attributedTitle.map { String($0.characters) } }
      .map { $0.split(separator: "\n").first.map(String.init) ?? $0 }
  }

  /// 面板左栏里看得见的拼音：按钮本身和它的上层都没有隐藏（切到笔画时隐藏的是整个拼音列）。
  private func visibleSpellings(in controller: KeyboardViewController) -> [String] {
    descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { button in
        guard (button.accessibilityIdentifier ?? "").hasPrefix("candidatePanelSpelling_") else { return false }
        return sequence(first: button as UIView, next: \.superview).allSatisfy { !$0.isHidden }
      }
      .compactMap { $0.configuration?.title }
  }

  private func preedit(in controller: KeyboardViewController) throws -> String {
    try XCTUnwrap(button("preeditButton", in: controller).configuration?.title)
  }

  /// 全拼九键的展开面板是三栏、只盖住键区；在面板里选拼音、退格都不收起面板，而是按新的一代候选重建。
  func testNineKeyExpandedPanelStaysOpenWhileSpellingsAreChosen() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    let controller = try nineKeyController(typing: "6464224")
    // 读音行显示拼音读音，不是数字。
    let typed = try preedit(in: controller)
    XCTAssertFalse(typed.contains("6464"), typed)

    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(candidatePanel(in: controller))
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    XCTAssertGreaterThanOrEqual(panel.frame.minY, strip.convert(strip.bounds, to: controller.view).maxY,
                                "the panel covers the keys, not the candidate strip")
    for identifier in ["candidatePanelBack", "candidatePanelDelete", "candidatePanelClear",
                       "candidatePanelSingleToggle"] {
      XCTAssertNoThrow(try button(identifier, in: controller))
    }
    // 面板底色透明，被它盖住的键藏起来，候选栏不藏。
    let key = try button("nineKey6", in: controller)
    XCTAssertTrue(sequence(first: key as UIView, next: \.superview).contains { $0.alpha == 0 })
    XCTAssertFalse(sequence(first: strip, next: \.superview).contains { $0.alpha == 0 })
    XCTAssertEqual(try button("expandCandidates", in: controller).accessibilityLabel, "收起候选")
    XCTAssertFalse(panelCandidates(in: controller).isEmpty)
    let spellings = visibleSpellings(in: controller)
    XCTAssertTrue(spellings.contains("ning"), "\(spellings)")
    // 拼音之后是下一个数字键上的大写字母，最后是数字本身，无障碍名称分得清三者。
    let letter = try button("candidatePanelSpelling_M", in: controller)
    XCTAssertEqual(letter.accessibilityLabel, "选择字母 M")
    XCTAssertEqual(try button("candidatePanelSpelling_6", in: controller).accessibilityLabel, "输入数字 6")
    XCTAssertEqual(try button("candidatePanelSpelling_ning", in: controller).accessibilityLabel, "选择拼音 ning")
    XCTAssertEqual(try button("nineKeySpelling_M", in: controller).accessibilityLabel, "选择字母 M")

    try button("candidatePanelSpelling_ning", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(candidatePanel(in: controller) === panel, "choosing a spelling keeps the panel open")
    let locked = try preedit(in: controller)
    XCTAssertTrue(locked.hasPrefix("ning'"), locked)
    XCTAssertTrue(visibleSpellings(in: controller).contains("bai"), "\(visibleSpellings(in: controller))")

    try button("candidatePanelSpelling_bai", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(candidatePanel(in: controller) === panel)
    XCTAssertEqual(try preedit(in: controller), "ning'bai")
    // 数字都锁定之后，列表是最后一次锁定的选项，选另一项就换掉它。
    XCTAssertTrue(visibleSpellings(in: controller).contains("cai"), "\(visibleSpellings(in: controller))")

    // 全部锁定时退格先撤销最后一次锁定，面板不关。
    try button("candidatePanelDelete", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(candidatePanel(in: controller) === panel, "backspace keeps the panel open")
    let undone = try preedit(in: controller)
    XCTAssertTrue(undone.hasPrefix("ning'"), undone)
    XCTAssertTrue(visibleSpellings(in: controller).contains("bai"), "\(visibleSpellings(in: controller))")
    XCTAssertTrue(visibleSpellings(in: controller).contains("cai"), "\(visibleSpellings(in: controller))")

    // 再退格删一个数字，组字还在，面板也还在。
    try button("candidatePanelDelete", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(candidatePanel(in: controller) === panel)
    XCTAssertFalse(panelCandidates(in: controller).isEmpty)

    let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    }
    let attachment = XCTAttachment(image: image)
    attachment.name = "Nine-key expanded panel"
    attachment.lifetime = .keepAlways
    add(attachment)

    try button("candidatePanelBack", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertNil(candidatePanel(in: controller))
    XCTAssertFalse(try preedit(in: controller).isEmpty, "返回 only closes the panel")
    XCTAssertFalse(sequence(first: key as UIView, next: \.superview).contains { $0.alpha == 0 })
    XCTAssertEqual(try button("expandCandidates", in: controller).accessibilityLabel, "展开全部候选")
  }

  /// 「单字」只留单字，「笔画」按首字笔顺筛；收起面板时两项都清掉。
  func testNineKeyExpandedPanelFiltersBySingleCharacterAndStrokes() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    let controller = try nineKeyController(typing: "6464")
    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let all = panelCandidates(in: controller)
    XCTAssertTrue(all.contains { $0.count > 1 }, "\(all)")

    let single = try button("candidatePanelSingleToggle", in: controller)
    XCTAssertEqual(single.configuration?.title, "单字")
    single.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let singles = panelCandidates(in: controller)
    XCTAssertFalse(singles.isEmpty)
    XCTAssertTrue(singles.allSatisfy { $0.count == 1 }, "\(singles)")
    XCTAssertEqual(single.configuration?.title, "全部")

    let strokeToggle = try button("candidatePanelStrokeToggle", in: controller)
    try XCTSkipIf(strokeToggle.isHidden, "this bundle carries no stroke dictionary")
    strokeToggle.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(strokeToggle.configuration?.title, "拼音")
    XCTAssertTrue(visibleSpellings(in: controller).isEmpty, "the left column shows strokes now")
    let prefix = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "candidatePanelStrokePrefix"
    } as? UILabel)
    XCTAssertEqual(prefix.text, "笔画")
    // 宁（宀）起笔是点，拧（扌）起笔是横。
    XCTAssertTrue(singles.contains("宁") && singles.contains("拧"), "\(singles)")
    try button("candidatePanelStroke_n", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(prefix.text, "丶")
    let dotted = panelCandidates(in: controller)
    XCTAssertTrue(dotted.contains("宁"), "\(dotted)")
    XCTAssertFalse(dotted.contains("拧"), "\(dotted)")
    XCTAssertTrue(dotted.allSatisfy { $0.count == 1 }, "the single-character filter is kept")

    // 笔画模式下退格先删笔画，数字不动（读音跟着首选变，ning 和 ming 都有可能，所以看下面回到拼音后的拼音栏）。
    try button("candidatePanelDelete", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(prefix.text, "笔画")
    XCTAssertTrue(panelCandidates(in: controller).contains("拧"))

    // 回到拼音时笔画清掉，单字保留；收起面板时都清掉。
    try button("candidatePanelStroke_h", in: controller).sendActions(for: .primaryActionTriggered)
    strokeToggle.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(panelCandidates(in: controller).contains("宁"))
    XCTAssertTrue(visibleSpellings(in: controller).contains("ning"), "\(visibleSpellings(in: controller))")
    XCTAssertTrue(panelCandidates(in: controller).allSatisfy { $0.count == 1 })
    try button("candidatePanelBack", in: controller).sendActions(for: .primaryActionTriggered)
    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("candidatePanelSingleToggle", in: controller).configuration?.title, "单字")
    XCTAssertTrue(panelCandidates(in: controller).contains { $0.count > 1 })
  }

  /// 「重输」清掉组字，面板随组字一起收起。
  func testNineKeyExpandedPanelClearEndsTheComposition() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    let controller = try nineKeyController(typing: "64426")
    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertNotNil(candidatePanel(in: controller))
    try button("candidatePanelClear", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertNil(candidatePanel(in: controller))
    XCTAssertTrue(descendants(controller.view).allSatisfy {
      $0.accessibilityIdentifier?.hasPrefix("candidate-") != true || $0.isHidden
    })
  }

  /// 其他方案展开的仍是普通候选网格：同样装在键区，底栏是「返回」和 ⌫，没有九键的两栏。
  func testQuanpinKeepsTheFullCandidatePanel() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    for letter in ["Y", "I"] {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(letter)" } as? UIButton)
        .sendActions(for: .primaryActionTriggered)
    }
    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(candidatePanel(in: controller))
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    XCTAssertEqual(panel.convert(panel.bounds, to: controller.view).minY,
                   strip.convert(strip.bounds, to: controller.view).maxY, accuracy: 0.5, "the grid opens under the strip")
    XCTAssertNoThrow(try button("closeCandidatePanel", in: controller))
    XCTAssertNoThrow(try button("candidatePanelBackspace", in: controller))
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanelBack" })
  }

  // MARK: - 数字键盘顺序

  func testNumberKeypadOrderMapsGridPositions() {
    let phone = KeyboardLayoutPreference.NumberKeypadOrder.phone
    let calculator = KeyboardLayoutPreference.NumberKeypadOrder.calculator
    XCTAssertEqual((0..<3).flatMap { row in (0..<3).map { phone.digit(row: row, column: $0) } },
                   [1, 2, 3, 4, 5, 6, 7, 8, 9])
    XCTAssertEqual((0..<3).flatMap { row in (0..<3).map { calculator.digit(row: row, column: $0) } },
                   [7, 8, 9, 4, 5, 6, 1, 2, 3])
    XCTAssertEqual(KeyboardLayoutPreference.NumberKeypadOrder.shared(in: nil), .phone)
    XCTAssertEqual(KeyboardLayoutPreference.NumberKeypadOrder.shared(in: ["touch_number_keypad_order": "abacus"]), .phone)
    XCTAssertEqual(KeyboardLayoutPreference.NumberKeypadOrder.shared(in: ["touch_number_keypad_order": "calculator"]),
                   .calculator)
  }

  /// 计算器顺序只改数字层：7 8 9 在上、1 2 3 在下；字母层仍是 1-2-3。
  func testCalculatorOrderReordersOnlyTheNineKeyDigitLayer() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .nineKey
    KeyboardLayoutPreference.numberKeypadOrder = .calculator
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)

    XCTAssertEqual(try button("nineKey1", in: controller).configuration?.title, KeyboardViewController.nineKeySymbolsFace)
    XCTAssertEqual(try button("nineKey7", in: controller).configuration?.title, "PQRS")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let faces = try (1...9).map { try XCTUnwrap(button("nineKey\($0)", in: controller).configuration?.title) }
    XCTAssertEqual(faces, ["7", "8", "9", "4", "5", "6", "1", "2", "3"])
    XCTAssertEqual(try button("nineKey1", in: controller).accessibilityLabel, "数字 7")
    XCTAssertEqual(try button("nineKey9", in: controller).accessibilityLabel, "数字 3")
    let top = try button("nineKey1", in: controller)
    let bottom = try button("nineKey7", in: controller)
    XCTAssertLessThan(top.convert(top.bounds, to: controller.view).minY,
                      bottom.convert(bottom.bounds, to: controller.view).minY)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("nineKey7", in: controller).configuration?.title, "PQRS")
    XCTAssertEqual(try button("nineKey7", in: controller).accessibilityLabel, "7 PQRS")
  }

  // MARK: - 26 键数字键盘

  func testTwentySixKeyNumberLayoutReadsTheDocumentAndOnlyAppliesToPhoneLetterKeys() {
    typealias Layout = KeyboardLayoutPreference.TwentySixKeyNumberLayout
    XCTAssertEqual(Layout.shared(in: nil), .row)
    XCTAssertEqual(Layout.shared(in: ["touch_twenty_six_key_number_layout": "abacus"]), .row)
    XCTAssertEqual(Layout.shared(in: ["touch_twenty_six_key_number_layout": "nine_key"]), .nineKey)
    XCTAssertEqual(Layout.allCases.map(\.title), ["一行", "九宫格"])
    XCTAssertTrue(KeyboardViewController.opensNineKeyDigitPad(layout: .nineKey, formFactor: .phone, letterKeys: true))
    XCTAssertFalse(KeyboardViewController.opensNineKeyDigitPad(layout: .row, formFactor: .phone, letterKeys: true))
    // iPad 全尺寸键盘保留一行的 123 页；九键、笔画、手写、假名和大千的字母层不是 26 键字母。
    XCTAssertFalse(KeyboardViewController.opensNineKeyDigitPad(layout: .nineKey, formFactor: .tablet, letterKeys: true))
    XCTAssertFalse(KeyboardViewController.opensNineKeyDigitPad(layout: .nineKey, formFactor: .phone, letterKeys: false))
  }

  /// 选了九宫格后，全拼 26 键的 123 换成九键的数字层：标点栏、按数字键盘顺序排的 3×3、⌫ . ！ 和底行的 0；返回键回到 26 键字母，组字不受影响。
  func testNineKeyNumberLayoutTurnsTheTwentySixKey123IntoTheDigitGrid() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    KeyboardLayoutPreference.twentySixKeyNumberLayout = .nineKey
    KeyboardLayoutPreference.numberKeypadOrder = .calculator
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    func shown(_ view: UIView) -> Bool { sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden } }
    func view(_ id: String) throws -> UIView {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == id }, id)
    }
    let letterN = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    let letterI = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 I" } as? UIButton)
    XCTAssertFalse(shown(try view("nineKeySidebar")), "the letter layer is still the 26 keys")
    letterN.sendActions(for: .primaryActionTriggered)
    letterI.sendActions(for: .primaryActionTriggered)
    let composing = try button("preeditButton", in: controller).configuration?.title

    let toggle = try button("layoutToggleButton", in: controller)
    toggle.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, composing,
                   "tapping 123 keeps the composition, as the row page does")
    XCTAssertTrue(shown(try view("nineKeySidebar")))
    XCTAssertFalse(shown(try view("symbolLayerRow0")), "the row-of-digits page is not drawn")
    XCTAssertFalse(shown(letterN))
    let faces = try (1...9).map { try XCTUnwrap(button("nineKey\($0)", in: controller).configuration?.title) }
    XCTAssertEqual(faces, ["7", "8", "9", "4", "5", "6", "1", "2", "3"])
    XCTAssertEqual(try button("nineKey1", in: controller).accessibilityLabel, "数字 7")
    XCTAssertNil(try button("nineKey5", in: controller).accessibilityHint)
    XCTAssertEqual(try button("nineKey5", in: controller).gestureRecognizers?
      .compactMap { $0 as? UILongPressGestureRecognizer }.filter(\.isEnabled).count, 0,
                   "no pinyin letters to offer on a 26-key digit pad")
    XCTAssertEqual(try button("nineKeyMiddleKey", in: controller).configuration?.title, ".")
    XCTAssertTrue(shown(try button("nineKeyZero", in: controller)))
    XCTAssertTrue(shown(try button("nineKeyDelete", in: controller)))
    XCTAssertEqual(try button("nineKeyClosingMark", in: controller).configuration?.title, "！")
    XCTAssertEqual(toggle.configuration?.title, "拼音")
    XCTAssertEqual(toggle.accessibilityLabel, "切换到字母键盘")
    XCTAssertTrue(shown(try button("bottomLanguageKey", in: controller)))
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant,
                   KeyboardViewController.defaultKeyboardHeight, "the digit pad is as tall as the letters")

    // 返回键回到 26 键字母，不是九键的字母层。
    toggle.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(letterN))
    XCTAssertFalse(shown(try view("nineKeySidebar")))
    XCTAssertEqual(toggle.configuration?.title, "123")
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, composing)
    XCTAssertEqual(try button("nineKey5", in: controller).gestureRecognizers?
      .compactMap { $0 as? UILongPressGestureRecognizer }.filter(\.isEnabled).count, 1)

    // 英文的数字层与一行的 123 页一样用 ASCII 标点，返回键标为 ABC。
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    toggle.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(try view("nineKeySidebar")))
    XCTAssertNotNil(shownSymbolKey(",", in: controller))
    XCTAssertNotNil(shownSymbolKey("?", in: controller))
    XCTAssertNil(shownSymbolKey("，", in: controller))
    XCTAssertEqual(try button("nineKeyClosingMark", in: controller).configuration?.title, "!")
    XCTAssertEqual(toggle.configuration?.title, "ABC")
    // 在数字层上切回中文，标点跟着换回中文。
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotNil(shownSymbolKey("，", in: controller))
    XCTAssertEqual(try button("nineKeyClosingMark", in: controller).configuration?.title, "！")

    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    })
    attachment.name = "26-key nine-key digit pad"
    attachment.lifetime = .keepAlways
    add(attachment)
  }

  /// 默认的一行、九键方案和 iPad 全尺寸键盘都不受这个设置影响。
  func testNineKeyNumberLayoutLeavesOtherKeyboardsAlone() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    func shown(_ view: UIView) -> Bool { sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden } }
    func layer(after scheme: ChineseInputScheme, layout: KeyboardLayoutPreference.TwentySixKeyNumberLayout,
               tablet: Bool = false) throws -> KeyboardViewController {
      InputSchemePreference.scheme = scheme
      KeyboardLayoutPreference.twentySixKeyNumberLayout = layout
      let controller = KeyboardViewController()
      if tablet {
        controller.traitOverrides.userInterfaceIdiom = .pad
        controller.traitOverrides.horizontalSizeClass = .regular
      }
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: tablet ? 834 : 390, height: 320)
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      return controller
    }
    func symbolRow(_ controller: KeyboardViewController) throws -> UIView {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "symbolLayerRow0" })
    }

    XCTAssertTrue(shown(try symbolRow(try layer(after: .quanpin, layout: .row))), "row stays the default")
    XCTAssertTrue(shown(try symbolRow(try layer(after: .quanpin, layout: .nineKey, tablet: true))),
                  "the full-size iPad keyboard keeps its row page")
    XCTAssertTrue(shown(try symbolRow(try layer(after: .handwriting, layout: .nineKey))))
    let nineKey = try layer(after: .nineKey, layout: .nineKey)
    XCTAssertEqual(try button("layoutToggleButton", in: nineKey).configuration?.title, "九键")
    XCTAssertEqual(try button("nineKey2", in: nineKey).configuration?.title, "2")
  }

}

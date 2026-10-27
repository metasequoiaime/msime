import XCTest
import UIKit

/// 「按键布局」：三类键面的底行样式和顶部数字行。固定共享偏好的读法、底行表里每种样式的键序和份额、真键盘按表排出的底行、数字行的门槛与高度、样式在键盘开着时当场生效，以及统计热力图跟着样式画。三端同一套排法，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
@MainActor
final class KeyLayoutStyleTests: XCTestCase {
  private typealias Slot = TouchBottomRowLayout.Slot
  private static let styleKeys = [
    TouchKeyLayoutStyle.twentySixKeyDocumentKey, TouchKeyLayoutStyle.fourteenKeyDocumentKey,
    TouchKeyLayoutStyle.nineKeyDocumentKey, TouchKeyLayoutStyle.numberRowDocumentKey,
  ]

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    // 用例改的是模拟器共用的那份文档和 App Group，结束后四项、方案和中/英键设置都放回原样。
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    let saved = Self.styleKeys.map { document?[$0] }
    let mirror = TouchKeyLayoutStyle.mirrored
    let scheme = InputSchemePreference.scheme
    let hidesLanguage = KeyboardLayoutPreference.hideLanguageKey
    let toolbarHidden = TouchToolbarLocalPreference.hidden
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { document in
        for (key, value) in zip(Self.styleKeys, saved) { document[key] = value }
      }
      TouchKeyLayoutStyle.mirrored = mirror
      InputSchemePreference.scheme = scheme
      KeyboardLayoutPreference.hideLanguageKey = hidesLanguage
      TouchToolbarLocalPreference.hidden = toolbarHidden
    }
    KeyboardLayoutPreference.hideLanguageKey = false
    TouchToolbarLocalPreference.hidden = false
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
  }

  // MARK: - 偏好

  /// 缺键、不认识的写法（较新版本加的样式）和不是字符串的值都按标准读；数字行不是布尔值时按关。
  func testDocumentParsingFallsBackToStandard() {
    XCTAssertEqual(TouchKeyLayoutStyle(in: nil), TouchKeyLayoutStyle())
    XCTAssertEqual(TouchKeyLayoutStyle(in: [:]), TouchKeyLayoutStyle())
    let unknown: [String: Any] = [
      "touch_twenty_six_key_style": "split_space", "touch_fourteen_key_style": 3,
      "touch_nine_key_style": ["emoji"], "touch_number_row": "true",
    ]
    XCTAssertEqual(TouchKeyLayoutStyle(in: unknown), TouchKeyLayoutStyle())
    let known: [String: Any] = [
      "touch_twenty_six_key_style": "comma_period", "touch_fourteen_key_style": "long_space",
      "touch_nine_key_style": "emoji", "touch_number_row": true,
    ]
    XCTAssertEqual(TouchKeyLayoutStyle(in: known),
                   TouchKeyLayoutStyle(twentySixKey: .commaPeriod, fourteenKey: .longSpace, nineKey: .emoji, numberRow: true))
    // 线上写法与 client-core 的 `as_str` 一致。
    XCTAssertEqual(TouchTwentySixKeyStyle.allCases.map(\.rawValue), ["standard", "comma_period", "long_space", "emoji"])
    XCTAssertEqual(TouchFourteenKeyStyle.allCases.map(\.rawValue), ["standard", "long_space", "emoji"])
    XCTAssertEqual(TouchNineKeyStyle.allCases.map(\.rawValue), ["standard", "comma_period", "emoji"])
    // 设置页上的名字。
    XCTAssertEqual(TouchTwentySixKeyStyle.allCases.map(\.title), ["标准", "逗号句号在空格两侧", "长空格", "表情键"])
    XCTAssertEqual(TouchFourteenKeyStyle.allCases.map(\.title), ["标准", "长空格", "表情键"])
    XCTAssertEqual(TouchNineKeyStyle.allCases.map(\.title), ["标准", "底栏带标点", "表情键"])
  }

  /// 保存只写刚改的那一项，文档接受了才更新镜像；标准样式和关着的数字行不留在文档里。iOS 以前没有对应的本机设置，没有旧值要迁移：没写过的文档读出来就是现在的键盘。
  func testSavingWritesOneFieldAndTheMirror() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(TouchKeyLayoutStyle(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)),
                   TouchKeyLayoutStyle(), "a document written before the styles existed reads as today's keyboard")
    XCTAssertTrue(TouchKeyLayoutStyle.save(twentySixKey: .longSpace, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(nineKey: .commaPeriod, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(numberRow: true, stateRoot: state))
    var document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_twenty_six_key_style"] as? String, "long_space")
    XCTAssertEqual(document["touch_nine_key_style"] as? String, "comma_period")
    XCTAssertEqual(document["touch_number_row"] as? Bool, true)
    XCTAssertNil(document["touch_fourteen_key_style"], "an untouched style stays out of the document")
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored,
                   TouchKeyLayoutStyle(twentySixKey: .longSpace, nineKey: .commaPeriod, numberRow: true))
    XCTAssertTrue(TouchKeyLayoutStyle.save(fourteenKey: .emoji, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(twentySixKey: .standard, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(numberRow: false, stateRoot: state))
    document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertNil(document["touch_twenty_six_key_style"], "the default is not written")
    XCTAssertNil(document["touch_number_row"])
    XCTAssertEqual(TouchKeyLayoutStyle(in: document), TouchKeyLayoutStyle(fourteenKey: .emoji, nineKey: .commaPeriod))
    // 镜像只认文档：读不到文档时不动，文档里是什么就抄成什么。
    TouchKeyLayoutStyle.mirror(nil)
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored, TouchKeyLayoutStyle(fourteenKey: .emoji, nineKey: .commaPeriod))
    TouchKeyLayoutStyle.mirror(["touch_number_row": true])
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored, TouchKeyLayoutStyle(numberRow: true))
  }

  // MARK: - 底行表

  /// 每种样式的键序和份额，三端逐项一致；样式加的键从空格里让出份额，整行总份额不变。
  func testBottomRowTablesPinEveryStyle() {
    func row(_ keys: [TouchBottomRowLayout.Key]) -> [String] {
      keys.map { "\($0.slot.rawValue) \(String(format: "%.2f", Double($0.weight)))" }
    }
    XCTAssertEqual(row(TouchBottomRowLayout.twentySixKey(.standard, globe: false)),
                   ["layer 1.25", "symbols 1.05", "space 4.00", "comma 1.00", "language 1.05", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.twentySixKey(.commaPeriod, globe: false)),
                   ["layer 1.25", "symbols 1.05", "comma 1.00", "space 3.00", "period 1.00", "language 1.05", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.twentySixKey(.longSpace, globe: false)),
                   ["layer 1.25", "comma 1.00", "space 5.05", "language 1.05", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.twentySixKey(.emoji, globe: false)),
                   ["layer 1.25", "emoji 1.05", "space 4.00", "comma 1.00", "language 1.05", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.letterGrid(.standard, globe: false)),
                   ["layer 1.25", "comma 1.00", "space 4.00", "period 1.00", "language 1.05", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.letterGrid(.longSpace, globe: false)),
                   ["layer 1.25", "comma 1.00", "space 5.00", "language 1.05", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.letterGrid(.emoji, globe: false)),
                   ["layer 1.25", "emoji 1.00", "space 4.00", "comma 1.00", "language 1.05", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKey(.standard, globe: false)),
                   ["symbols 1.05", "layer 1.25", "space 4.20", "language 1.05", "returnKey 1.60"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKey(.commaPeriod, globe: false)),
                   ["symbols 1.05", "layer 1.25", "comma 1.00", "space 2.20", "period 1.00", "language 1.05", "returnKey 1.60"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKey(.emoji, globe: false)),
                   ["symbols 1.05", "layer 1.25", "emoji 1.05", "space 3.15", "language 1.05", "returnKey 1.60"])
    // 地球键跟在 123 后面占 1 份；九键双拼的 0 跟在空格后面；隐藏中/英时它的份额并进空格。
    XCTAssertEqual(row(TouchBottomRowLayout.twentySixKey(.longSpace, globe: true, language: false)),
                   ["layer 1.25", "globe 1.00", "comma 1.00", "space 6.10", "returnKey 1.90"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKey(.commaPeriod, globe: true, zero: true)),
                   ["symbols 1.05", "layer 1.25", "globe 1.00", "comma 1.00", "space 2.20", "period 1.00", "zero 1.05",
                    "language 1.05", "returnKey 1.60"])
    func total(_ keys: [TouchBottomRowLayout.Key]) -> Double { keys.reduce(0) { $0 + Double($1.weight) } }
    for style in TouchTwentySixKeyStyle.allCases {
      XCTAssertEqual(total(TouchBottomRowLayout.twentySixKey(style, globe: false)),
                     total(TouchBottomRowLayout.twentySixKey(.standard, globe: false)), accuracy: 1e-9, style.rawValue)
    }
    for style in TouchFourteenKeyStyle.allCases {
      XCTAssertEqual(total(TouchBottomRowLayout.letterGrid(style, globe: false)),
                     total(TouchBottomRowLayout.letterGrid(.standard, globe: false)), accuracy: 1e-9, style.rawValue)
    }
    for style in TouchNineKeyStyle.allCases {
      XCTAssertEqual(total(TouchBottomRowLayout.nineKey(style, globe: false)),
                     total(TouchBottomRowLayout.nineKey(.standard, globe: false)), accuracy: 1e-9, style.rawValue)
    }
  }

  /// 手机的数字行是整整一排：五排键加四个排间距；26 键收紧的键帽省下的高度按五排平分到上下留白，字母键仍是 44pt。iPad 的第五排仍只看「数字行与 Tab 键」。
  func testPhoneNumberRowAddsAFullRow() {
    let phone = KeyboardFormFactor.phone
    XCTAssertEqual(phone.keyRows(numberRow: false, phoneNumberRow: true), 5)
    XCTAssertEqual(phone.keyRows(numberRow: true), 4)
    XCTAssertEqual(KeyboardFormFactor.tablet.keyRows(numberRow: false, phoneNumberRow: true), 4)
    // 2 + 50 + 4 + 5 × 52 + 4 × 7 + 4。
    XCTAssertEqual(phone.keyboardHeight(topRow: 50, rowSpacing: 7, landscape: false, handwriting: false, phoneNumberRow: true), 348)
    XCTAssertEqual(phone.keyboardHeight(topRow: 50, rowSpacing: 11, landscape: false, handwriting: false, letterKeys: true,
                                        phoneNumberRow: true), 348)
    XCTAssertEqual(KeyboardFormFactor.phoneLetterVerticalPadding(rows: 4), KeyboardFormFactor.phoneLetterVerticalPadding)
    XCTAssertEqual(KeyboardFormFactor.phoneLetterVerticalPadding(rows: 5), 12)
    XCTAssertEqual(KeyboardHeightPercent.portraitKeyBlockHeight(tablet: false, numberRow: false, rowSpacing: 7, phoneNumberRow: true),
                   5 * 52 + 4 * 7)
  }

  // MARK: - 键盘

  /// 中文 26 键的每种样式：真键盘底行显示的键、顺序和宽度都照表；隐藏中/英时它的份额给空格。英文 26 键不受影响。
  func testTwentySixKeyStylesDrawTheTable() throws {
    select(.quanpin)
    for style in TouchTwentySixKeyStyle.allCases {
      for hidesLanguage in [false, true] {
        KeyboardLayoutPreference.hideLanguageKey = hidesLanguage
        XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: style)))
        let controller = keyboard()
        let label = "\(style.rawValue) hidesLanguage=\(hidesLanguage)"
        try assertBottomRow(controller, matches: TouchBottomRowLayout.twentySixKey(
          style, globe: controller.needsInputModeSwitchKey, language: !hidesLanguage), label)
        if !hidesLanguage { attach(controller, "26 键 \(style.title)") }
        controller.viewDidDisappear(false)
      }
    }
    KeyboardLayoutPreference.hideLanguageKey = false
    // 英文 26 键照旧：123 | 符 | . | 空格 | , | 中/英 | 回车。
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace)))
    let controller = keyboard()
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                   [.layer] + (controller.needsInputModeSwitchKey ? [.globe] : []) + [.symbols, .period, .space, .comma, .language, .returnKey])
    controller.viewDidDisappear(false)
  }

  /// 26 键各样式里逗号和句号的手势：标准和表情键的逗号长按出句号；逗号句号在空格两侧时逗号长按打开常用标点、句号是独立的键；长空格时逗号下滑或长按都出句号。
  func testTwentySixKeyCommaGesturesFollowTheStyle() throws {
    select(.quanpin)
    for style in TouchTwentySixKeyStyle.allCases {
      XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: style)))
      let controller = keyboard()
      let comma = try XCTUnwrap(try button("quickPunctuationKey", in: controller) as? KeyboardKeyButton)
      switch style {
      case .commaPeriod:
        XCTAssertNil(comma.cornerHint, style.rawValue)
        XCTAssertNotNil(comma.menu, "the comma opens the quick punctuation menu")
        XCTAssertFalse(try button("bottomPeriodKey", in: controller).isHidden)
      case .longSpace:
        XCTAssertEqual(comma.cornerHint, "。")
        XCTAssertTrue(comma.swipesCornerHint, "a downward swipe types the full stop")
        XCTAssertEqual(comma.cornerHintSwipeDirection, .down)
      case .standard, .emoji:
        XCTAssertEqual(comma.cornerHint, "。", style.rawValue)
        XCTAssertFalse(comma.swipesCornerHint, style.rawValue)
        XCTAssertNil(comma.menu, style.rawValue)
      }
      controller.viewDidDisappear(false)
    }
  }

  /// 长空格与 Android 原来的长空格布局一致：符 挪到工具栏，数字入口是带空格的 123 层而不是数字九宫格，那一层原来的表情或符号入口换成逗号；工具栏显示方式为隐藏时这一行仍然显示，只有品牌、符号和收起。
  func testLongSpaceMovesSymbolsToTheToolbarAndKeepsASpacedNumberLayer() throws {
    select(.quanpin)
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace)))
    var controller = keyboard()
    let symbols = try button("symbolsShortcut", in: controller)
    XCTAssertTrue(shown(symbols), "the symbol entry moves to the toolbar")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("symbolLayerKey0_0", in: controller)), "123 opens the layer that has a space")
    XCTAssertFalse(shown(try button("nineKey1", in: controller)), "not the borrowed number pad")
    let layerComma = try XCTUnwrap(try button("layerCommaKey", in: controller) as? KeyboardKeyButton)
    XCTAssertTrue(shown(layerComma))
    XCTAssertEqual(layerComma.configuration?.title, "，")
    XCTAssertEqual(layerComma.cornerHint, "。")
    XCTAssertEqual(layerComma.cornerHintSwipeDirection, .down)
    XCTAssertFalse(shown(try button("layerEmojiKey", in: controller)))
    XCTAssertFalse(shown(try button("symbolPanelKey", in: controller)))
    let unit = try button("spaceKey", in: controller).bounds.width / SymbolLayerLayout.spaceWeight
    XCTAssertEqual(layerComma.bounds.width, unit * SymbolLayerLayout.panelWeight, accuracy: 0.5)
    attach(controller, "26 键 长空格 123 层")
    symbols.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardSymbolPanel" })
    controller.viewDidDisappear(false)

    TouchToolbarLocalPreference.hidden = true
    controller = keyboard()
    let bar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" } as? UIStackView)
    XCTAssertFalse(bar.isHidden, "a hidden toolbar still carries the moved symbol entry")
    XCTAssertEqual(bar.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier),
                   ["moreShortcut", "symbolsShortcut", "dismissShortcut"])
    controller.viewDidDisappear(false)

    // 别的样式和别的键面没有这个入口。
    TouchToolbarLocalPreference.hidden = false
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .emoji)))
    controller = keyboard()
    XCTAssertFalse(shown(try button("symbolsShortcut", in: controller)))
    controller.viewDidDisappear(false)
  }

  /// 14 键与 18 键共用一套样式；隐藏中/英时它的份额给空格。
  func testLetterGridStylesDrawTheTable() throws {
    for scheme in [ChineseInputScheme.fourteenKey, .eighteenKey] {
      select(scheme)
      for style in TouchFourteenKeyStyle.allCases {
        for hidesLanguage in [false, true] {
          KeyboardLayoutPreference.hideLanguageKey = hidesLanguage
          XCTAssertTrue(write(TouchKeyLayoutStyle(fourteenKey: style)))
          let controller = keyboard()
          try assertBottomRow(controller, matches: TouchBottomRowLayout.letterGrid(
            style, globe: controller.needsInputModeSwitchKey, language: !hidesLanguage),
                              "\(scheme.rawValue) \(style.rawValue) hidesLanguage=\(hidesLanguage)")
          if scheme == .fourteenKey && !hidesLanguage { attach(controller, "14 键 \(style.title)") }
          controller.viewDidDisappear(false)
        }
      }
    }
  }

  /// 九键各样式，笔画沿用九键外框也跟着九键的样式；九键双拼字母层的 0 跟在空格后面；隐藏中/英时它的份额给空格。数字层不变。
  func testNineKeyStylesDrawTheTable() throws {
    for scheme in [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke] {
      select(scheme)
      // 笔画只在测试宿主带了 msime-stroke.db 时才能选上（CI 不带）；没带时选择落到别的方案，跳过。
      if scheme == .stroke && InputSchemePreference.scheme != .stroke { continue }
      for style in TouchNineKeyStyle.allCases {
        for hidesLanguage in [false, true] {
          KeyboardLayoutPreference.hideLanguageKey = hidesLanguage
          XCTAssertTrue(write(TouchKeyLayoutStyle(nineKey: style)))
          let controller = keyboard()
          try assertBottomRow(controller, matches: TouchBottomRowLayout.nineKey(
            style, globe: controller.needsInputModeSwitchKey, zero: scheme == .nineKeyShuangpin, language: !hidesLanguage),
                              "\(scheme.rawValue) \(style.rawValue) hidesLanguage=\(hidesLanguage)")
          if scheme == .nineKey && !hidesLanguage { attach(controller, "9 键 \(style.title)") }
          if style == .commaPeriod {
            // ， 与手机底行一样跟随标点：中文标点时印 ，，长按仍是常用标点菜单。
            let comma = try button("quickPunctuationKey", in: controller)
            XCTAssertEqual(comma.configuration?.title, "，")
            XCTAssertNotNil(comma.menu)
          }
          // 数字层照旧：0 在中间，没有 ， 和 。。
          try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
          controller.view.layoutIfNeeded()
          XCTAssertFalse(shown(try button("quickPunctuationKey", in: controller)), "\(scheme.rawValue) \(style.rawValue)")
          XCTAssertFalse(shown(try button("bottomPeriodKey", in: controller)))
          XCTAssertFalse(shown(try button("layerEmojiKey", in: controller)))
          controller.viewDidDisappear(false)
        }
      }
    }
  }

  /// 顶部数字行：26 键、14 键和 18 键的字母层上方一排 1 到 0，键盘整整高一排；数字页和符号页用补充行占住这一排，切层时高度不变；九键不画；手机横屏不画，存着的值不变。
  func testNumberRowAddsOneRowAndKeepsTheHeightAcrossLayers() throws {
    XCTAssertTrue(write(TouchKeyLayoutStyle(numberRow: true)))
    let taller = KeyboardViewController.keyboardHeight(phoneNumberRow: true)
    XCTAssertEqual(taller - KeyboardViewController.defaultKeyboardHeight,
                   KeyboardHeightPercent.portraitKeyHeight(tablet: false) + CGFloat(KeyboardLayoutPreference.rowSpacing), accuracy: 0.01)
    for scheme in [ChineseInputScheme.quanpin, .fourteenKey, .eighteenKey] {
      select(scheme)
      // 14 键和 18 键的顶栏上面多一行读音（`separateReadingRow`），数字行照样只加一排。
      let taller = KeyboardViewController.keyboardHeight(separateReadingRow: scheme != .quanpin, phoneNumberRow: true)
      let controller = keyboard()
      let row = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "numberRow" } as? UIStackView)
      XCTAssertTrue(shown(row), scheme.rawValue)
      XCTAssertEqual(row.arrangedSubviews.filter { !$0.isHidden }.compactMap { ($0 as? UIButton)?.configuration?.title }.joined(),
                     "1234567890")
      XCTAssertEqual(height(of: controller), taller, accuracy: 0.5, scheme.rawValue)
      if scheme == .quanpin {
        // 字母键仍是 44pt：多出的留白按五排重算。
        let letter = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" })
        XCTAssertEqual(letter.bounds.height, 44, accuracy: 0.5)
        attach(controller, "26 键 数字行")
      }
      // 数字页（借用的九宫格）上补充行占住这一排。
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      XCTAssertFalse(shown(row))
      XCTAssertTrue(shown(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "symbolExtraRow" })))
      XCTAssertEqual(height(of: controller), taller, accuracy: 0.5, "\(scheme.rawValue) number layer")
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      XCTAssertTrue(shown(row))
      // 英文 26 键也有数字行，它的 123 层同样占住这一排。
      if scheme == .quanpin {
        try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
        try layoutAtRequestedHeight(controller)
        XCTAssertTrue(shown(row))
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        try layoutAtRequestedHeight(controller)
        XCTAssertEqual(height(of: controller), taller, accuracy: 0.5, "English 123 layer")
      }
      controller.viewDidDisappear(false)
    }
    // 九键不画。
    select(.nineKey)
    var controller = keyboard()
    XCTAssertFalse(shown(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "numberRow" })))
    XCTAssertEqual(height(of: controller), KeyboardViewController.defaultKeyboardHeight, accuracy: 0.5)
    controller.viewDidDisappear(false)
    // 手机横屏不画。
    select(.quanpin)
    controller = KeyboardViewController()
    controller.traitOverrides.verticalSizeClass = .compact
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 874, height: KeyboardViewController.keyboardHeight(landscape: true))
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "numberRow" })),
                   "phone landscape has no room for a fifth row")
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.numberRow, true, "the stored choice stays")
    controller.viewDidDisappear(false)
  }

  /// 数字行的键记成 Digit1…Digit0，热力图里画在字母上方。
  func testNumberRowKeysCountAsDigits() throws {
    XCTAssertTrue(write(TouchKeyLayoutStyle(numberRow: true)))
    select(.quanpin)
    let controller = keyboard()
    let row = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "numberRow" } as? UIStackView)
    try assertRow(row, draws: TypingKeyHeatmap.letterRows(for: TouchKeyLayoutStyle(numberRow: true))[0])
    XCTAssertEqual(TypingKeyHeatmap.letterRows(for: TouchKeyLayoutStyle(numberRow: true)).count, 5)
    XCTAssertEqual(TypingKeyHeatmap.middleLetterRow(for: TouchKeyLayoutStyle(numberRow: true)), 2)
    controller.viewDidDisappear(false)
  }

  /// 键盘开着时在设置里改样式，下次重载文档当场换上，正在组的字不受影响。
  func testStyleChangesApplyLiveWithoutDroppingTheComposition() async throws {
    select(.quanpin)
    let controller = keyboard()
    for letter in "ni" {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(letter.uppercased())" } as? UIButton)
        .sendActions(for: .primaryActionTriggered)
    }
    let preedit = try button("preeditButton", in: controller).accessibilityValue
    XCTAssertFalse((preedit ?? "").isEmpty, "typing ni opens a composition")
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .commaPeriod, numberRow: true)))
    controller.viewWillAppear(false)
    let period = try button("bottomPeriodKey", in: controller)
    let deadline = Date().addingTimeInterval(15)
    while !shown(period) && Date() < deadline {
      // 让出主线程，后台读回的文档在主队列上应用。
      try await Task.sleep(for: .milliseconds(50))
    }
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(period), "the new style is drawn without reopening the keyboard")
    let numberRow = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "numberRow" })
    XCTAssertTrue(shown(numberRow))
    try assertLetterKeysAre44(controller, "number row switched on live")
    XCTAssertEqual(try button("preeditButton", in: controller).accessibilityValue, preedit, "the composition survives")
    // 再当场关掉数字行：留白按四排重算，字母键仍是 44pt。
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .commaPeriod)))
    controller.viewWillAppear(false)
    let offDeadline = Date().addingTimeInterval(15)
    while shown(numberRow) && Date() < offDeadline {
      try await Task.sleep(for: .milliseconds(50))
    }
    try layoutAtRequestedHeight(controller)
    XCTAssertFalse(shown(numberRow), "the number row goes away without reopening the keyboard")
    try assertLetterKeysAre44(controller, "number row switched off live")
    XCTAssertEqual(try button("preeditButton", in: controller).accessibilityValue, preedit, "the composition survives")
    controller.viewDidDisappear(false)
  }

  /// 数字行在同一趟排版里出现时（从九键换到全拼、横屏转回竖屏），26 键的上下留白按这一趟的排数算，字母键仍是 44pt，键区面板的底边也停在同一处。
  func testLetterKeysStay44WhenTheNumberRowAppearsInOnePass() throws {
    XCTAssertTrue(write(TouchKeyLayoutStyle(numberRow: true)))
    let taller = KeyboardViewController.keyboardHeight(phoneNumberRow: true)
    // 九键没有数字行，换成全拼时数字行和 26 键在同一趟里出现。
    select(.nineKey)
    var controller = keyboard()
    var numberRow = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "numberRow" })
    XCTAssertFalse(shown(numberRow))
    select(.quanpin)
    controller.viewWillAppear(false)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(numberRow), "nine-key to quanpin")
    XCTAssertEqual(height(of: controller), taller, accuracy: 0.5)
    try assertLetterKeysAre44(controller, "nine-key to quanpin")
    controller.viewDidDisappear(false)

    // 横屏不画数字行，转回竖屏时它在同一趟里回来。
    controller = KeyboardViewController()
    controller.traitOverrides.verticalSizeClass = .compact
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 874, height: KeyboardViewController.keyboardHeight(landscape: true))
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    numberRow = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "numberRow" })
    XCTAssertFalse(shown(numberRow), "landscape")
    controller.traitOverrides.verticalSizeClass = .regular
    controller.updateTraitsIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 402, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(numberRow), "landscape to portrait")
    XCTAssertEqual(height(of: controller), taller, accuracy: 0.5)
    try assertLetterKeysAre44(controller, "landscape to portrait")
    // 键区面板（这里是「更多」）按此刻的留白停在按键底边，与新建的五排键盘停在同一处。
    let rotated = try morePanelGap(controller)
    controller.viewDidDisappear(false)
    controller = keyboard()
    XCTAssertEqual(rotated, try morePanelGap(controller), accuracy: 0.5, "the panel stops where the keys stop")
    controller.viewDidDisappear(false)
  }

  /// 打开「更多」面板，返回回车键底边与面板底边的距离。
  private func morePanelGap(_ controller: KeyboardViewController) throws -> CGFloat {
    let returnKey = try button("returnKey", in: controller)
    let keysBottom = returnKey.convert(returnKey.bounds, to: controller.view).maxY
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" })
    return panel.convert(panel.bounds, to: controller.view).maxY - keysBottom
  }

  // MARK: - 热力图

  /// 统计热力图的底行跟着用户的样式画，与真键盘一一对应；画在键盘上的键不再列进「其他键」。
  func testHeatmapsDrawTheUsersStyle() throws {
    select(.quanpin)
    for style in TouchTwentySixKeyStyle.allCases {
      XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: style)))
      let controller = keyboard()
      let bottom = try XCTUnwrap(try button("spaceKey", in: controller).superview as? UIStackView)
      try assertRow(bottom, draws: TypingKeyHeatmap.letterRows(for: TouchKeyLayoutStyle(twentySixKey: style)).last ?? [])
      controller.viewDidDisappear(false)
    }
    select(.nineKey)
    for style in TouchNineKeyStyle.allCases {
      XCTAssertTrue(write(TouchKeyLayoutStyle(nineKey: style)))
      let controller = keyboard()
      let bottom = try XCTUnwrap(try button("spaceKey", in: controller).superview as? UIStackView)
      try assertRow(bottom, draws: TypingKeyHeatmap.nineKeyBottomCells(for: style))
      controller.viewDidDisappear(false)
    }
    // 标准样式就是原来那张图。
    XCTAssertEqual(TypingKeyHeatmap.letterRows(for: TouchKeyLayoutStyle()), TypingKeyHeatmap.keyboardRows)
    XCTAssertEqual(TypingKeyHeatmap.nineKeyBottomCells(for: .standard), TypingKeyHeatmap.nineKeyBottomRow)
    let counts = TypingKeyHeatmap(counts: ["Period": 4, TypingKeyID.symbol: 3, TypingKeyID.emoji: 2, "Digit1": 1])
    XCTAssertEqual(counts.others(nineKey: false).map(\.id), ["Period", TypingKeyID.emoji, "Digit1"])
    XCTAssertEqual(counts.others(nineKey: false, style: TouchKeyLayoutStyle(twentySixKey: .commaPeriod)).map(\.id),
                   [TypingKeyID.emoji, "Digit1"])
    XCTAssertEqual(counts.others(nineKey: false, style: TouchKeyLayoutStyle(twentySixKey: .longSpace)).map(\.id),
                   ["Period", TypingKeyID.symbol, TypingKeyID.emoji, "Digit1"])
    XCTAssertEqual(counts.others(nineKey: false, style: TouchKeyLayoutStyle(twentySixKey: .emoji, numberRow: true)).map(\.id),
                   ["Period", TypingKeyID.symbol])
    XCTAssertEqual(counts.others(nineKey: true, style: TouchKeyLayoutStyle(nineKey: .emoji)).map(\.id),
                   ["Period", "Digit1"])
    let drawn = TouchTwentySixKeyStyle.allCases.flatMap { TypingKeyHeatmap.cells(nineKey: false, style: TouchKeyLayoutStyle(twentySixKey: $0, numberRow: true)) }
      + TouchNineKeyStyle.allCases.flatMap { TypingKeyHeatmap.cells(nineKey: true, style: TouchKeyLayoutStyle(nineKey: $0)) }
    XCTAssertEqual(drawn.map(\.id).filter { !TypingKeyID.known.contains($0) }, [], "only ids the store accepts")
  }

  // MARK: - 辅助

  private func write(_ style: TouchKeyLayoutStyle) -> Bool {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { document in
      document[TouchKeyLayoutStyle.twentySixKeyDocumentKey] = style.twentySixKey.rawValue
      document[TouchKeyLayoutStyle.fourteenKeyDocumentKey] = style.fourteenKey.rawValue
      document[TouchKeyLayoutStyle.nineKeyDocumentKey] = style.nineKey.rawValue
      document[TouchKeyLayoutStyle.numberRowDocumentKey] = style.numberRow
    }
    if written { TouchKeyLayoutStyle.mirrored = style }
    return written
  }

  /// 文档和 App Group 都选中 `scheme`：新建的键盘先按文档里的方案行事。
  private func select(_ scheme: ChineseInputScheme) {
    _ = InputSchemePreference.update { selection in
      selection.enabled = ChineseInputScheme.allCases
      selection.scheme = scheme
    }
    InputSchemePreference.scheme = scheme
  }

  /// iPhone 17 竖屏宽度的键盘，按它请求的高度排好。
  private func keyboard() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 402, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    try? layoutAtRequestedHeight(controller)
    return controller
  }

  /// 26 键手机竖屏的字母键高 44pt（键帽收紧后的高度），与有几排无关。
  private func assertLetterKeysAre44(_ controller: KeyboardViewController, _ label: String,
                                     file: StaticString = #filePath, line: UInt = #line) throws {
    let letter = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" }, file: file, line: line)
    XCTAssertEqual(letter.bounds.height, 44, accuracy: 0.5, label, file: file, line: line)
  }

  private func height(of controller: KeyboardViewController) -> CGFloat {
    controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant ?? 0
  }

  private func layoutAtRequestedHeight(_ controller: KeyboardViewController) throws {
    controller.view.frame.size.height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    controller.view.layoutIfNeeded()
  }

  private static let slotByIdentifier: [String: Slot] = [
    "symbolPanelKey": .symbols, "layoutToggleButton": .layer, "inputModeSwitchButton": .globe, "layerEmojiKey": .emoji,
    "quickPunctuationKey": .comma, "spaceKey": .space, "bottomPeriodKey": .period, "nineKeyZero": .zero,
    "bottomLanguageKey": .language, "returnKey": .returnKey,
  ]

  /// 底行里显示着的键和它们的宽度。
  private func visibleBottomRow(_ controller: KeyboardViewController) -> [(slot: Slot, width: CGFloat)] {
    guard let row = (try? button("spaceKey", in: controller))?.superview as? UIStackView else { return [] }
    return row.arrangedSubviews.filter { !$0.isHidden && $0.bounds.width > 0.5 }.compactMap { view in
      view.accessibilityIdentifier.flatMap { Self.slotByIdentifier[$0] }.map { ($0, view.bounds.width) }
    }
  }

  /// 底行显示的键依次是表里的键，每个键占全行键宽之和的比例等于它的份额占比。
  private func assertBottomRow(_ controller: KeyboardViewController, matches keys: [TouchBottomRowLayout.Key], _ label: String,
                               file: StaticString = #filePath, line: UInt = #line) throws {
    let row = visibleBottomRow(controller)
    XCTAssertEqual(row.map(\.slot), keys.map(\.slot), label, file: file, line: line)
    guard row.count == keys.count else { return }
    let widths = row.reduce(0) { $0 + $1.width }
    let weights = keys.reduce(0) { $0 + $1.weight }
    for (drawn, key) in zip(row, keys) {
      XCTAssertEqual(drawn.width / widths, key.weight / weights, accuracy: 0.01, "\(label) \(key.slot.rawValue)", file: file, line: line)
    }
  }

  /// 键盘按下各键时记的 id，按无障碍标识对应；数字行的键按键面。地球键热力图不画，比对时滤掉。
  private static let recordedID: [String: String] = [
    "symbolPanelKey": TypingKeyID.symbol, "layoutToggleButton": TypingKeyID.layer, "layerEmojiKey": TypingKeyID.emoji,
    "spaceKey": TypingKeyID.space, "bottomLanguageKey": TypingKeyID.language, "returnKey": TypingKeyID.enter,
    "quickPunctuationKey": TypingKeyID.quickPunctuation, "bottomPeriodKey": "Period", "nineKeyZero": "Nine0",
  ]

  private func assertRow(_ row: UIStackView, draws cells: [TypingKeyHeatmap.Cell],
                         file: StaticString = #filePath, line: UInt = #line) throws {
    let keys = row.arrangedSubviews.filter { !$0.isHidden && $0.accessibilityIdentifier != "inputModeSwitchButton" }
    let ids = keys.map { key -> String in
      if let id = key.accessibilityIdentifier.flatMap({ Self.recordedID[$0] }) { return id }
      return "Digit\((key as? UIButton)?.configuration?.title ?? "?")"
    }
    XCTAssertEqual(ids, cells.map(\.id), file: file, line: line)
    guard ids.count == cells.count else { return }
    let total = keys.reduce(0) { $0 + $1.bounds.width }
    let weights = cells.reduce(0) { $0 + $1.width }
    for (key, cell) in zip(keys, cells) {
      XCTAssertEqual(Double(key.bounds.width / total), cell.width / weights, accuracy: 0.01, cell.id, file: file, line: line)
    }
  }

  private func attach(_ controller: KeyboardViewController, _ name: String) {
    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    })
    attachment.name = name
    attachment.lifetime = .keepAlways
    add(attachment)
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func shown(_ view: UIView) -> Bool {
    sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden }
  }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }
}

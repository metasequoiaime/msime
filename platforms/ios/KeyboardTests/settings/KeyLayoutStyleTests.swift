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
    XCTAssertEqual(TouchKeyLayoutStyle(in: ["touch_nine_key_style": "tall_return"]), TouchKeyLayoutStyle(nineKey: .tallReturn))
    // 线上写法与 client-core 的 `as_str` 一致。
    XCTAssertEqual(TouchTwentySixKeyStyle.allCases.map(\.rawValue), ["standard", "comma_period", "long_space", "emoji"])
    XCTAssertEqual(TouchFourteenKeyStyle.allCases.map(\.rawValue), ["standard", "long_space", "emoji"])
    XCTAssertEqual(TouchNineKeyStyle.allCases.map(\.rawValue), ["standard", "comma_period", "emoji", "tall_return"])
    // 设置页上的名字。
    XCTAssertEqual(TouchTwentySixKeyStyle.allCases.map(\.title), ["标准", "逗号句号在空格两侧", "长空格", "表情键"])
    XCTAssertEqual(TouchFourteenKeyStyle.allCases.map(\.title), ["标准", "长空格", "表情键"])
    XCTAssertEqual(TouchNineKeyStyle.allCases.map(\.title), ["标准", "底栏带标点", "表情键", "右侧大回车"])
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
    // 右侧大回车：符 与回车各占一个侧列（整宽的 14%：不带地球键时整宽 9.15 份，带地球键时与标准九键底行一样是 10.15 份），中间几格合起来是九宫格宽，空格（数字层是 0）占剩下的；地球键和九键双拼的 0 都从空格里让出，隐藏中/英时它的份额给空格。
    XCTAssertEqual(TouchBottomRowLayout.nineKeyFrameWeight, 9.15, accuracy: 1e-9)
    XCTAssertEqual(TouchBottomRowLayout.nineKeyRowWeight(globe: false), 9.15, accuracy: 1e-9)
    XCTAssertEqual(TouchBottomRowLayout.nineKeyRowWeight(globe: true), 10.15, accuracy: 1e-9)
    XCTAssertEqual(TouchBottomRowLayout.nineKeySideWeight(globe: false), 9.15 * 0.14, accuracy: 1e-9)
    XCTAssertEqual(TouchBottomRowLayout.nineKeySideWeight(globe: true), 10.15 * 0.14, accuracy: 1e-9)
    XCTAssertEqual(row(TouchBottomRowLayout.nineKey(.tallReturn, globe: false)),
                   ["symbols 1.28", "layer 1.25", "space 4.29", "language 1.05", "returnKey 1.28"])
    XCTAssertEqual(TouchBottomRowLayout.nineKey(.tallReturn, globe: false), TouchBottomRowLayout.nineKeyTallReturn(globe: false))
    XCTAssertEqual(row(TouchBottomRowLayout.nineKeyTallReturn(globe: true, zero: true)),
                   ["symbols 1.42", "layer 1.25", "globe 1.00", "space 2.96", "zero 1.05", "language 1.05", "returnKey 1.42"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKeyTallReturn(globe: false, language: false)),
                   ["symbols 1.28", "layer 1.25", "space 5.34", "returnKey 1.28"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKeyTallReturn(globe: false, digits: true)),
                   ["symbols 1.28", "layer 1.25", "zero 3.24", "space 1.05", "language 1.05", "returnKey 1.28"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKeyTallReturn(globe: true, zero: true, language: false, digits: true)),
                   ["symbols 1.42", "layer 1.25", "globe 1.00", "zero 4.01", "space 1.05", "returnKey 1.42"])
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
      for globe in [false, true] {
        XCTAssertEqual(total(TouchBottomRowLayout.nineKey(style, globe: globe)),
                       total(TouchBottomRowLayout.nineKey(.standard, globe: globe)), accuracy: 1e-9, "\(style.rawValue) globe \(globe)")
      }
    }
  }

  /// 右侧大回车中列的 123、地球键、中/英在整行里的宽度与同一地球键状态下的标准九键底行（拼音九键和笔画的那一行，不带九键双拼另加的 0）相同，字母层和数字层都是，九键双拼字母层的 0 与数字层的空格是 1.05 份。带地球键时标准底行是 10.15 份，所以这些键的绝对宽度比不带地球键时窄，不能按 9.15 份算。
  func testTallReturnMiddleKeysKeepTheStandardNineKeyWidths() {
    func share(_ keys: [TouchBottomRowLayout.Key], _ slot: Slot) -> Double? {
      guard let key = keys.first(where: { $0.slot == slot }) else { return nil }
      return Double(key.weight) / keys.reduce(0) { $0 + Double($1.weight) }
    }
    for globe in [false, true] {
      for zero in [false, true] {
        for language in [false, true] {
          let standard = TouchBottomRowLayout.nineKey(.standard, globe: globe, language: language)
          for digits in [false, true] {
            let tall = TouchBottomRowLayout.nineKeyTallReturn(globe: globe, zero: zero, language: language, digits: digits)
            let label = "globe \(globe) zero \(zero) language \(language) digits \(digits)"
            for slot: Slot in [.layer, .globe, .language] {
              XCTAssertEqual(share(tall, slot) ?? -1, share(standard, slot) ?? -1, accuracy: 1e-9, "\(label) \(slot.rawValue)")
            }
            let small: Slot? = digits ? .space : zero ? .zero : nil
            if let small {
              XCTAssertEqual(share(tall, small) ?? 0, 1.05 / (globe ? 10.15 : 9.15), accuracy: 1e-9, "\(label) \(small.rawValue)")
            }
            // 符 与回车各是整宽的 14%。
            XCTAssertEqual(share(tall, .symbols) ?? 0, 0.14, accuracy: 1e-9, label)
            XCTAssertEqual(share(tall, .returnKey) ?? 0, 0.14, accuracy: 1e-9, label)
          }
        }
      }
    }
    // 带地球键时 123 是整宽的 1.25 / 10.15，不是 1.25 / 9.15。
    XCTAssertEqual(share(TouchBottomRowLayout.nineKeyTallReturn(globe: true), .layer) ?? 0, 1.25 / 10.15, accuracy: 1e-9)
    // 键盘扣键距时数的是拼音九键标准底行的键：符、123、空格、回车，加上显示着的地球键和中/英。
    XCTAssertEqual(TouchBottomRowLayout.nineKeyStandardKeyCount(globe: false, language: true), 5)
    XCTAssertEqual(TouchBottomRowLayout.nineKeyStandardKeyCount(globe: true, language: true), 6)
    XCTAssertEqual(TouchBottomRowLayout.nineKeyStandardKeyCount(globe: false, language: false), 4)
    XCTAssertEqual(TouchBottomRowLayout.nineKeyStandardKeyCount(globe: true, language: false), 5)
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
        XCTAssertTrue(comma.holdsCornerHint, "so does a long press")
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

  /// 九键各样式，笔画沿用九键外框也跟着九键的样式；九键双拼字母层的 0 跟在空格后面；隐藏中/英时它的份额给空格。数字层不变。右侧大回车换的是整个外框，见 `testTallReturnFrame`。
  func testNineKeyStylesDrawTheTable() throws {
    for scheme in [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke] {
      select(scheme)
      // 笔画只在测试宿主带了 msime-stroke.db 时才能选上（CI 不带）；没带时选择落到别的方案，跳过。
      if scheme == .stroke && InputSchemePreference.scheme != .stroke { continue }
      for style in TouchNineKeyStyle.allCases where style != .tallReturn {
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

  /// 右侧大回车的外框：符 与符号栏同列同宽，底行中间几格与九宫格（笔画是笔画键）左右对齐，123、中/英、地球键与 `standard`（拼音九键标准底行上量到的键宽，见 `standardNineKeyWidths`）逐个等宽，0（数字层是空格）与标准底行的 符 等宽，回车从右列第三格伸到底行；右列的 ！ 不画、不接触摸、不进无障碍。`digits` 为数字层：空格在 0 右边、与 中/英 同宽。
  private func assertTallReturnFrame(_ controller: KeyboardViewController, digits: Bool, standard: [String: CGFloat], _ label: String,
                                     file: StaticString = #filePath, line: UInt = #line) throws {
    func frame(_ view: UIView) -> CGRect { view.convert(view.bounds, to: controller.view) }
    let sidebar = try view("nineKeySidebar", in: controller)
    let delete = try button("nineKeyDelete", in: controller)
    let middle = try button("nineKeyMiddleKey", in: controller)
    let closing = try button("nineKeyClosingMark", in: controller)
    let tall = try button("tallReturnKey", in: controller)
    let slot = try view("tallReturnSlot", in: controller)
    let symbols = try button("symbolPanelKey", in: controller)
    let space = try button("spaceKey", in: controller)
    XCTAssertTrue(shown(tall), label, file: file, line: line)
    XCTAssertTrue(shown(slot), label, file: file, line: line)
    XCTAssertTrue(shown(symbols), label, file: file, line: line)
    XCTAssertFalse(shown(try button("returnKey", in: controller)), label, file: file, line: line)
    XCTAssertEqual(closing.alpha, 0, label, file: file, line: line)
    XCTAssertFalse(closing.isUserInteractionEnabled, label, file: file, line: line)
    XCTAssertFalse(closing.isAccessibilityElement, label, file: file, line: line)
    // 右列：⌫、中间键各占一格，回车从第三格的上边伸到底行的下边，与右列同宽。
    let rowGap = frame(middle).minY - frame(delete).maxY
    XCTAssertEqual(frame(tall).minX, frame(delete).minX, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(tall).width, frame(delete).width, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(tall).minY, frame(middle).maxY + rowGap, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(tall).minY, frame(closing).minY, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(tall).maxY, frame(space).maxY, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(tall).height, frame(closing).height + (frame(space).minY - frame(closing).maxY) + frame(space).height,
                   accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(slot).minX, frame(tall).minX, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(slot).width, frame(tall).width, accuracy: 0.5, label, file: file, line: line)
    // 左列：符 在符号栏正下方、同宽，与底行同一排。
    XCTAssertEqual(frame(symbols).minX, frame(sidebar).minX, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(symbols).width, frame(sidebar).width, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(symbols).minY, frame(space).minY, accuracy: 0.5, label, file: file, line: line)
    XCTAssertGreaterThan(frame(symbols).minY, frame(sidebar).maxY, label, file: file, line: line)
    // 中间几格的左右边就是九宫格（笔画键）的左右边。
    let grid: CGRect
    if let strokes = descendants(controller.view).first(where: { $0 is StrokeKeypadView && shown($0) }) {
      grid = frame(strokes)
    } else {
      grid = frame(try button("nineKey7", in: controller)).union(frame(try button("nineKey9", in: controller)))
    }
    let row = try XCTUnwrap(space.superview as? UIStackView)
    let middleKeys = row.arrangedSubviews.filter { !$0.isHidden && $0 !== symbols && $0 !== slot && $0.bounds.width > 0.5 }
    let first = try XCTUnwrap(middleKeys.first), last = try XCTUnwrap(middleKeys.last)
    XCTAssertEqual(frame(first).minX, grid.minX, accuracy: 0.5, label, file: file, line: line)
    XCTAssertEqual(frame(last).maxX, grid.maxX, accuracy: 0.5, label, file: file, line: line)
    // 123、中/英、地球键与标准底行上的同一个键等宽，0（数字层是空格）与标准底行的 符 等宽（都是 1.05 份）。标准底行先扣掉自己的键距再按份额分，不扣键距时这几个键会宽出几 pt。
    let references: [(String, String)] = [
      ("layoutToggleButton", "layoutToggleButton"), ("bottomLanguageKey", "bottomLanguageKey"),
      ("inputModeSwitchButton", "inputModeSwitchButton"), (digits ? "spaceKey" : "nineKeyZero", "symbolPanelKey"),
    ]
    for (identifier, reference) in references {
      let key = try view(identifier, in: controller)
      guard shown(key) else { continue }
      let expected = try XCTUnwrap(standard[reference], "\(label) the standard row has no \(reference)", file: file, line: line)
      // 容差是一个像素（3x 屏上 1/3pt）：两边都按像素取整，stack view 把舍入余量分给哪个键不一定相同。不扣键距时差出约 3pt，远在容差之外。
      XCTAssertEqual(key.bounds.width, expected, accuracy: 0.34, "\(label) \(identifier)", file: file, line: line)
    }
    // 空格（数字层是 0）占剩下的，是中间最宽的一格。
    let fill = try view(digits ? "nineKeyZero" : "spaceKey", in: controller)
    XCTAssertTrue(shown(fill), label, file: file, line: line)
    XCTAssertEqual(middleKeys.map(\.bounds.width).max() ?? 0, fill.bounds.width, accuracy: 0.01, label, file: file, line: line)
  }

  /// 标准九键底行上量到的键宽：123、地球键、中/英 和 符，按无障碍标识记，没显示的键不记。右侧大回车的中列拿它们比，而不是照公式重算一遍。
  private func standardNineKeyWidths(_ controller: KeyboardViewController, _ label: String,
                                     file: StaticString = #filePath, line: UInt = #line) throws -> [String: CGFloat] {
    XCTAssertTrue(shown(try button("returnKey", in: controller)), "\(label) draws the standard row", file: file, line: line)
    XCTAssertFalse(shown(try button("tallReturnKey", in: controller)), label, file: file, line: line)
    var widths: [String: CGFloat] = [:]
    for identifier in ["layoutToggleButton", "inputModeSwitchButton", "bottomLanguageKey", "symbolPanelKey"] {
      let key = try view(identifier, in: controller)
      if shown(key) && key.bounds.width > 0.5 { widths[identifier] = key.bounds.width }
    }
    return widths
  }

  /// 同一个键盘当场换成 `style`：写进文档后重新出现，后台读回的文档在主队列上应用，等到 `ready` 成立后按请求的高度排好。
  private func restyle(_ controller: KeyboardViewController, _ style: TouchKeyLayoutStyle, _ label: String,
                       until ready: () throws -> Bool, file: StaticString = #filePath, line: UInt = #line) async throws {
    XCTAssertTrue(write(style), label, file: file, line: line)
    controller.viewWillAppear(false)
    let deadline = Date().addingTimeInterval(15)
    while try !ready() && Date() < deadline {
      try await Task.sleep(for: .milliseconds(50))
    }
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(try ready(), "\(label) the new style is drawn on the same keyboard", file: file, line: line)
  }

  private func view(_ identifier: String, in controller: KeyboardViewController) throws -> UIView {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier },
                  "No view with accessibility identifier \(identifier).")
  }

  /// iPad 竖屏宽度的全宽键盘，按它请求的高度排好。
  private func tablet() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = .pad
    controller.traitOverrides.horizontalSizeClass = .regular
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 1032, height: KeyboardViewController.keyboardHeight(.tablet))
    controller.viewWillAppear(false)
    try? layoutAtRequestedHeight(controller)
    return controller
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

  // MARK: - 与 Android、HarmonyOS 对齐

  /// 设置页按版本列组，与 Android、鸿蒙同一规则：26 键一组只看中文 26 键（全拼、双拼、五笔、粤拼），日文罗马字、韩文、越南文和藏文的 26 键不算；14/18 键看全拼、双拼、五笔；9 键看全拼、双拼、笔画。既没有中文 26 键也没有 14/18 键时不列顶部数字行开关。
  func testSettingsOfferingFollowsTheEditionsSchemes() {
    func offering(_ engineSchemes: Set<String>) -> TouchKeyLayoutOffering {
      TouchKeyLayoutOffering { engineSchemes.contains($0.engineScheme) }
    }
    func groups(_ offering: TouchKeyLayoutOffering) -> [Bool] {
      [offering.twentySixKey, offering.letterGrid, offering.nineKey, offering.numberRow]
    }
    XCTAssertEqual(groups(offering(["quanpin"])), [true, true, true, true])
    XCTAssertEqual(groups(offering(["shuangpin"])), [true, true, false, true])
    XCTAssertEqual(groups(offering(["wubi"])), [true, true, false, true])
    XCTAssertEqual(groups(offering(["cantonese"])), [true, false, false, true])
    XCTAssertEqual(groups(offering(["stroke"])), [false, false, true, false])
    XCTAssertEqual(groups(offering(["zhuyin"])), [false, false, false, false])
    for scheme in ["japanese", "korean", "vietnamese", "tibetan"] {
      XCTAssertEqual(groups(offering([scheme])), [false, false, false, false], scheme)
    }
    XCTAssertEqual(groups(offering(["japanese", "korean", "vietnamese", "tibetan", "stroke"])), [false, false, true, false])
    // 键盘页的「按键布局」入口只在至少有一组时显示。
    XCTAssertTrue(offering(["stroke"]).offersAnyGroup)
    XCTAssertTrue(offering(["cantonese"]).offersAnyGroup)
    XCTAssertFalse(offering(["zhuyin"]).offersAnyGroup)
    XCTAssertFalse(offering(["japanese", "korean", "vietnamese", "tibetan"]).offersAnyGroup)
    XCTAssertFalse(offering([]).offersAnyGroup)
    // 列进 26 键一组的入口正是键盘按 26 键样式画的那些：写中文或粤拼、不是九键 / 14 / 18 键 / 注音 / 笔画 / 手写。
    let twentySixKey = ChineseInputScheme.allCases.filter {
      ($0.writesChinese || $0.isCantonese) && !$0.isMandarinNineKey && !$0.isStroke && !$0.isZhuyin
        && ![.handwriting, .fourteenKey, .eighteenKey].contains($0)
    }
    XCTAssertEqual(Set(TouchKeyLayoutOffering.twentySixKeySchemes), Set(twentySixKey))
  }

  /// 设置页的入口副标题和几段说明三端逐字相同，Android 和鸿蒙的设置页用同样的文字。
  func testSettingsTextsMatchTheOtherPlatforms() {
    XCTAssertEqual(TouchKeyLayoutOffering.entrySubtitle, "各类键盘的底行样式和顶部数字行")
    XCTAssertEqual(TouchKeyLayoutOffering.twentySixKeyFooter, "长空格：符号键移到工具栏；表情键：长按 123 打开符号。")
    XCTAssertEqual(TouchKeyLayoutOffering.letterGridFooter, "表情键：长按 123 打开符号。")
    XCTAssertEqual(TouchKeyLayoutOffering.nineKeyFooter, "右侧大回车：回车竖占两行，符号键在左下角。")
    XCTAssertEqual(TouchKeyLayoutOffering.numberRowSubtitle, "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。")
  }

  /// 26 键样式只作用于中文方案：日文罗马字、韩文、越南文和藏文的 26 键保留标准底行，长空格也不把 符 挪到工具栏。测试宿主没带某种语言的资源、选不上时跳过它。
  func testTwentySixKeyStylesApplyToChineseSchemesOnly() throws {
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace)))
    for scheme in [ChineseInputScheme.japanese, .korean, .vietnamese, .tibetan] {
      select(scheme)
      guard InputSchemePreference.scheme == scheme else { continue }
      let controller = keyboard()
      XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                     TouchBottomRowLayout.twentySixKey(.standard, globe: controller.needsInputModeSwitchKey).map(\.slot), scheme.rawValue)
      XCTAssertFalse(shown(try button("symbolsShortcut", in: controller)), scheme.rawValue)
      controller.viewDidDisappear(false)
    }
    select(.quanpin)
    let controller = keyboard()
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                   TouchBottomRowLayout.twentySixKey(.longSpace, globe: controller.needsInputModeSwitchKey).map(\.slot))
    controller.viewDidDisappear(false)
  }

  /// 「长空格」的字母层底行没有句号键，逗号与 Android、HarmonyOS 一样下滑输入句号：14 键和 18 键的逗号长按仍打开常用标点；26 键自定义了常用标点时下滑照样输入句号，长按打开那份菜单。别的样式的逗号没有下滑。
  func testLongSpaceCommaSwipesDownForTheFullStop() throws {
    for scheme in [ChineseInputScheme.fourteenKey, .eighteenKey] {
      select(scheme)
      for style in TouchFourteenKeyStyle.allCases {
        XCTAssertTrue(write(TouchKeyLayoutStyle(fourteenKey: style)))
        let controller = keyboard()
        let label = "\(scheme.rawValue) \(style.rawValue)"
        let comma = try XCTUnwrap(try button("quickPunctuationKey", in: controller) as? KeyboardKeyButton)
        XCTAssertNotNil(comma.menu, "long-press opens the quick punctuation menu: \(label)")
        if style == .longSpace {
          XCTAssertEqual(comma.cornerHint, "。", label)
          XCTAssertNotNil(comma.onCornerHint, label)
          XCTAssertTrue(comma.swipesCornerHint, label)
          XCTAssertEqual(comma.cornerHintSwipeDirection, .down, label)
          XCTAssertFalse(comma.holdsCornerHint, "a long press is left to the menu: \(label)")
        } else {
          XCTAssertNil(comma.cornerHint, label)
          XCTAssertFalse(comma.swipesCornerHint, label)
        }
        controller.viewDidDisappear(false)
      }
    }
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences()?["touch_quick_punctuation_symbols"]
    let storedSymbols = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_quick_punctuation_symbols"] = previous }
      KeyboardLayoutPreference.defaults.set(storedSymbols, forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    }
    XCTAssertTrue(KeyboardLayoutPreference.saveQuickPunctuationSymbols(["☆", "=", "！"]))
    select(.quanpin)
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace)))
    let controller = keyboard()
    let comma = try XCTUnwrap(try button("quickPunctuationKey", in: controller) as? KeyboardKeyButton)
    XCTAssertEqual(comma.configuration?.title, "☆")
    XCTAssertEqual(comma.menu?.children.map(\.title), ["☆", "=", "！"])
    XCTAssertEqual(comma.cornerHint, "。")
    XCTAssertNotNil(comma.onCornerHint)
    XCTAssertTrue(comma.swipesCornerHint, "the custom list keeps the down-swipe for the full stop")
    XCTAssertFalse(comma.holdsCornerHint, "a long press opens the custom menu")
    controller.viewDidDisappear(false)
  }

  /// 密码框和只收数字的输入框里三类样式都退回标准底行，与 Android 的 `bottomRowStyle` 相同；顶部数字行照画。回到普通输入框时样式回来。
  func testPasswordAndDigitFieldsKeepTheStandardRows() throws {
    XCTAssertTrue(KeyboardViewController.keepsStandardBottomRows(keyboardType: .default, secure: true, contentType: nil))
    for type in [UIKeyboardType.numberPad, .decimalPad, .asciiCapableNumberPad, .phonePad] {
      XCTAssertTrue(KeyboardViewController.keepsStandardBottomRows(keyboardType: type, secure: nil, contentType: nil), "\(type.rawValue)")
    }
    for contentType in [UITextContentType.password, .newPassword] {
      XCTAssertTrue(KeyboardViewController.keepsStandardBottomRows(keyboardType: .default, secure: false, contentType: contentType))
    }
    for type in [UIKeyboardType.default, .numbersAndPunctuation, .emailAddress, .URL, .namePhonePad, .twitter, .webSearch] {
      XCTAssertFalse(KeyboardViewController.keepsStandardBottomRows(keyboardType: type, secure: false, contentType: nil), "\(type.rawValue)")
    }
    XCTAssertFalse(KeyboardViewController.keepsStandardBottomRows(keyboardType: .default, secure: nil, contentType: .username))

    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace, nineKey: .tallReturn, numberRow: true)))
    select(.quanpin)
    var controller = keyboard()
    controller.applyInputContext(keyboardType: .default, documentIdentifier: UUID())
    try layoutAtRequestedHeight(controller)
    let globe = controller.needsInputModeSwitchKey
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot), TouchBottomRowLayout.twentySixKey(.longSpace, globe: globe).map(\.slot))
    controller.applyInputContext(keyboardType: .numberPad, documentIdentifier: UUID())
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot), TouchBottomRowLayout.twentySixKey(.standard, globe: globe).map(\.slot),
                   "a digit-only field keeps the standard row")
    XCTAssertFalse(shown(try button("symbolsShortcut", in: controller)))
    XCTAssertTrue(shown(try view("numberRow", in: controller)), "the number row is not a bottom-row style")
    controller.applyInputContext(keyboardType: .default, documentIdentifier: UUID())
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot), TouchBottomRowLayout.twentySixKey(.longSpace, globe: globe).map(\.slot),
                   "the style comes back in an ordinary field")
    controller.viewDidDisappear(false)

    select(.nineKey)
    controller = keyboard()
    controller.applyInputContext(keyboardType: .default, documentIdentifier: UUID())
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("tallReturnKey", in: controller)))
    controller.applyInputContext(keyboardType: .decimalPad, documentIdentifier: UUID())
    try layoutAtRequestedHeight(controller)
    XCTAssertFalse(shown(try button("tallReturnKey", in: controller)))
    XCTAssertTrue(shown(try button("returnKey", in: controller)))
    XCTAssertTrue(shown(try button("nineKeyClosingMark", in: controller)))
    XCTAssertEqual(try button("nineKeyClosingMark", in: controller).alpha, 1)
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                   TouchBottomRowLayout.nineKey(.standard, globe: controller.needsInputModeSwitchKey).map(\.slot))
    controller.viewDidDisappear(false)
  }

  /// iPad 没开「数字行与 Tab 键」时与手机一样按「顶部数字行」画：26 键（iPad 自己的字母排法）、14 键和 18 键的字母层上方多一排 1 到 0，键盘高一排；开着时 26 键照旧是 iPad 的数字行和 Tab，14 键和 18 键仍按「顶部数字行」画。14 键和九键的样式在 iPad 上同样生效。
  func testTabletDrawsTheNumberRowLikePhones() async throws {
    let storedFullKeys = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.tabletFullKeysKey)
    defer { KeyboardLayoutPreference.defaults.set(storedFullKeys, forKey: KeyboardLayoutPreference.tabletFullKeysKey) }
    let rowHeight = KeyboardHeightPercent.portraitKeyHeight(tablet: true) + CGFloat(KeyboardLayoutPreference.rowSpacing)
    for fullKeys in [false, true] {
      KeyboardLayoutPreference.tabletFullKeys = fullKeys
      for scheme in [ChineseInputScheme.quanpin, .fourteenKey, .eighteenKey] {
        select(scheme)
        let label = "\(scheme.rawValue) fullKeys=\(fullKeys)"
        XCTAssertTrue(write(TouchKeyLayoutStyle()))
        let without = tablet()
        let fullRow = fullKeys && scheme == .quanpin
        XCTAssertEqual(shown(try view("numberRow", in: without)), fullRow, label)
        let shorter = height(of: without)
        without.viewDidDisappear(false)
        XCTAssertTrue(write(TouchKeyLayoutStyle(fourteenKey: .longSpace, numberRow: true)))
        let controller = tablet()
        let row = try view("numberRow", in: controller)
        XCTAssertTrue(shown(row), label)
        XCTAssertEqual(try button("numberRowKey1", in: controller).configuration?.title, "1")
        XCTAssertEqual(height(of: controller) - shorter, fullRow ? 0 : rowHeight, accuracy: 0.5, label)
        if scheme != .quanpin {
          XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                         TouchBottomRowLayout.letterGrid(.longSpace, globe: controller.needsInputModeSwitchKey).map(\.slot), label)
          // 数字页用补充行占住这一排，切层时高度不变。
          try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
          try layoutAtRequestedHeight(controller)
          XCTAssertTrue(shown(try view("symbolExtraRow", in: controller)), label)
          XCTAssertEqual(height(of: controller) - shorter, rowHeight, accuracy: 0.5, "\(label) number layer")
        }
        controller.viewDidDisappear(false)
      }
    }
    KeyboardLayoutPreference.tabletFullKeys = false
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle(numberRow: true)))
    let controller = tablet()
    let standard = try standardNineKeyWidths(controller, "iPad")
    let tall = try button("tallReturnKey", in: controller)
    try await restyle(controller, TouchKeyLayoutStyle(nineKey: .tallReturn, numberRow: true), "iPad") { self.shown(tall) }
    XCTAssertTrue(shown(tall), "the nine-key style applies on iPad")
    XCTAssertFalse(shown(try view("numberRow", in: controller)), "nine-key has no number row")
    try assertTallReturnFrame(controller, digits: false, standard: standard, "iPad")
    controller.viewDidDisappear(false)
  }

  // MARK: - 右侧大回车

  /// 九键「右侧大回车」的外框：符 在符号栏正下方、与符号栏同宽；123 到 中/英 这几格正好在九宫格下方、与它左右对齐，空格占剩下的宽度；右列是 ⌫ 和中间键，回车从第三格伸到底行、两倍高中间无缝，底行原来回车的位置留给它；右列的 ！ 让出位置，接到符号栏末尾。九键双拼和笔画用同一个外框，九键双拼的 0 照旧跟在空格后面；隐藏中/英时空格变宽。
  func testTallReturnFrame() async throws {
    // 拼音九键标准底行上量到的键宽，按是否隐藏中/英记。九键双拼的标准底行多一个 0，右侧大回车仍按拼音九键那一行定宽，所以拿拼音九键量到的比。
    var pinyinStandard: [Bool: [String: CGFloat]] = [:]
    for scheme in [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke] {
      select(scheme)
      if scheme == .stroke && InputSchemePreference.scheme != .stroke { continue }
      for hidesLanguage in [false, true] {
        KeyboardLayoutPreference.hideLanguageKey = hidesLanguage
        let label = "\(scheme.rawValue) hidesLanguage=\(hidesLanguage)"
        // 先按标准样式量底行，再让同一个键盘当场换成右侧大回车。
        XCTAssertTrue(write(TouchKeyLayoutStyle()))
        let controller = keyboard()
        let measured = try standardNineKeyWidths(controller, label)
        if scheme == .nineKey { pinyinStandard[hidesLanguage] = measured }
        let standard = try XCTUnwrap(scheme == .nineKeyShuangpin ? pinyinStandard[hidesLanguage] : measured, label)
        let tall = try button("tallReturnKey", in: controller)
        try await restyle(controller, TouchKeyLayoutStyle(nineKey: .tallReturn), label) { self.shown(tall) }
        try assertTallReturnFrame(controller, digits: false, standard: standard, label)
        XCTAssertEqual(visibleBottomRow(controller).map(\.slot), TouchBottomRowLayout.nineKeyTallReturn(
          globe: controller.needsInputModeSwitchKey, zero: scheme == .nineKeyShuangpin, language: !hidesLanguage).map(\.slot), label)
        if scheme != .nineKeyShuangpin {
          XCTAssertFalse(shown(try button("nineKeyZero", in: controller)), label)
        }
        // ！ 接在符号栏默认标点的末尾。
        let sidebar = try view("nineKeySidebar", in: controller)
        let marks = descendants(sidebar).compactMap { $0 as? UIButton }.filter { shown($0) }
          .sorted { $0.convert($0.bounds, to: sidebar).minY < $1.convert($1.bounds, to: sidebar).minY }
          .compactMap { $0.configuration?.title }
        XCTAssertEqual(marks, ["，", "。", "？", "！"], label)
        if !hidesLanguage { attach(controller, "9 键 右侧大回车 \(scheme.rawValue)") }
        controller.viewDidDisappear(false)
      }
    }
    KeyboardLayoutPreference.hideLanguageKey = false
    // 其他样式照旧：右列第三格是 ！，符号栏只有 ，。？，没有伸下来的回车。
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle(nineKey: .commaPeriod)))
    let controller = keyboard()
    XCTAssertFalse(shown(try button("tallReturnKey", in: controller)))
    XCTAssertFalse(shown(try button("nineKeySidebarClosingMark", in: controller)))
    XCTAssertFalse(shown(try view("tallReturnSlot", in: controller)))
    let closing = try button("nineKeyClosingMark", in: controller)
    XCTAssertTrue(shown(closing))
    XCTAssertEqual(closing.alpha, 1)
    XCTAssertTrue(closing.isAccessibilityElement)
    controller.viewDidDisappear(false)
  }

  /// 右侧大回车与底行的回车是同一个键：键面跟着组字和输入框一起变，按下与底行的回车一样结束组字；符号栏末尾的 ！ 按下同样结束组字。中/英 与它不在同一行，回车不把左边让给中/英。
  func testTallReturnActsAsTheReturnKey() throws {
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle(nineKey: .tallReturn)))
    let controller = keyboard()
    let tall = try button("tallReturnKey", in: controller)
    let enter = try button("returnKey", in: controller)
    func sameFace(_ label: String) {
      XCTAssertEqual(tall.configuration?.title, enter.configuration?.title, label)
      XCTAssertEqual(tall.configuration?.image, enter.configuration?.image, label)
      XCTAssertEqual(tall.accessibilityLabel, enter.accessibilityLabel, label)
      XCTAssertEqual(tall.configuration?.background.backgroundColor, enter.configuration?.background.backgroundColor, label)
    }
    func compose() throws {
      try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
      try button("nineKey4", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse((try button("preeditButton", in: controller).accessibilityValue ?? "").isEmpty, "typing opens a composition")
    }
    sameFace("idle")
    let idleTitle = tall.configuration?.title
    let idlePreedit = try button("preeditButton", in: controller).accessibilityValue
    try compose()
    sameFace("composing")
    XCTAssertEqual(tall.configuration?.title, "确认")
    // 按下右侧大回车：组字结束，键面回到空闲时的样子，与按底行的回车相同。
    tall.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("preeditButton", in: controller).accessibilityValue, idlePreedit, "the tall return ends the composition")
    XCTAssertEqual(tall.configuration?.title, idleTitle)
    sameFace("after return")
    // 符号栏末尾的 ！ 是能按的标点键：组字时按下，组字随标点一起上屏。
    let sidebarMark = try button("nineKeySidebarClosingMark", in: controller)
    XCTAssertTrue(shown(sidebarMark))
    XCTAssertEqual(sidebarMark.accessibilityLabel, "符号 ！")
    try compose()
    sidebarMark.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("preeditButton", in: controller).accessibilityValue, idlePreedit, "the sidebar ！ ends the composition")
    XCTAssertEqual(tall.configuration?.title, idleTitle)
    // 回车格左边几 pt 不让给 中/英：中/英 在底行，回车从上一排伸下来，两者之间留着回车自己的位置。
    let root = try XCTUnwrap(descendants(controller.view).first { $0 is KeyAreaStackView })
    let slot = try view("tallReturnSlot", in: controller)
    let frame = slot.convert(slot.bounds, to: root)
    XCTAssertTrue(root.hitTest(CGPoint(x: frame.minX + 2, y: frame.midY), with: nil) === tall)
    XCTAssertTrue(root.hitTest(CGPoint(x: frame.midX, y: frame.minY - 10), with: nil) === tall)
    controller.viewDidDisappear(false)
  }

  /// 右侧大回车叠在按键列上、不在任何一行里，它与相邻键之间的键距照样两边各分一半：底行里它左边的 中/英（隐藏时是空格）、第三排里它左边的九宫格 9 键、右列里它上面的中间键。标准外框里回车和 ！ 各自分到这几段键距里靠自己的那一半，这里也一样。
  func testTallReturnTakesItsHalfOfTheNeighbouringGaps() throws {
    select(.nineKey)
    for hidesLanguage in [false, true] {
      KeyboardLayoutPreference.hideLanguageKey = hidesLanguage
      XCTAssertTrue(write(TouchKeyLayoutStyle(nineKey: .tallReturn)))
      let controller = keyboard()
      let label = "hidesLanguage=\(hidesLanguage)"
      let root = try XCTUnwrap(descendants(controller.view).first { $0 is KeyAreaStackView })
      func frame(_ view: UIView) -> CGRect { view.convert(view.bounds, to: root) }
      let tall = try button("tallReturnKey", in: controller)
      let slot = frame(try view("tallReturnSlot", in: controller))
      let tallFrame = frame(tall)
      let left = try button(hidesLanguage ? "spaceKey" : "bottomLanguageKey", in: controller)
      let middle = try button("nineKeyMiddleKey", in: controller)
      let nine = try button("nineKey9", in: controller)
      let closing = frame(try button("nineKeyClosingMark", in: controller))
      // 底行：回车格左边紧挨着的一点归回车，靠 中/英 的一点归 中/英。
      XCTAssertGreaterThan(slot.minX - frame(left).maxX, 2, label)
      XCTAssertTrue(root.hitTest(CGPoint(x: slot.minX - 1, y: slot.midY), with: nil) === tall, label)
      XCTAssertTrue(root.hitTest(CGPoint(x: frame(left).maxX + 1, y: slot.midY), with: nil) === left, label)
      // 第三排：九宫格 9 键与回车之间的键距。
      XCTAssertGreaterThan(tallFrame.minX - frame(nine).maxX, 2, label)
      XCTAssertTrue(root.hitTest(CGPoint(x: tallFrame.minX - 1, y: closing.midY), with: nil) === tall, label)
      XCTAssertTrue(root.hitTest(CGPoint(x: frame(nine).maxX + 1, y: closing.midY), with: nil) === nine, label)
      // 右列：中间键与回车之间的行距。
      XCTAssertGreaterThan(tallFrame.minY - frame(middle).maxY, 2, label)
      XCTAssertTrue(root.hitTest(CGPoint(x: tallFrame.midX, y: tallFrame.minY - 1), with: nil) === tall, label)
      XCTAssertTrue(root.hitTest(CGPoint(x: tallFrame.midX, y: frame(middle).maxY + 1), with: nil) === middle, label)
      controller.viewDidDisappear(false)
    }
    KeyboardLayoutPreference.hideLanguageKey = false
    // 标准外框：同样两处键距里靠右列第三格的一半归 ！。
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    let controller = keyboard()
    let root = try XCTUnwrap(descendants(controller.view).first { $0 is KeyAreaStackView })
    let closing = try button("nineKeyClosingMark", in: controller)
    let frame = closing.convert(closing.bounds, to: root)
    XCTAssertTrue(root.hitTest(CGPoint(x: frame.minX - 1, y: frame.midY), with: nil) === closing)
    XCTAssertTrue(root.hitTest(CGPoint(x: frame.midX, y: frame.minY - 1), with: nil) === closing)
    XCTAssertFalse(shown(try button("tallReturnKey", in: controller)))
    controller.viewDidDisappear(false)
  }

  /// 右侧大回车的数字层照同一外框：符 留在符号栏下方，123 的位置是回到字母层的键，0 占空格的位置、空格在它右边，回车仍两倍高；数字层符号栏里的 符 移到底行，运算符占满整条符号栏。从别的键面借来的数字九宫格不跟九键的样式。
  func testTallReturnNumberLayer() async throws {
    var pinyinStandard: [String: CGFloat] = [:]
    for scheme in [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke] {
      select(scheme)
      if scheme == .stroke && InputSchemePreference.scheme != .stroke { continue }
      let label = scheme.rawValue
      // 切换键、地球键、中/英 在数字层里也与字母层的标准底行等宽，切层时不横移；九键双拼同样拿拼音九键的标准底行比。
      XCTAssertTrue(write(TouchKeyLayoutStyle()))
      let controller = keyboard()
      let measured = try standardNineKeyWidths(controller, label)
      if scheme == .nineKey { pinyinStandard = measured }
      let standard = scheme == .nineKeyShuangpin ? pinyinStandard : measured
      let tall = try button("tallReturnKey", in: controller)
      try await restyle(controller, TouchKeyLayoutStyle(nineKey: .tallReturn), label) { self.shown(tall) }
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      XCTAssertEqual(try button("nineKey7", in: controller).configuration?.title, "7", label)
      try assertTallReturnFrame(controller, digits: true, standard: standard, label)
      XCTAssertEqual(visibleBottomRow(controller).map(\.slot), TouchBottomRowLayout.nineKeyTallReturn(
        globe: controller.needsInputModeSwitchKey, digits: true).map(\.slot), label)
      XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, scheme == .stroke ? "笔" : "九键", label)
      XCTAssertFalse(shown(try button("numberPadSymbolPanelKey", in: controller)), label)
      let sidebar = try view("nineKeySidebar", in: controller)
      let operators = try view("numberPadOperatorScroll", in: controller)
      XCTAssertEqual(operators.bounds.height, sidebar.bounds.height, accuracy: 0.5, "the operators fill the sidebar: \(label)")
      attach(controller, "9 键 右侧大回车 数字层 \(scheme.rawValue)")
      // 回到字母层，外框照旧。
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      try assertTallReturnFrame(controller, digits: false, standard: standard, "\(label) letters again")
      controller.viewDidDisappear(false)
    }
    // 标准九键的数字层：符 在符号栏第三排。
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    var controller = keyboard()
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("numberPadSymbolPanelKey", in: controller)))
    XCTAssertEqual(try button("numberPadSymbolPanelKey", in: controller).bounds.height,
                   try button("nineKeyDelete", in: controller).bounds.height, accuracy: 0.5)
    controller.viewDidDisappear(false)
    // 全拼借用的数字九宫格。
    select(.quanpin)
    XCTAssertTrue(write(TouchKeyLayoutStyle(nineKey: .tallReturn)))
    controller = keyboard()
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("nineKey7", in: controller)), "quanpin borrows the number pad")
    XCTAssertFalse(shown(try button("tallReturnKey", in: controller)))
    XCTAssertTrue(shown(try button("returnKey", in: controller)))
    controller.viewDidDisappear(false)
  }

  /// 热力图照右侧大回车的外框画：侧栏是 ，。？！，侧栏下方是 符，九宫格下方是 123 | 空格 | 中，右列是 ⌫、分词和跨两排的回车。侧栏是唯一记 `SoftPunctuation` 的键，于是印占比；九宫格下方几格与真键盘底行的键一一对应。
  func testTallReturnHeatmap() throws {
    let style = TouchKeyLayoutStyle(nineKey: .tallReturn)
    let cells = TypingKeyHeatmap.cells(nineKey: true, style: style)
    XCTAssertEqual(cells.map(\.face), ["，。？！"] + TypingKeyHeatmap.nineKeyGrid.joined().map(\.face)
      + ["删除", "分词", "符", "123", "空格", "中", "换行"])
    XCTAssertEqual(TypingKeyHeatmap.nineKeySidebar(for: .tallReturn).id, TypingKeyID.punctuation)
    XCTAssertEqual(TypingKeyHeatmap.sharedFaces(TypingKeyHeatmap.nineKeySidebar(for: .tallReturn), nineKey: true, style: style), [])
    XCTAssertEqual(cells.map(\.id).filter { !TypingKeyID.known.contains($0) }, [])
    // 其他样式的侧栏和右列不变。
    for nine in TouchNineKeyStyle.allCases where nine != .tallReturn {
      XCTAssertEqual(TypingKeyHeatmap.nineKeySidebar(for: nine), TypingKeyHeatmap.nineKeySidebar)
      XCTAssertEqual(TypingKeyHeatmap.nineKeyControls(for: nine), TypingKeyHeatmap.nineKeyControls)
    }
    select(.nineKey)
    XCTAssertTrue(write(style))
    let controller = keyboard()
    let bottom = try XCTUnwrap(try button("spaceKey", in: controller).superview as? UIStackView)
    let drawn = bottom.arrangedSubviews.filter {
      !$0.isHidden && $0.accessibilityIdentifier != "inputModeSwitchButton" && $0.accessibilityIdentifier != "tallReturnSlot"
    }
    let expected = TypingKeyHeatmap.nineKeyBottomCells(for: .tallReturn)
    XCTAssertEqual(drawn.compactMap { $0.accessibilityIdentifier.flatMap { Self.recordedID[$0] } }, expected.dropLast().map(\.id))
    // 最后一格画的是伸下来的回车，不在底行里，按 Return 计。
    XCTAssertEqual(expected.last?.id, TypingKeyID.enter)
    controller.viewDidDisappear(false)
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
    for style in TouchNineKeyStyle.allCases where style != .tallReturn {
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
    "bottomLanguageKey": .language, "returnKey": .returnKey, "tallReturnSlot": .returnKey,
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

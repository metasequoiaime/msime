import ObjectiveC
import XCTest
import UIKit

/// 「按键布局」：三类键面的底行样式和顶部数字行。固定共享偏好的读法、底行表里每种样式的键序和份额、真键盘按表排出的底行、数字行的门槛与高度、样式在键盘开着时当场生效，以及统计热力图跟着样式画。三端同一套排法，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
@MainActor
final class KeyLayoutStyleTests: XCTestCase {
  private typealias Slot = TouchBottomRowLayout.Slot
  private static let styleKeys = [
    TouchKeyLayoutStyle.twentySixKeyDocumentKey, TouchKeyLayoutStyle.fourteenKeyDocumentKey,
    TouchKeyLayoutStyle.nineKeyDocumentKey, TouchKeyLayoutStyle.numberRowDocumentKey,
    TouchKeyLayoutStyle.symbolFirstDocumentKey, TouchKeyLayoutStyle.numberPadSpaceDocumentKey,
    TouchKeyLayoutStyle.nineKeySidebarDocumentKey, TouchKeyLayoutStyle.digitSidebarDocumentKey,
    TouchKeyLayoutStyle.languageKeySideDocumentKey,
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
    XCTAssertEqual(TouchTwentySixKeyStyle.allCases.map(\.title), ["标准", "逗号句号", "长空格", "表情键"])
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
    // 九键一族自己的数字层（#10061）：符 和切换键与字母层同一格、同宽，0 和空格合起来占字母层空格（九键双拼还有它的 0）那一份；隐藏中/英时它的份额给 0。
    XCTAssertEqual(row(TouchBottomRowLayout.nineKeyDigits(globe: false)),
                   ["symbols 1.05", "layer 1.25", "zero 3.15", "space 1.05", "language 1.05", "returnKey 1.60"])
    XCTAssertEqual(row(TouchBottomRowLayout.nineKeyDigits(globe: true, zero: true, language: false)),
                   ["symbols 1.05", "layer 1.25", "globe 1.00", "zero 5.25", "space 1.05", "returnKey 1.60"])
    func total(_ keys: [TouchBottomRowLayout.Key]) -> Double { keys.reduce(0) { $0 + Double($1.weight) } }
    for globe in [false, true] {
      for zero in [false, true] {
        for language in [false, true] {
          let letters = TouchBottomRowLayout.nineKey(.standard, globe: globe, zero: zero, language: language)
          let digits = TouchBottomRowLayout.nineKeyDigits(globe: globe, zero: zero, language: language)
          let label = "globe \(globe) zero \(zero) language \(language)"
          XCTAssertEqual(total(digits), total(letters), accuracy: 1e-9, label)
          XCTAssertEqual(Array(digits.prefix(globe ? 3 : 2)), Array(letters.prefix(globe ? 3 : 2)), label)
          XCTAssertEqual(digits.last, letters.last, label)
          XCTAssertEqual(digits.filter { $0.slot == .zero }.count, 1, label)
        }
      }
    }
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

  /// 26 键各样式里逗号和句号的手势：标准和表情键的逗号长按出句号；逗号句号在空格两侧时逗号长按打开常用标点、句号是独立的键；长空格时逗号上滑、下滑或长按都出句号。
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
        XCTAssertTrue(comma.swipesCornerHint, "a swipe types the full stop")
        XCTAssertTrue(comma.cornerHintSwipesEitherWay, "upward or downward")
        XCTAssertTrue(comma.holdsCornerHint, "so does a long press")
      case .standard, .emoji:
        XCTAssertEqual(comma.cornerHint, "。", style.rawValue)
        XCTAssertFalse(comma.swipesCornerHint, style.rawValue)
        XCTAssertNil(comma.menu, style.rawValue)
      }
      controller.viewDidDisappear(false)
    }
  }

  /// 长空格：符 挪到工具栏；数字入口与其他样式一样是数字九宫格，它的底行有空格（长按空格语音照常）；#+= 层原来的表情或符号入口换成逗号；工具栏显示方式为隐藏时这一行仍然显示，只有品牌、符号和收起。
  func testLongSpaceMovesSymbolsToTheToolbarAndKeepsASpacedNumberLayer() throws {
    select(.quanpin)
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace)))
    var controller = keyboard()
    let symbols = try button("symbolsShortcut", in: controller)
    XCTAssertTrue(shown(symbols), "the symbol entry moves to the toolbar")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("nineKey1", in: controller)), "123 opens the number pad")
    XCTAssertFalse(shown(try button("symbolLayerKey0_0", in: controller)))
    XCTAssertTrue(shown(try button("spaceKey", in: controller)), "the number pad has a space")
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "拼音")
    try button("numberPadMoreSymbols", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    let layerComma = try XCTUnwrap(try button("layerCommaKey", in: controller) as? KeyboardKeyButton)
    XCTAssertTrue(shown(layerComma))
    XCTAssertEqual(layerComma.configuration?.title, "，")
    XCTAssertEqual(layerComma.cornerHint, "。")
    XCTAssertTrue(layerComma.swipesCornerHint)
    XCTAssertTrue(layerComma.cornerHintSwipesEitherWay, "the layer comma swipes up or down for the full stop")
    XCTAssertTrue(layerComma.holdsCornerHint, "and a long press types it too")
    XCTAssertFalse(shown(try button("layerEmojiKey", in: controller)))
    XCTAssertFalse(shown(try button("symbolPanelKey", in: controller)))
    let unit = try button("spaceKey", in: controller).bounds.width / SymbolLayerLayout.spaceWeight
    XCTAssertEqual(layerComma.bounds.width, unit * SymbolLayerLayout.panelWeight, accuracy: 0.5)
    attach(controller, "26 键 长空格 #+= 层")
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

  /// 「键盘工具栏」设置页的预览（`KeyboardToolbarPreviewRow`）与真键盘空闲时的工具栏逐个按钮对得上：每种 26 键样式、显示方式为「输入时显示」和「隐藏」各量一遍。只有长空格把 符 放在品牌键后面；表情键样式底行也没有 符，入口是长按 123，工具栏上没有；隐藏时长空格这一行只剩品牌、符 和收起，其他样式不画工具栏内容。
  func testToolbarPreviewMatchesTheKeyboardToolbarForEveryTwentySixKeyStyle() throws {
    select(.quanpin)
    let ids: [KeyboardPreviewToolbarItem: String] = [
      .symbols: "symbolsShortcut", .emoji: "emojiShortcut", .phrases: "phrasesShortcut", .clipboard: "clipboardShortcut",
      .skin: "skinShortcut", .scheme: "schemeButton", .ai: "aiShortcut", .characterSet: "characterSetShortcut",
      .fullwidth: "fullwidthShortcut", .punctuation: "punctuationShortcut",
    ]
    XCTAssertEqual(Set(ids.keys), Set(KeyboardPreviewToolbarItem.allCases))
    for hidden in [false, true] {
      TouchToolbarLocalPreference.hidden = hidden
      for style in TouchTwentySixKeyStyle.allCases {
        let layout = TouchKeyLayoutStyle(twentySixKey: style)
        XCTAssertTrue(write(layout))
        let controller = keyboard()
        let label = "\(style.rawValue), hidden \(hidden)"
        let bar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" } as? UIStackView)
        let preview = KeyboardToolbarPreviewRow.items(TouchToolbarPreference.load(), phrases: TouchToolbarLocalPreference.phrases,
                                                      scheme: TouchToolbarLocalPreference.scheme, hidden: hidden, keyLayout: layout)
        XCTAssertEqual(bar.isHidden, preview == nil, label)
        if let preview {
          // 品牌键、收起键以及预览不画的回复和语音入口不比。
          let drawn = Set(ids.values)
          let visible = bar.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier).filter(drawn.contains)
          XCTAssertEqual(visible, preview.compactMap { ids[$0] }, label)
          XCTAssertEqual(bar.arrangedSubviews.filter { !$0.isHidden }.first?.accessibilityIdentifier, "moreShortcut", label)
        }
        XCTAssertEqual(preview?.contains(.symbols) ?? false, style == .longSpace, label)
        XCTAssertEqual(shown(try button("symbolsShortcut", in: controller)), style == .longSpace, label)
        controller.viewDidDisappear(false)
      }
    }
  }

  /// 工具栏设成隐藏、26 键选了长空格时来回切中/英，键盘高度和按键位置都不变：英文 26 键画标准底行，工具栏上的 符 跟着消失，但这一行按键盘留着，只剩品牌和收起，与 Android、鸿蒙相同；iOS 的顶栏本来就一直占着它的高度。Android 和鸿蒙曾在这里随中/英把整行工具栏收起又放出，键盘跟着跳；iOS 曾在英文时把整条工具栏连同品牌和收起一起藏掉。
  func testLongSpaceToolbarRowKeepsTheHeightAcrossLanguageToggles() throws {
    select(.quanpin)
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace)))
    TouchToolbarLocalPreference.hidden = true
    let controller = keyboard()
    let symbols = try button("symbolsShortcut", in: controller)
    let space = try button("spaceKey", in: controller)
    let bar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" } as? UIStackView)
    let chineseHeight = height(of: controller)
    let chineseSpaceTop = controller.view.convert(space.bounds, from: space).minY
    XCTAssertTrue(shown(symbols), "Chinese long space carries the symbol entry on the hidden toolbar")
    for (pass, english) in [true, false].enumerated() {
      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      XCTAssertEqual(shown(symbols), !english, "pass \(pass): only Chinese long space moves 符 to the toolbar")
      XCTAssertFalse(bar.isHidden, "pass \(pass): the long-space keyboard keeps its toolbar row in English too")
      XCTAssertEqual(bar.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier),
                     english ? ["moreShortcut", "dismissShortcut"] : ["moreShortcut", "symbolsShortcut", "dismissShortcut"],
                     "pass \(pass): English keeps only the logo and the collapse button")
      XCTAssertEqual(height(of: controller), chineseHeight, accuracy: 0.5, "pass \(pass): the keyboard height stays")
      XCTAssertEqual(controller.view.convert(space.bounds, from: space).minY, chineseSpaceTop, accuracy: 0.5,
                     "pass \(pass): the bottom row does not move")
    }
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

  /// 九键各样式，笔画沿用九键外框也跟着九键的样式；九键双拼字母层的 0 跟在空格后面；隐藏中/英时它的份额给空格。数字层不跟样式，按 `TouchBottomRowLayout.nineKeyDigits` 排。右侧大回车换的是整个外框，见 `testTallReturnFrame`。
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
          // 数字层不跟样式：符 | 九键 | 0 | 空格 | 中 | 回车，符 和切换键与字母层同一格（#10061），没有 ， 和 。。
          try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
          controller.view.layoutIfNeeded()
          try assertBottomRow(controller, matches: TouchBottomRowLayout.nineKeyDigits(
            globe: controller.needsInputModeSwitchKey, zero: scheme == .nineKeyShuangpin, language: !hidesLanguage),
                              "\(scheme.rawValue) \(style.rawValue) digits hidesLanguage=\(hidesLanguage)")
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
      // 英文 26 键也有数字行，它的 123 同样打开数字九宫格，补充行占住这一排，补的是 ASCII 的括号。
      if scheme == .quanpin {
        try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
        try layoutAtRequestedHeight(controller)
        XCTAssertTrue(shown(row))
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        try layoutAtRequestedHeight(controller)
        XCTAssertTrue(shown(try button("nineKey1", in: controller)))
        XCTAssertTrue(shown(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "symbolExtraRow" })))
        XCTAssertEqual(try button("symbolExtraKey0", in: controller).configuration?.title, "(")
        XCTAssertEqual(height(of: controller), taller, accuracy: 0.5, "English number pad")
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
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

  /// 设置页的入口副标题、各样式的说明和数字行开关的说明三端逐字相同，Android 和鸿蒙的设置页用同样的文字。说明只给有副作用的四个样式，其余样式页面不画说明这一行。
  func testSettingsTextsMatchTheOtherPlatforms() {
    XCTAssertEqual(TouchKeyLayoutOffering.entrySubtitle, "各类键盘的底行样式、键位和顶部数字行")
    XCTAssertEqual(TouchKeyLayoutOffering.numberRowSubtitle, "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。")
    XCTAssertEqual(TouchTwentySixKeyStyle.allCases.map(TouchKeyLayoutOffering.note),
                   [nil, nil, "符号键移到工具栏。", "长按 123 打开符号。"])
    XCTAssertEqual(TouchFourteenKeyStyle.allCases.map(TouchKeyLayoutOffering.note), [nil, nil, "长按 123 打开符号。"])
    XCTAssertEqual(TouchNineKeyStyle.allCases.map(TouchKeyLayoutOffering.note), [nil, nil, nil, "回车竖占两行，符号键在左下角。"])
    // 页面按分段取当前样式的说明：换了分段说的是那一类选中的样式。
    let style = TouchKeyLayoutStyle(twentySixKey: .longSpace, fourteenKey: .standard, nineKey: .tallReturn)
    XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.note(style), "符号键移到工具栏。")
    XCTAssertNil(TouchKeyLayoutKind.letterGrid.note(style))
    XCTAssertEqual(TouchKeyLayoutKind.nineKey.note(style), "回车竖占两行，符号键在左下角。")
    XCTAssertNil(TouchKeyLayoutKind.twentySixKey.note(TouchKeyLayoutStyle()))
    XCTAssertEqual(TouchKeyLayoutKind.letterGrid.note(TouchKeyLayoutStyle(fourteenKey: .emoji)), "长按 123 打开符号。")
  }

  /// 「按键布局」页顶部的分段：按 26 键、14/18 键、9 键的顺序只列本版本提供的类型，分段名和 identifier 三端一致；只有一类时页面不画分段控件，一类都没有时没有分段。
  func testSettingsPageKindsFollowTheOffering() {
    func offering(_ engineSchemes: Set<String>) -> TouchKeyLayoutOffering {
      TouchKeyLayoutOffering { engineSchemes.contains($0.engineScheme) }
    }
    XCTAssertEqual(TouchKeyLayoutKind.allCases.map(\.title), ["26 键", "14/18 键", "9 键"])
    XCTAssertEqual(TouchKeyLayoutKind.allCases.map(\.identifier), ["keyLayoutKind_26", "keyLayoutKind_14", "keyLayoutKind_9"])
    XCTAssertEqual(offering(["quanpin"]).kinds, [.twentySixKey, .letterGrid, .nineKey])
    XCTAssertEqual(offering(["shuangpin"]).kinds, [.twentySixKey, .letterGrid])
    XCTAssertEqual(offering(["cantonese"]).kinds, [.twentySixKey])
    XCTAssertEqual(offering(["stroke"]).kinds, [.nineKey])
    XCTAssertEqual(offering(["zhuyin"]).kinds, [])
    XCTAssertNil(offering(["zhuyin"]).initialKind(current: .twentySixKey))
  }

  /// 分段默认停在用户当前在用的中文键盘那一类：全拼、双拼、五笔、粤拼的 26 键停在 26 键；全拼 14 键、18 键和选了 14/18 键布局的双拼、五笔停在 14/18 键；九键、九键双拼和笔画停在 9 键。判断不了（手写、注音、非中文 26 键）或本版本不列那一类时停在第一个。
  func testSettingsPageStartsOnTheCurrentKeyboard() {
    func kind(_ scheme: ChineseInputScheme, shuangpin: TouchShuangpinLayoutPreference.Layout = .twentySixKey,
              wubi: TouchWubiLayoutPreference.Layout = .twentySixKey) -> TouchKeyLayoutKind? {
      TouchKeyLayoutKind.of(scheme, shuangpinLayout: shuangpin, wubiLayout: wubi)
    }
    for scheme in [ChineseInputScheme.quanpin, .shuangpin, .ziranma, .microsoft, .shoudao, .wubi, .cantonese, .cantoneseShuangpin] {
      XCTAssertEqual(kind(scheme), .twentySixKey, scheme.rawValue)
    }
    for scheme in [ChineseInputScheme.fourteenKey, .eighteenKey] {
      XCTAssertEqual(kind(scheme), .letterGrid, scheme.rawValue)
    }
    XCTAssertEqual(kind(.shuangpin, shuangpin: .fourteenKey), .letterGrid)
    XCTAssertEqual(kind(.ziranma, shuangpin: .eighteenKey), .letterGrid)
    XCTAssertEqual(kind(.wubi, wubi: .fourteenKey), .letterGrid)
    // 双拼、五笔的字母键布局只管它们自己：全拼 26 键不因为双拼选了 14 键就算 14/18 键。
    XCTAssertEqual(kind(.quanpin, shuangpin: .fourteenKey, wubi: .fourteenKey), .twentySixKey)
    for scheme in [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke] {
      XCTAssertEqual(kind(scheme), .nineKey, scheme.rawValue)
    }
    for scheme in [ChineseInputScheme.handwriting, .zhuyin, .japanese, .japaneseNineKey, .korean, .vietnamese, .tibetan] {
      XCTAssertNil(kind(scheme), scheme.rawValue)
    }
    let everything = TouchKeyLayoutOffering { _ in true }
    XCTAssertEqual(everything.initialKind(current: .nineKey), .nineKey)
    XCTAssertEqual(everything.initialKind(current: .letterGrid), .letterGrid)
    XCTAssertEqual(everything.initialKind(current: nil), .twentySixKey, "a keyboard the page cannot place starts on the first kind")
    let strokeOnly = TouchKeyLayoutOffering { $0.engineScheme == "stroke" }
    XCTAssertEqual(strokeOnly.initialKind(current: .twentySixKey), .nineKey, "a kind the edition does not list falls back to the first one listed")
  }

  /// 预览里高亮的键按底行表的键位认：标准样式不高亮，其余样式高亮它与标准不同的那几个键，而且这些键都在这个样式的底行里。
  func testSettingsPreviewHighlightsWhatTheStyleChanges() {
    typealias Slots = Set<Slot>
    XCTAssertEqual(TouchTwentySixKeyStyle.allCases.map(TouchKeyLayoutOffering.highlightedSlots),
                   [[], [.comma, .period], [.space], [.emoji]] as [Slots])
    XCTAssertEqual(TouchFourteenKeyStyle.allCases.map(TouchKeyLayoutOffering.highlightedSlots), [[], [.space], [.emoji]] as [Slots])
    XCTAssertEqual(TouchNineKeyStyle.allCases.map(TouchKeyLayoutOffering.highlightedSlots),
                   [[], [.comma, .period], [.emoji], [.returnKey]] as [Slots])
    let styles = TouchTwentySixKeyStyle.allCases.flatMap { twentySix in
      TouchFourteenKeyStyle.allCases.flatMap { fourteen in
        TouchNineKeyStyle.allCases.map { TouchKeyLayoutStyle(twentySixKey: twentySix, fourteenKey: fourteen, nineKey: $0) }
      }
    }
    for style in styles {
      for kind in TouchKeyLayoutKind.allCases {
        let label = "\(kind.rawValue) \(style)"
        let row = Set(kind.bottomRow(style).map(\.slot))
        XCTAssertTrue(kind.highlightedSlots(style).isSubset(of: row), label)
      }
    }
    // 预览的底行就是键盘用的那张表，隐藏中/英键时与键盘一样把它的份额并进空格。
    let style = TouchKeyLayoutStyle(twentySixKey: .emoji, fourteenKey: .longSpace, nineKey: .tallReturn)
    XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.bottomRow(style), TouchBottomRowLayout.twentySixKey(.emoji, globe: false))
    XCTAssertEqual(TouchKeyLayoutKind.letterGrid.bottomRow(style), TouchBottomRowLayout.letterGrid(.longSpace, globe: false))
    XCTAssertEqual(TouchKeyLayoutKind.nineKey.bottomRow(style), TouchBottomRowLayout.nineKey(.tallReturn, globe: false))
    XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.bottomRow(style, language: false),
                   TouchBottomRowLayout.twentySixKey(.emoji, globe: false, language: false))
  }

  /// 打开「符」与「123」互换后，26 键分段的预览底行照键盘把 符 排到 123 左边，并在样式高亮之外再高亮 符 和 123；长空格和表情键样式没有 符，底行和高亮都不变；14/18 键和 9 键不受这个开关影响。
  func testSettingsPreviewFollowsSymbolFirst() {
    for style in TouchTwentySixKeyStyle.allCases {
      let off = TouchKeyLayoutStyle(twentySixKey: style)
      let on = TouchKeyLayoutStyle(twentySixKey: style, symbolFirst: true)
      let label = style.rawValue
      let standardRow = TouchBottomRowLayout.twentySixKey(style, globe: false)
      XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.bottomRow(off), standardRow, label)
      XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.bottomRow(on), TouchBottomRowLayout.symbolFirst(standardRow), label)
      XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.bottomRow(on, language: false),
                     TouchBottomRowLayout.symbolFirst(TouchBottomRowLayout.twentySixKey(style, globe: false, language: false)), label)
      XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.highlightedSlots(off), TouchKeyLayoutOffering.highlightedSlots(style), label)
      let slots = TouchKeyLayoutKind.twentySixKey.bottomRow(on).map(\.slot)
      if standardRow.contains(where: { $0.slot == .symbols }) {
        XCTAssertEqual(Array(slots.prefix(2)), [.symbols, .layer], label)
        XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.highlightedSlots(on),
                       TouchKeyLayoutOffering.highlightedSlots(style).union([.symbols, .layer]), label)
      } else {
        XCTAssertEqual(slots, standardRow.map(\.slot), label)
        XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.highlightedSlots(on), TouchKeyLayoutOffering.highlightedSlots(style), label)
      }
      XCTAssertTrue(TouchKeyLayoutKind.twentySixKey.highlightedSlots(on).isSubset(of: Set(slots)), label)
    }
    XCTAssertEqual(TouchKeyLayoutKind.twentySixKey.bottomRow(TouchKeyLayoutStyle(symbolFirst: true)).map(\.slot),
                   [.symbols, .layer, .space, .comma, .language, .returnKey])
    let swapped = TouchKeyLayoutStyle(fourteenKey: .emoji, nineKey: .commaPeriod, symbolFirst: true)
    let plain = TouchKeyLayoutStyle(fourteenKey: .emoji, nineKey: .commaPeriod)
    for kind in [TouchKeyLayoutKind.letterGrid, .nineKey] {
      XCTAssertEqual(kind.bottomRow(swapped), kind.bottomRow(plain), kind.rawValue)
      XCTAssertEqual(kind.highlightedSlots(swapped), kind.highlightedSlots(plain), kind.rawValue)
    }
  }

  /// 数字行开着时 26 键和 14/18 键的预览顶上画一排 1 到 0，9 键的预览不画；关着时三类都不画。
  func testSettingsPreviewDrawsTheNumberRowOnLetterKeyboardsOnly() {
    let on = TouchKeyLayoutStyle(numberRow: true)
    XCTAssertEqual(TouchKeyLayoutKind.allCases.map { $0.drawsNumberRow(on) }, [true, true, false])
    XCTAssertEqual(TouchKeyLayoutKind.allCases.map { $0.drawsNumberRow(TouchKeyLayoutStyle()) }, [false, false, false])
    XCTAssertEqual(TypingKeyHeatmap.numberRow.map(\.face), "1234567890".map(String.init))
    // 预览的 26 键字母行与热力图同一张表：数字行开着时热力图就是数字行加这三排再加底行。
    XCTAssertEqual(TypingKeyHeatmap.letterRows(for: on),
                   [TypingKeyHeatmap.numberRow] + TypingKeyHeatmap.twentySixKeyLetterRows + [TypingKeyHeatmap.letterRows(for: on).last!])
    XCTAssertEqual(TypingKeyHeatmap.twentySixKeyLetterRows.map { $0.map(\.face).joined() },
                   ["QWERTYUIOP", "ASDFGHJKL", "分词ZXCVBNM删除"])
  }

  // MARK: - 整块键盘预览（键盘页、皮肤页）

  /// 键盘页和皮肤各页的整块预览（`KeyboardSkinPreview`）按用户的样式画字母层底行，键序和份额与键盘同一张表。以前它写死 `123 | ， | 空格 | 中/英 | 换行`，用户的标准底行 `123 | 符 | 空格 | ， | 中 | 回车` 在预览里对不上（2026-10-29 iPhone XR）。
  func testSkinPreviewBottomRowFollowsEachStyle() {
    func slots(nineKey: Bool = false, letterGrid: Bool = false, _ style: TouchKeyLayoutStyle) -> [Slot] {
      KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: style, language: true).bottomRow.map(\.slot)
    }
    let twentySix: [TouchTwentySixKeyStyle: [Slot]] = [
      .standard: [.layer, .symbols, .space, .comma, .language, .returnKey],
      .commaPeriod: [.layer, .symbols, .comma, .space, .period, .language, .returnKey],
      .longSpace: [.layer, .comma, .space, .language, .returnKey],
      .emoji: [.layer, .emoji, .space, .comma, .language, .returnKey],
    ]
    for style in TouchTwentySixKeyStyle.allCases {
      let keyLayout = TouchKeyLayoutStyle(twentySixKey: style)
      let layer = KeyboardPreviewLetterLayer(nineKey: false, letterGrid: false, style: keyLayout, language: true)
      XCTAssertEqual(layer.kind, .twentySixKey, style.rawValue)
      XCTAssertEqual(layer.bottomRow.map(\.slot), twentySix[style], style.rawValue)
      XCTAssertEqual(layer.bottomRow, TouchBottomRowLayout.twentySixKey(style, globe: false), style.rawValue)
      XCTAssertFalse(layer.tallReturn, style.rawValue)
    }
    let letterGrid: [TouchFourteenKeyStyle: [Slot]] = [
      .standard: [.layer, .comma, .space, .period, .language, .returnKey],
      .longSpace: [.layer, .comma, .space, .language, .returnKey],
      .emoji: [.layer, .emoji, .space, .comma, .language, .returnKey],
    ]
    for style in TouchFourteenKeyStyle.allCases {
      let keyLayout = TouchKeyLayoutStyle(fourteenKey: style)
      let layer = KeyboardPreviewLetterLayer(nineKey: false, letterGrid: true, style: keyLayout, language: true)
      XCTAssertEqual(layer.kind, .letterGrid, style.rawValue)
      XCTAssertEqual(layer.bottomRow.map(\.slot), letterGrid[style], style.rawValue)
      XCTAssertEqual(layer.bottomRow, TouchBottomRowLayout.letterGrid(style, globe: false), style.rawValue)
    }
    let nine: [TouchNineKeyStyle: [Slot]] = [
      .standard: [.symbols, .layer, .space, .language, .returnKey],
      .commaPeriod: [.symbols, .layer, .comma, .space, .period, .language, .returnKey],
      .emoji: [.symbols, .layer, .emoji, .space, .language, .returnKey],
      .tallReturn: [.symbols, .layer, .space, .language, .returnKey],
    ]
    for style in TouchNineKeyStyle.allCases {
      let keyLayout = TouchKeyLayoutStyle(nineKey: style)
      // 九键优先于字母格：两个都传时画九键。
      let layer = KeyboardPreviewLetterLayer(nineKey: true, letterGrid: true, style: keyLayout, language: true)
      XCTAssertEqual(layer.kind, .nineKey, style.rawValue)
      XCTAssertEqual(layer.bottomRow.map(\.slot), nine[style], style.rawValue)
      XCTAssertEqual(layer.bottomRow, TouchBottomRowLayout.nineKey(style, globe: false), style.rawValue)
      XCTAssertEqual(layer.tallReturn, style == .tallReturn, style.rawValue)
      XCTAssertEqual(layer.nineKeySidebar, style == .tallReturn ? ["，", "。", "？", "！"] : ["，", "。", "？"], style.rawValue)
      XCTAssertEqual(layer.nineKeyControls, style == .tallReturn ? ["⌫", "分词"] : ["⌫", "分词", "！"], style.rawValue)
      // 换了字母层的表时，皮肤页这些预览与「按键布局」页一样画那张表的一屏，右侧大回车也不再接 ！。
      let custom = TouchKeyLayoutStyle(nineKey: style, nineKeySidebar: ["@", "#", "~", "%"])
      XCTAssertEqual(KeyboardPreviewLetterLayer(nineKey: true, letterGrid: false, style: custom, language: true).nineKeySidebar,
                     ["@", "#", "~"], style.rawValue)
    }
    // 每一类只读自己那一项样式：别的两类换了样式，底行不变。
    let mixed = TouchKeyLayoutStyle(twentySixKey: .emoji, fourteenKey: .longSpace, nineKey: .tallReturn)
    XCTAssertEqual(slots(mixed), twentySix[.emoji])
    XCTAssertEqual(slots(letterGrid: true, mixed), letterGrid[.longSpace])
    XCTAssertEqual(slots(nineKey: true, mixed), nine[.tallReturn])
    // 右侧大回车的中间几格合起来与九宫格同宽，两端 符 和回车各占一个侧列，与键盘的外框相同。
    let tall = KeyboardPreviewLetterLayer(nineKey: true, letterGrid: false, style: TouchKeyLayoutStyle(nineKey: .tallReturn), language: true)
    let side = TouchBottomRowLayout.nineKeySideWeight(globe: false)
    XCTAssertEqual(tall.bottomRow.first?.weight ?? 0, side, accuracy: 0.0001)
    XCTAssertEqual(tall.bottomRow.last?.weight ?? 0, side, accuracy: 0.0001)
    XCTAssertEqual(tall.bottomRow.reduce(0) { $0 + $1.weight }, TouchBottomRowLayout.nineKeyRowWeight(globe: false), accuracy: 0.0001)
  }

  /// 预览照键盘认「符」与「123」互换和「隐藏中/英键」：互换只动 26 键里带 符 的样式，隐藏中/英时它的份额并进空格；九键双拼的 0 跟在空格后面。
  func testSkinPreviewBottomRowFollowsSymbolFirstAndHiddenLanguage() {
    for style in TouchTwentySixKeyStyle.allCases {
      let on = KeyboardPreviewLetterLayer(nineKey: false, letterGrid: false,
                                          style: TouchKeyLayoutStyle(twentySixKey: style, symbolFirst: true), language: true)
      XCTAssertEqual(on.bottomRow, TouchBottomRowLayout.symbolFirst(TouchBottomRowLayout.twentySixKey(style, globe: false)), style.rawValue)
    }
    let swapped = KeyboardPreviewLetterLayer(nineKey: false, letterGrid: false, style: TouchKeyLayoutStyle(symbolFirst: true), language: true)
    XCTAssertEqual(swapped.bottomRow.map(\.slot), [.symbols, .layer, .space, .comma, .language, .returnKey])
    let longSpace = KeyboardPreviewLetterLayer(nineKey: false, letterGrid: false,
                                               style: TouchKeyLayoutStyle(twentySixKey: .longSpace, symbolFirst: true), language: true)
    XCTAssertEqual(longSpace.bottomRow.map(\.slot), [.layer, .comma, .space, .language, .returnKey])
    for (nineKey, letterGrid) in [(false, true), (true, false)] {
      let plain = KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: TouchKeyLayoutStyle(), language: true)
      let on = KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: TouchKeyLayoutStyle(symbolFirst: true), language: true)
      XCTAssertEqual(on.bottomRow, plain.bottomRow, "\(plain.kind)")
    }

    for (nineKey, letterGrid) in [(false, false), (false, true), (true, false)] {
      let style = TouchKeyLayoutStyle(twentySixKey: .commaPeriod, fourteenKey: .emoji, nineKey: .tallReturn)
      let shown = KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: style, language: true)
      let hidden = KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: style, language: false)
      let label = "\(shown.kind)"
      XCTAssertEqual(hidden.bottomRow.map(\.slot), shown.bottomRow.map(\.slot).filter { $0 != .language }, label)
      XCTAssertEqual(hidden.bottomRow.reduce(0) { $0 + $1.weight }, shown.bottomRow.reduce(0) { $0 + $1.weight }, accuracy: 0.0001, label)
      let space = { (layer: KeyboardPreviewLetterLayer) in layer.bottomRow.first { $0.slot == .space }?.weight ?? 0 }
      XCTAssertEqual(space(hidden), space(shown) + TouchBottomRowLayout.languageWeight, accuracy: 0.0001, label)
    }

    let zero = KeyboardPreviewLetterLayer(nineKey: true, letterGrid: false, style: TouchKeyLayoutStyle(), language: true, nineKeyZero: true)
    XCTAssertEqual(zero.bottomRow, TouchBottomRowLayout.nineKey(.standard, globe: false, zero: true))
    XCTAssertEqual(zero.bottomRow.map(\.slot), [.symbols, .layer, .space, .zero, .language, .returnKey])
    // 26 键和 14/18 键没有 0，传了也不画。
    let letters = KeyboardPreviewLetterLayer(nineKey: false, letterGrid: false, style: TouchKeyLayoutStyle(), language: true, nineKeyZero: true)
    XCTAssertFalse(letters.bottomRow.contains { $0.slot == .zero })
  }

  /// 「中/英键放在空格左边」（#10228）：键盘页和皮肤页的整块预览与键盘排同一行，三类都在最后一步把中/英挪到空格左边，九键双拼的 0 跟着空格一起右移；隐藏了中/英键时不变；换了左列的表时底行照样排。
  func testSkinPreviewBottomRowFollowsLanguageKeySide() {
    for (nineKey, letterGrid) in [(false, false), (false, true), (true, false)] {
      for style in [TouchKeyLayoutStyle(), TouchKeyLayoutStyle(twentySixKey: .commaPeriod, fourteenKey: .emoji, nineKey: .tallReturn)] {
        var left = style
        left.languageKeySide = .left
        let right = KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: style, language: true)
        let moved = KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: left, language: true)
        XCTAssertEqual(moved.bottomRow, TouchBottomRowLayout.languageBeforeSpace(right.bottomRow), "\(right.kind)")
        XCTAssertNotEqual(moved.bottomRow, right.bottomRow, "\(right.kind)")
        let hidden = KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: left, language: false)
        XCTAssertEqual(hidden.bottomRow,
                       KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: style, language: false).bottomRow,
                       "\(right.kind)")
      }
    }
    let left = TouchKeyLayoutStyle(languageKeySide: .left)
    XCTAssertEqual(KeyboardPreviewLetterLayer(nineKey: false, letterGrid: false, style: left, language: true).bottomRow.map(\.slot),
                   [.layer, .symbols, .language, .space, .comma, .returnKey])
    let zero = KeyboardPreviewLetterLayer(nineKey: true, letterGrid: false, style: left, language: true, nineKeyZero: true)
    XCTAssertEqual(zero.bottomRow.map(\.slot), [.symbols, .layer, .language, .space, .zero, .returnKey])
    let custom = TouchKeyLayoutStyle(nineKeySidebar: ["@", "#", "~"], languageKeySide: .left)
    let sidebar = KeyboardPreviewLetterLayer(nineKey: true, letterGrid: false, style: custom, language: true)
    XCTAssertEqual(sidebar.nineKeySidebar, ["@", "#", "~"])
    XCTAssertEqual(sidebar.bottomRow.map(\.slot), [.symbols, .layer, .language, .space, .returnKey])
  }

  /// 数字行开着时 26 键和 14/18 键的整块预览顶上画 1 到 0，九键不画；关着时都不画。
  func testSkinPreviewDrawsTheNumberRowOnLetterKeyboardsOnly() {
    for numberRow in [false, true] {
      let style = TouchKeyLayoutStyle(numberRow: numberRow)
      let drawn = [(false, false), (false, true), (true, false)].map { nineKey, letterGrid in
        KeyboardPreviewLetterLayer(nineKey: nineKey, letterGrid: letterGrid, style: style, language: true).numberRow
      }
      XCTAssertEqual(drawn, [numberRow, numberRow, false], "\(numberRow)")
    }
  }

  /// 「键盘」页和皮肤页的预览画用户正在用的那一类键盘（`KeyboardPreviewFace`），与 `TouchKeyLayoutKind.of`、Android `previewKind` 和鸿蒙相同：笔画和拼音九键、九键双拼一样画九宫格外框、读九键样式。以前 iOS 的「键盘」页给笔画画 26 键标准底行，皮肤页不论什么键盘都画 26 键。
  func testPreviewFaceFollowsTheCurrentKeyboardKind() {
    let layouts = TouchShuangpinLayoutPreference.Layout.allCases.flatMap { shuangpin in
      TouchWubiLayoutPreference.Layout.allCases.map { (shuangpin, $0) }
    }
    for scheme in ChineseInputScheme.allCases {
      for (shuangpin, wubi) in layouts {
        let face = KeyboardPreviewFace(scheme, shuangpinLayout: shuangpin, wubiLayout: wubi)
        let kind = TouchKeyLayoutKind.of(scheme, shuangpinLayout: shuangpin, wubiLayout: wubi)
        let label = "\(scheme) \(shuangpin) \(wubi)"
        let layer = KeyboardPreviewLetterLayer(nineKey: face.nineKey, letterGrid: face.letterGrid != nil,
                                               style: TouchKeyLayoutStyle(), language: true)
        // 不读样式的方案（手写、注音、日文、韩文等）画 26 键。
        XCTAssertEqual(layer.kind, kind ?? .twentySixKey, label)
        XCTAssertEqual(face.styled, kind != nil, label)
        XCTAssertEqual(face.letterGrid, LetterKeyGrid.of(scheme, shuangpinLayout: shuangpin, wubiLayout: wubi), label)
        XCTAssertEqual(face.nineKeyZero, scheme == .nineKeyShuangpin, label)
      }
    }

    // 九键一族都画九宫格外框；选了右侧大回车时笔画同样画竖跨两行的回车，与键盘的 `nineKeyFrame`、`tallReturn` 一致。
    for scheme in [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke] {
      let face = KeyboardPreviewFace(scheme, shuangpinLayout: .twentySixKey, wubiLayout: .twentySixKey)
      XCTAssertTrue(face.nineKey, "\(scheme)")
      XCTAssertTrue(face.styled, "\(scheme)")
      let tall = KeyboardPreviewLetterLayer(nineKey: face.nineKey, letterGrid: face.letterGrid != nil,
                                            style: TouchKeyLayoutStyle(nineKey: .tallReturn), language: true, nineKeyZero: face.nineKeyZero)
      XCTAssertEqual(tall.kind, .nineKey, "\(scheme)")
      XCTAssertTrue(tall.tallReturn, "\(scheme)")
      XCTAssertEqual(tall.nineKeyControls, ["⌫", "分词"], "\(scheme)")
    }

    // 14/18 键画各自的字母格，五笔 14 键第三排左端是 符。
    let quanpinEighteen = KeyboardPreviewFace(.eighteenKey, shuangpinLayout: .twentySixKey, wubiLayout: .twentySixKey)
    XCTAssertEqual(quanpinEighteen.letterGrid, .eighteenKey)
    XCTAssertTrue(quanpinEighteen.gridSplitsSyllables)
    XCTAssertEqual(KeyboardPreviewFace(.shuangpin, shuangpinLayout: .fourteenKey, wubiLayout: .twentySixKey).letterGrid, .fourteenKey)
    let wubiFourteen = KeyboardPreviewFace(.wubi, shuangpinLayout: .eighteenKey, wubiLayout: .fourteenKey)
    XCTAssertEqual(wubiFourteen.letterGrid, .fourteenKey)
    XCTAssertFalse(wubiFourteen.gridSplitsSyllables)
    XCTAssertFalse(wubiFourteen.nineKey)
  }

  /// 皮肤页从共享文档读方案和双拼、五笔布局（`KeyboardPreviewFace(in:)`），文档里选的是笔画时缩略图画九宫格外框，选双拼 18 键时画 18 键字母格。
  func testPreviewFaceReadsTheSharedDocument() {
    let stroke: [String: Any] = ["touch_keyboard_schemes": ["selected": "stroke", "enabled": ["quanpin", "stroke"]]]
    XCTAssertEqual(KeyboardPreviewFace(in: stroke), KeyboardPreviewFace(.stroke, shuangpinLayout: .twentySixKey, wubiLayout: .twentySixKey))
    XCTAssertTrue(KeyboardPreviewFace(in: stroke).nineKey)
    let eighteen: [String: Any] = [
      "touch_keyboard_schemes": ["selected": "xiaohe", "enabled": ["quanpin", "xiaohe"]],
      TouchShuangpinLayoutPreference.documentKey: TouchShuangpinLayoutPreference.Layout.eighteenKey.rawValue,
    ]
    XCTAssertEqual(KeyboardPreviewFace(in: eighteen).letterGrid, .eighteenKey)
    XCTAssertFalse(KeyboardPreviewFace(in: eighteen).nineKey)
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

  /// 「长空格」的字母层底行没有句号键，逗号与 Android、HarmonyOS 一样上滑或下滑输入句号：14 键和 18 键的逗号长按仍打开常用标点；26 键自定义了常用标点时滑动照样输入句号，长按打开那份菜单。别的样式的逗号不接滑动。
  func testLongSpaceCommaSwipesEitherWayForTheFullStop() throws {
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
          XCTAssertTrue(comma.cornerHintSwipesEitherWay, label)
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
    XCTAssertTrue(comma.swipesCornerHint, "the custom list keeps the swipe for the full stop")
    XCTAssertTrue(comma.cornerHintSwipesEitherWay, "upward or downward")
    XCTAssertFalse(comma.holdsCornerHint, "a long press opens the custom menu")
    controller.viewDidDisappear(false)
  }

  /// 长空格逗号的滑动判定（`SwipeHintPolicy.swipedEitherWay`），与 Android #10087 的 `ImeBottomRow.bindCommaSwipe` 相同：上滑、下滑都算，门槛是字母键滑动输入符号的 14 pt（严格大于），纵向要大于横向；点按时手指带一下不到门槛不算；按满长按时长还没滑出去的触摸归长按，之后再挪手指不起滑动。
  func testLongSpaceCommaSwipePolicy() {
    let down = CGPoint(x: 40, y: 100)
    let hold = KeyboardKeyButton.cornerHintHoldDelay
    let threshold = SwipeHintPolicy.threshold
    func swiped(dx: CGFloat = 0, dy: CGFloat, after elapsed: TimeInterval = 0.1) -> Bool {
      SwipeHintPolicy.swipedEitherWay(down: down, current: CGPoint(x: down.x + dx, y: down.y + dy), elapsed: elapsed, holdDelay: hold)
    }
    XCTAssertEqual(threshold, 14, "the same threshold as the letter-key swipe symbols")
    XCTAssertTrue(swiped(dy: threshold + 0.5), "a downward swipe")
    XCTAssertTrue(swiped(dy: -(threshold + 0.5)), "an upward swipe, the letter keys' default direction")
    XCTAssertFalse(swiped(dy: threshold), "exactly on the threshold is still a tap")
    XCTAssertFalse(swiped(dy: -threshold), "upward too")
    XCTAssertFalse(swiped(dx: 2, dy: -8), "tap drift below the threshold does not type the full stop")
    XCTAssertFalse(swiped(dx: 30, dy: 20), "a mostly sideways move is not a swipe")
    XCTAssertFalse(swiped(dx: -30, dy: -20), "nor to the other side")
    XCTAssertTrue(swiped(dy: -20, after: hold - 0.01), "a swipe just before the long-press delay still counts")
    XCTAssertFalse(swiped(dy: -20, after: hold), "once the long-press delay has passed the touch belongs to the long press")
    XCTAssertFalse(swiped(dy: 20, after: hold + 1), "in either direction")
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

  /// 右侧大回车中列的宽度约束按当时的键距扣键距，键距变了要重排底行。键盘里调完高度点「完成」、共享文档却没写成时，键盘按文档对齐 App Group，键距可能当场变了（这里从 6pt 变成文档里的 3pt），此后没有别的重排：123、地球键、中/英 仍按 6pt 扣键距，比标准底行窄约 1.6pt，直到下一次重排。用例让偏好文件不可改写来模拟写失败。
  func testTallReturnFollowsTheKeySpacingTheFailedGeometryCommitFallsBackTo() async throws {
    let groupKeySpacing = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.keySpacingKey)
    let groupHeight = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.heightAdjustmentKey)
    let documentKeySpacing = MetasequoiaInputSessionBridge.loadSharedPreferences()?["touch_key_spacing_tenths"]
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_key_spacing_tenths"] = documentKeySpacing }
      KeyboardLayoutPreference.defaults.set(groupKeySpacing, forKey: KeyboardLayoutPreference.keySpacingKey)
      KeyboardLayoutPreference.defaults.set(groupHeight, forKey: KeyboardLayoutPreference.heightAdjustmentKey)
    }
    select(.nineKey)
    // 先量 3pt 键距下标准九键底行的键宽，再让同一个键盘当场换成 6pt 键距的右侧大回车。
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(keySpacing: 3))
    let controller = keyboard()
    let standard = try standardNineKeyWidths(controller, "3pt")
    let tall = try button("tallReturnKey", in: controller)
    XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(keySpacing: 6))
    try await restyle(controller, TouchKeyLayoutStyle(nineKey: .tallReturn), "6pt") { self.shown(tall) }
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 6)
    let layer = try view("layoutToggleButton", in: controller)
    let standardLayer = try XCTUnwrap(standard["layoutToggleButton"])
    XCTAssertGreaterThan(abs(layer.bounds.width - standardLayer), 1, "at 6pt 123 is not as wide as at 3pt")

    // 文档里的键距改成 3pt，App Group 和键盘仍是 6pt；偏好文件设成不可改写，键盘调整的「确认」写文档失败，键盘按文档对齐成 3pt。
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_key_spacing_tenths"] = 30 })
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 6)
    let document = URL(fileURLWithPath: MetasequoiaInputSessionBridge.sharedStateDirectory)
      .appendingPathComponent("preferences.json").path
    XCTAssertEqual(chflags(document, UInt32(UF_IMMUTABLE)), 0, "the preferences file can be locked")
    defer { chflags(document, 0) }
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "moreCard-键盘调整" } as? UIControl)
      .sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardAdjustConfirm" } as? UIControl)
      .sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 3, "the failed commit falls back to the document's key spacing")
    try layoutAtRequestedHeight(controller)
    try assertTallReturnFrame(controller, digits: false, standard: standard, "after the failed commit")
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
    // 标准九键的数字层：符 在底行最左、与字母层同一格（#10061），符号栏里不再放第二个 符。
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    var controller = keyboard()
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertFalse(shown(try button("numberPadSymbolPanelKey", in: controller)))
    XCTAssertTrue(shown(try button("symbolPanelKey", in: controller)))
    try assertBottomRow(controller, matches: TouchBottomRowLayout.nineKeyDigits(globe: controller.needsInputModeSwitchKey),
                        "standard nine-key digits")
    controller.viewDidDisappear(false)
    // 借来的数字九宫格：符 仍在符号栏第三排。
    select(.quanpin)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    controller = keyboard()
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("numberPadSymbolPanelKey", in: controller)))
    XCTAssertEqual(try button("numberPadSymbolPanelKey", in: controller).bounds.height,
                   try button("nineKeyDelete", in: controller).bounds.height, accuracy: 0.5)
    XCTAssertFalse(shown(try button("symbolPanelKey", in: controller)))
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

  // MARK: - 「符」与「123」互换、数字键盘空格位置（#10056）

  /// 两项的读法：互换不是布尔值时按关；空格位置缺键、写 `platform_default` 或不认识时按 iOS 原来的 `afterZero`。线上写法、选项名和设置页文字与 client-core、Android、鸿蒙逐字相同。
  func testSymbolFirstAndNumberPadSpaceParsing() {
    XCTAssertEqual(TouchKeyLayoutStyle(in: ["touch_twenty_six_symbol_first": true, "touch_number_pad_space": "before_zero"]),
                   TouchKeyLayoutStyle(symbolFirst: true, numberPadSpace: .beforeZero))
    XCTAssertEqual(TouchKeyLayoutStyle(in: ["touch_number_pad_space": "right_column"]).numberPadSpace, .rightColumn)
    for unknown: Any in ["platform_default", "space_in_grid", 3] {
      XCTAssertEqual(TouchKeyLayoutStyle(in: ["touch_number_pad_space": unknown]).numberPadSpace, .afterZero, "\(unknown)")
    }
    XCTAssertFalse(TouchKeyLayoutStyle(in: ["touch_twenty_six_symbol_first": "true"]).symbolFirst)
    XCTAssertEqual(TouchKeyLayoutStyle().numberPadSpace, .afterZero, "iOS keeps its own number pad by default")
    XCTAssertEqual(TouchNumberPadSpace.allCases.map(\.rawValue), ["right_column", "after_zero", "before_zero"])
    XCTAssertEqual(TouchNumberPadSpace.allCases.map(\.title), ["空格在右列", "空格在 0 右边", "空格在 0 左边"])
    XCTAssertEqual(TouchKeyLayoutOffering.symbolFirstTitle, "26 键「符」与「123」互换")
    XCTAssertEqual(TouchKeyLayoutOffering.symbolFirstSubtitle, "符 排在 123 左边；长空格和表情键样式没有 符，不受影响。")
    XCTAssertEqual(TouchKeyLayoutOffering.numberPadTitle, "数字键盘")
    XCTAssertEqual(TouchKeyLayoutOffering.numberPadFooter, "九宫格数字键盘里空格的位置；右侧大回车不受影响。")
  }

  /// 保存：互换开关关着时不留在文档里；数字键盘写用户点的那一种，包括与 iOS 出厂排法相同的 `after_zero`（明确选过，同步到别的设备也按它画）。
  func testSymbolFirstAndNumberPadSpaceSaving() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(TouchKeyLayoutStyle.save(symbolFirst: true, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(numberPadSpace: .afterZero, stateRoot: state))
    var document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_twenty_six_symbol_first"] as? Bool, true)
    XCTAssertEqual(document["touch_number_pad_space"] as? String, "after_zero")
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.numberPadSpace, .afterZero)
    XCTAssertTrue(TouchKeyLayoutStyle.mirrored.symbolFirst)
    XCTAssertTrue(TouchKeyLayoutStyle.save(symbolFirst: false, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(numberPadSpace: .rightColumn, stateRoot: state))
    document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertNil(document["touch_twenty_six_symbol_first"], "the default is not written")
    XCTAssertEqual(TouchKeyLayoutStyle(in: document), TouchKeyLayoutStyle(numberPadSpace: .rightColumn))
  }

  /// 表：互换只把 符 挪到 123 左边、份额不变，没有 符 的样式不变；数字层三种空格位置的键序和份额，整行总份额不变。
  func testSymbolFirstAndNumberPadSpaceTables() {
    for style in TouchTwentySixKeyStyle.allCases {
      for globe in [false, true] {
        let row = TouchBottomRowLayout.twentySixKey(style, globe: globe)
        let swapped = TouchBottomRowLayout.symbolFirst(row)
        XCTAssertEqual(Set(swapped), Set(row), style.rawValue)
        XCTAssertEqual(swapped.first?.slot, [.standard, .commaPeriod].contains(style) ? .symbols : .layer, style.rawValue)
        if [.standard, .commaPeriod].contains(style) {
          XCTAssertEqual(swapped[1].slot, .layer, style.rawValue)
          if globe { XCTAssertEqual(swapped[2].slot, .globe, "the globe stays after 123") }
        } else {
          XCTAssertEqual(swapped, row, "\(style.rawValue) has no 符")
        }
      }
    }
    XCTAssertEqual(TouchBottomRowLayout.nineKeyDigits(globe: false), TouchBottomRowLayout.nineKeyDigits(globe: false, space: .afterZero))
    for zero in [false, true] {
      for language in [true, false] {
        let after = TouchBottomRowLayout.nineKeyDigits(globe: false, zero: zero, language: language, space: .afterZero)
        let before = TouchBottomRowLayout.nineKeyDigits(globe: false, zero: zero, language: language, space: .beforeZero)
        let column = TouchBottomRowLayout.nineKeyDigits(globe: false, zero: zero, language: language, space: .rightColumn)
        let total = { (keys: [TouchBottomRowLayout.Key]) in keys.reduce(0) { $0 + $1.weight } }
        XCTAssertEqual(total(before), total(after), accuracy: 1e-9)
        XCTAssertEqual(total(column), total(after), accuracy: 1e-9)
        let wide = after.first { $0.slot == .zero }!.weight
        XCTAssertEqual(before.map(\.slot), after.map(\.slot).map { $0 == .zero ? .space : $0 == .space ? .zero : $0 })
        XCTAssertEqual(before.first { $0.slot == .space }!.weight, wide, accuracy: 1e-9)
        XCTAssertEqual(before.first { $0.slot == .zero }!.weight, TouchBottomRowLayout.zeroWeight, accuracy: 1e-9)
        XCTAssertFalse(column.contains { $0.slot == .space })
        XCTAssertEqual(column.first { $0.slot == .zero }!.weight, wide + TouchBottomRowLayout.zeroWeight, accuracy: 1e-9)
      }
    }
    XCTAssertEqual(TouchBottomRowLayout.borrowedNumberPad(globe: true, space: .beforeZero).map(\.slot),
                   [.layer, .globe, .space, .zero, .language, .returnKey])
    let column = TouchBottomRowLayout.borrowedNumberPad(globe: false, language: false, space: .rightColumn)
    XCTAssertEqual(column.map(\.slot), [.layer, .zero, .returnKey])
    XCTAssertEqual(column[1].weight, 6.3, accuracy: 1e-9, "0 takes the space's, its own and the hidden 中/英's shares")
  }

  /// 真键盘：互换后中文 26 键（标准和逗号句号在空格两侧）与英文 26 键的底行以 符 打头，宽度与不互换时相同；从它进的数字九宫格按九键一族排（符 | 返回），符号栏里不再放第二个 符。
  func testSymbolFirstSwapsTheTwentySixKeyRowAndItsNumberPad() throws {
    select(.quanpin)
    for style in [TouchTwentySixKeyStyle.standard, .commaPeriod] {
      XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: style)))
      var controller = keyboard()
      let plain = visibleBottomRow(controller)
      controller.viewDidDisappear(false)
      XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: style, symbolFirst: true)))
      controller = keyboard()
      let swapped = visibleBottomRow(controller)
      XCTAssertEqual(swapped.map(\.slot), TouchBottomRowLayout.symbolFirst(TouchBottomRowLayout.twentySixKey(
        style, globe: controller.needsInputModeSwitchKey)).map(\.slot), style.rawValue)
      for key in plain {
        let width = try XCTUnwrap(swapped.first { $0.slot == key.slot }).width
        XCTAssertEqual(width, key.width, accuracy: 0.5, "\(style.rawValue) \(key.slot.rawValue) keeps its width")
      }
      attach(controller, "26 键 \(style.title) 符与123互换")
      // 数字九宫格：符 和返回键在字母层 符 和 123 那两格。
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      XCTAssertEqual(try button("nineKey7", in: controller).configuration?.title, "7")
      try assertBottomRow(controller, matches: TouchBottomRowLayout.nineKeyDigits(globe: controller.needsInputModeSwitchKey),
                          "\(style.rawValue) number pad")
      XCTAssertFalse(shown(try button("numberPadSymbolPanelKey", in: controller)), "\(style.rawValue) one 符 only")
      controller.viewDidDisappear(false)
    }
    // 表情键样式没有 符，互换不起作用，借来的数字九宫格照旧以返回打头。
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .emoji, symbolFirst: true)))
    var controller = keyboard()
    XCTAssertEqual(visibleBottomRow(controller).first?.slot, .layer)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(visibleBottomRow(controller).first?.slot, .layer)
    XCTAssertTrue(shown(try button("numberPadSymbolPanelKey", in: controller)))
    controller.viewDidDisappear(false)
    // 英文 26 键同样互换。
    XCTAssertTrue(write(TouchKeyLayoutStyle(symbolFirst: true)))
    controller = keyboard()
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                   [.symbols, .layer] + (controller.needsInputModeSwitchKey ? [.globe] : []) + [.period, .space, .comma, .language, .returnKey])
    // 英文 26 键的 123 打开数字九宫格，与中文互换过的 26 键一样排成 符 | 返回，返回键在字母层 123 那一格；#+= 层同样（😀 或 符号 | 返回）。
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(try button("nineKey7", in: controller).configuration?.title, "7")
    try assertBottomRow(controller, matches: TouchBottomRowLayout.nineKeyDigits(globe: controller.needsInputModeSwitchKey),
                        "English number pad")
    XCTAssertFalse(shown(try button("numberPadSymbolPanelKey", in: controller)), "English number pad: one 符 only")
    try button("numberPadMoreSymbols", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    let moreRow = visibleBottomRow(controller).map(\.slot)
    XCTAssertEqual(moreRow.firstIndex(of: .layer), 1, "English #+= layer keeps 返回 where 123 was")
    XCTAssertTrue(moreRow.first == .emoji || moreRow.first == .symbols, "English #+= layer: \(moreRow)")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.viewDidDisappear(false)
    // 长空格样式没有 符，互换不起作用，它借来的数字九宫格照旧以返回打头。
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace, symbolFirst: true)))
    controller = keyboard()
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("nineKey1", in: controller)), "long space opens the number pad")
    XCTAssertEqual(visibleBottomRow(controller).first?.slot, .layer, "long space number pad")
    controller.viewDidDisappear(false)
    // 九键底行本来以 符 打头，不受影响。
    select(.nineKey)
    controller = keyboard()
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                   TouchBottomRowLayout.nineKey(.standard, globe: controller.needsInputModeSwitchKey).map(\.slot))
    controller.viewDidDisappear(false)
  }

  /// 真键盘：拼音九键自己的数字层和全拼借来的数字九宫格，三种空格位置的底行键序、右列空格的显隐和 ！ 的让位；缺省（`afterZero`）就是原来的数字层。
  func testNumberPadSpacePositionsOnTheRealKeyboard() throws {
    for scheme in [ChineseInputScheme.nineKey, .quanpin] {
      select(scheme)
      for space in TouchNumberPadSpace.allCases {
        XCTAssertTrue(write(TouchKeyLayoutStyle(numberPadSpace: space)))
        let controller = keyboard()
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        try layoutAtRequestedHeight(controller)
        let label = "\(scheme.rawValue) \(space.rawValue)"
        let globe = controller.needsInputModeSwitchKey
        if scheme == .nineKey {
          try assertBottomRow(controller, matches: TouchBottomRowLayout.nineKeyDigits(globe: globe, space: space), label)
        } else {
          let middle: [Slot] = switch space {
          case .afterZero: [.zero, .space]
          case .beforeZero: [.space, .zero]
          case .rightColumn: [.zero]
          }
          XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                         [.layer] + (globe ? [.globe] : []) + middle + [.language, .returnKey], label)
        }
        let columnSpace = try button("numberPadColumnSpaceKey", in: controller)
        let closing = try button("nineKeyClosingMark", in: controller)
        XCTAssertEqual(shown(columnSpace), space == .rightColumn, label)
        XCTAssertEqual(closing.alpha, space == .rightColumn ? 0 : 1, label)
        XCTAssertEqual(closing.isAccessibilityElement, space != .rightColumn, label)
        if space == .rightColumn {
          XCTAssertEqual(columnSpace.convert(columnSpace.bounds, to: controller.view),
                         closing.convert(closing.bounds, to: controller.view), "\(label) the space sits on the third cell")
        }
        attach(controller, "数字键盘 \(label)")
        // 回到字母层：底行的空格回来，右列空格收起。
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        try layoutAtRequestedHeight(controller)
        XCTAssertFalse(shown(columnSpace), label)
        XCTAssertTrue(shown(try button("spaceKey", in: controller)), label)
        XCTAssertEqual(closing.alpha, 1, label)
        controller.viewDidDisappear(false)
      }
    }
  }

  /// 数字层右列的空格叠在按键列上、不在任何一行里：面板盖住按键时它要和各行一起藏起来（不画、不接触摸、VoiceOver 读不到），关掉面板再回来。
  func testNumberPadColumnSpaceIsCoveredByAPanel() throws {
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle(numberPadSpace: .rightColumn)))
    let controller = keyboard()
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    let columnSpace = try button("numberPadColumnSpaceKey", in: controller)
    XCTAssertTrue(shown(columnSpace))

    try button("emojiShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardEmojiPicker" })
    XCTAssertEqual(columnSpace.alpha, 0, "the space does not show through the panel")
    XCTAssertFalse(columnSpace.isUserInteractionEnabled)
    XCTAssertTrue(columnSpace.accessibilityElementsHidden, "VoiceOver cannot reach the covered space")

    try button("dismissShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(columnSpace.alpha, 1)
    XCTAssertTrue(columnSpace.isUserInteractionEnabled)
    XCTAssertFalse(columnSpace.accessibilityElementsHidden)
    controller.viewDidDisappear(false)
  }

  /// 统计热力图跟着互换画：26 键底行的 符 在 123 左边。
  func testHeatmapFollowsSymbolFirst() {
    let rows = TypingKeyHeatmap.letterRows(for: TouchKeyLayoutStyle(symbolFirst: true))
    XCTAssertEqual(rows.last?.prefix(2).map(\.id), [TypingKeyID.symbol, TypingKeyID.layer])
    XCTAssertEqual(TypingKeyHeatmap.letterRows(for: TouchKeyLayoutStyle()).last?.prefix(2).map(\.id),
                   [TypingKeyID.layer, TypingKeyID.symbol])
  }

  // MARK: - 左侧符号栏

  /// 两张符号表的读法：逐项丢坏值（不是字符串的、空串、带空白或控制字符的、超过四个码位的和重复的），不是数组时是出厂表；镜像与文档一致。
  func testSidebarTablesReadItemByItem() {
    let document: [String: Any] = [
      TouchKeyLayoutStyle.nineKeySidebarDocumentKey: ["、", 3, " ； ", "、", "", "a b", "👨‍👩‍👧", NSNull()] as [Any],
      TouchKeyLayoutStyle.digitSidebarDocumentKey: "= %",
    ]
    let style = TouchKeyLayoutStyle(in: document)
    XCTAssertEqual(style.nineKeySidebar, ["、", "；"])
    XCTAssertEqual(style.digitSidebar, [], "a value that is not an array reads as the factory table")
    XCTAssertEqual(style.sidebarSymbols(.letter), ["、", "；"])
    XCTAssertEqual(style.sidebarSymbols(.digit), ["+", "-", "*", "/"])
    XCTAssertEqual(TouchKeyLayoutStyle(in: [:]).sidebarSymbols(.letter), ["，", "。", "？"])
    TouchKeyLayoutStyle.mirror(document)
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.nineKeySidebar, ["、", "；"])
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.digitSidebar, [])
  }

  /// 编辑页保存只写这一张表，文档接受了才写镜像；与 iOS 出厂表逐项相同（包括「恢复默认」）时写空数组，client-core 从文档里省掉它。
  func testSavingASidebarTableWritesOneFieldAndTheMirror() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(TouchKeyLayoutStyle.save(sidebar: ["、", "……", "（）"], table: .letter, stateRoot: state))
    var document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document[TouchKeyLayoutStyle.nineKeySidebarDocumentKey] as? [String], ["、", "……", "（）"])
    XCTAssertNil(document[TouchKeyLayoutStyle.digitSidebarDocumentKey], "the other table stays out of the document")
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.nineKeySidebar, ["、", "……", "（）"])
    XCTAssertTrue(TouchKeyLayoutStyle.save(sidebar: ["=", "%"], table: .digit, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(sidebar: ["，", "。", "？"], table: .letter, stateRoot: state))
    document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertTrue((document[TouchKeyLayoutStyle.nineKeySidebarDocumentKey] as? [Any])?.isEmpty ?? true,
                  "the factory table is written as an empty table")
    XCTAssertEqual(document[TouchKeyLayoutStyle.digitSidebarDocumentKey] as? [String], ["=", "%"])
    XCTAssertEqual(TouchKeyLayoutStyle(in: document), TouchKeyLayoutStyle(digitSidebar: ["=", "%"]))
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.nineKeySidebar, [])
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.digitSidebar, ["=", "%"])
    XCTAssertTrue(TouchKeyLayoutStyle.save(sidebar: ["+", "-", "*", "/"], table: .digit, stateRoot: state))
    XCTAssertEqual(TouchKeyLayoutStyle(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)),
                   TouchKeyLayoutStyle())
  }

  /// 9 键预览的符号栏：出厂表与键盘、热力图画的一样（右侧大回车接 ！），不描边；自定义表画一屏三项、每一格描边，右侧大回车也不接 ！。数字表不进 9 键预览，热力图始终画出厂的 ，。？。
  func testNineKeyPreviewDrawsTheLetterTable() {
    XCTAssertEqual(TouchKeyLayoutKind.previewSidebar(TouchKeyLayoutStyle()), ["，", "。", "？"])
    XCTAssertEqual(TouchKeyLayoutKind.previewSidebar(TouchKeyLayoutStyle()).joined(),
                   TypingKeyHeatmap.nineKeySidebar(for: .standard).face)
    XCTAssertEqual(TouchKeyLayoutKind.previewSidebar(TouchKeyLayoutStyle(nineKey: .tallReturn)).joined(),
                   TypingKeyHeatmap.nineKeySidebar(for: .tallReturn).face)
    let five = ["、", "；", "：", "……", ":-)"]
    XCTAssertEqual(TouchKeyLayoutKind.previewSidebar(TouchKeyLayoutStyle(nineKeySidebar: five)), ["、", "；", "："])
    XCTAssertEqual(TouchKeyLayoutKind.previewSidebar(TouchKeyLayoutStyle(nineKey: .tallReturn, nineKeySidebar: ["、"])), ["、"])
    XCTAssertFalse(TouchKeyLayoutKind.nineKey.highlightsSidebar(TouchKeyLayoutStyle()))
    XCTAssertFalse(TouchKeyLayoutKind.nineKey.highlightsSidebar(TouchKeyLayoutStyle(digitSidebar: ["="])))
    XCTAssertTrue(TouchKeyLayoutKind.nineKey.highlightsSidebar(TouchKeyLayoutStyle(nineKeySidebar: ["、"])))
    XCTAssertFalse(TouchKeyLayoutKind.twentySixKey.highlightsSidebar(TouchKeyLayoutStyle(nineKeySidebar: ["、"])))
    XCTAssertEqual(TouchKeyLayoutKind.nineKey.highlightedSlots(TouchKeyLayoutStyle(nineKeySidebar: ["、"])), [])
    XCTAssertEqual(TypingKeyHeatmap.cells(nineKey: true, style: TouchKeyLayoutStyle(nineKeySidebar: five)).first?.face, "，。？")
  }

  /// 出厂表时左列仍是原来的 ，。？ 三格：贴满符号栏、等高平分，与改版前逐点相同，自定义表的滚动列藏着。当场换成自定义表再换回出厂表，三格的帧和原来一样。
  func testFactorySidebarKeepsItsFrames() async throws {
    select(.nineKey)
    let controller = keyboard()
    let sidebar = try view("nineKeySidebar", in: controller)
    let custom = try view("nineKeyCustomSidebar", in: controller)
    let factory = sidebarMarks(sidebar)
    XCTAssertEqual(factory.map(\.title), ["，", "。", "？"])
    for (index, mark) in factory.enumerated() {
      XCTAssertEqual(mark.frame.height, sidebar.bounds.height / 3, accuracy: 0.5, mark.title)
      XCTAssertEqual(mark.frame.minY, sidebar.bounds.height / 3 * CGFloat(index), accuracy: 0.5, mark.title)
      XCTAssertEqual(mark.frame.minX, 0, accuracy: 0.5, mark.title)
      XCTAssertEqual(mark.frame.width, sidebar.bounds.width, accuracy: 0.5, mark.title)
    }
    XCTAssertFalse(shown(custom))
    try await restyle(controller, TouchKeyLayoutStyle(nineKeySidebar: ["、", "；"]), "custom") { self.shown(custom) }
    XCTAssertEqual(sidebarMarks(sidebar).map(\.title), ["、", "；"])
    try await restyle(controller, TouchKeyLayoutStyle(), "factory again") { !self.shown(custom) }
    let again = sidebarMarks(sidebar)
    XCTAssertEqual(again.map(\.title), factory.map(\.title))
    for (drawn, original) in zip(again, factory) {
      XCTAssertEqual(drawn.frame.minY, original.frame.minY, accuracy: 0.5, drawn.title)
      XCTAssertEqual(drawn.frame.height, original.frame.height, accuracy: 0.5, drawn.title)
      XCTAssertEqual(drawn.frame.width, original.frame.width, accuracy: 0.5, drawn.title)
    }
    controller.viewDidDisappear(false)
  }

  /// 自定义字母表：一屏最多三项、每项等高且不低于 28pt，多的上下滚动，从符号起滑交给滚动；少于三项时按项数均分左列。九宫格和右列不动。
  func testCustomLetterSidebarScrollsThreeAScreen() throws {
    select(.nineKey)
    let five = ["、", "；", "：", "……", ":-)"]
    XCTAssertTrue(write(TouchKeyLayoutStyle(nineKeySidebar: five)))
    let controller = keyboard()
    let scroll = try XCTUnwrap(try view("nineKeyCustomSidebar", in: controller) as? NineKeySidebarScrollView)
    let sidebar = try view("nineKeySidebar", in: controller)
    XCTAssertTrue(shown(scroll))
    XCTAssertEqual(scroll.frame.size, sidebar.bounds.size)
    XCTAssertEqual(scroll.buttons.map { $0.configuration?.title }, five)
    XCTAssertEqual(scroll.visibleRows, 3)
    for (index, key) in scroll.buttons.enumerated() {
      XCTAssertEqual(key.frame.height, scroll.bounds.height / 3, accuracy: 0.5, five[index])
      XCTAssertGreaterThanOrEqual(key.frame.height, NineKeySidebar.minRowHeight, five[index])
      XCTAssertEqual(key.frame.width, scroll.bounds.width, accuracy: 0.5, five[index])
      XCTAssertEqual(key.accessibilityIdentifier, "nineKeySidebarSymbol\(index)")
      XCTAssertEqual(key.accessibilityLabel, "符号 \(five[index])")
      XCTAssertTrue(scroll.touchesShouldCancel(in: key), "a drag that starts on a symbol scrolls")
    }
    XCTAssertGreaterThan(scroll.contentSize.height, scroll.bounds.height)
    XCTAssertFalse(descendants(sidebar).contains { ($0 as? UIButton)?.configuration?.title == "，" && shown($0) })
    // 右列的 ⌫、分词、！ 仍与九宫格三行对齐。
    for (index, identifier) in ["nineKeyDelete", "nineKeyMiddleKey", "nineKeyClosingMark"].enumerated() {
      let key = try button(identifier, in: controller)
      let row = try button("nineKey\(index * 3 + 3)", in: controller)
      XCTAssertEqual(key.convert(key.bounds, to: controller.view).midY, row.convert(row.bounds, to: controller.view).midY,
                     accuracy: 0.5, identifier)
    }
    controller.viewDidDisappear(false)

    XCTAssertTrue(write(TouchKeyLayoutStyle(nineKeySidebar: ["、", "；"])))
    let short = keyboard()
    let shortScroll = try XCTUnwrap(try view("nineKeyCustomSidebar", in: short) as? NineKeySidebarScrollView)
    XCTAssertEqual(shortScroll.visibleRows, 2)
    for key in shortScroll.buttons { XCTAssertEqual(key.frame.height, shortScroll.bounds.height / 2, accuracy: 0.5) }
    XCTAssertEqual(shortScroll.contentSize.height, shortScroll.bounds.height, accuracy: 0.5)
    short.viewDidDisappear(false)
  }

  /// 右侧大回车加自定义字母表：符号栏照用户的表画，末尾不接 ！；回车照样伸下来占右列第三格。
  func testTallReturnWithACustomTableAppendsNoExclamation() throws {
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle(nineKey: .tallReturn, nineKeySidebar: ["、", "；"])))
    let controller = keyboard()
    XCTAssertEqual(sidebarMarks(try view("nineKeySidebar", in: controller)).map(\.title), ["、", "；"])
    XCTAssertFalse(shown(try button("nineKeySidebarClosingMark", in: controller)))
    XCTAssertTrue(shown(try button("tallReturnKey", in: controller)))
    controller.viewDidDisappear(false)
  }

  /// 数字层的自定义表：九键自己的数字层和全拼借来的数字九宫格都换成用户的表，每项与右列的 ⌫ 同高，#+= 仍是同一个滚动栈的最后一项；当场换回出厂表时又是原来的四个运算符。
  func testDigitSidebarFollowsTheDigitTable() async throws {
    for scheme in [ChineseInputScheme.nineKey, .quanpin] {
      let label = scheme.rawValue
      select(scheme)
      XCTAssertTrue(write(TouchKeyLayoutStyle(digitSidebar: ["=", "%", "(", ")", "（）"])), label)
      let controller = keyboard()
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      let scroll = try view("numberPadOperatorScroll", in: controller)
      let stack = try XCTUnwrap(scroll.subviews.compactMap { $0 as? UIStackView }.first, label)
      func titles() -> [String?] { stack.arrangedSubviews.map { ($0 as? UIButton)?.configuration?.title } }
      XCTAssertEqual(titles(), ["=", "%", "(", ")", "（）", "#+="], label)
      XCTAssertEqual(stack.arrangedSubviews.last?.accessibilityIdentifier, "numberPadMoreSymbols", label)
      let delete = try button("nineKeyDelete", in: controller)
      for key in stack.arrangedSubviews { XCTAssertEqual(key.bounds.height, delete.bounds.height, accuracy: 0.5, label) }
      XCTAssertEqual(try button("numberPadSidebarSymbol0", in: controller).accessibilityLabel, "符号 =", label)
      XCTAssertTrue(shown(try button("numberPadSidebarSymbol0", in: controller)), label)
      try await restyle(controller, TouchKeyLayoutStyle(), label) { titles().first == "+" }
      XCTAssertEqual(titles(), ["+", "-", "*", "/", "#+="], label)
      XCTAssertNotNil(descendants(stack).first { $0.accessibilityLabel == "运算符 +" }, label)
      controller.viewDidDisappear(false)
    }
  }

  /// 新改为数字九宫格的键面上，数字页的键照数字页的规则上屏：英文按字面写出数字、小数点、运算符和 ASCII 的 !；韩文正在拼的音节原样上屏后接上数字；本地模式里 Engine 当拼写的数字交给会话，不写进文档，也不结束本地模式。
  func testNumberPadTypesInTheNewlyCoveredLayouts() throws {
    let document = SidebarDocument()
    try attach(document)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))

    // 英文：从全拼切到英文后按 123。
    select(.quanpin)
    var controller = keyboard()
    if try button("bottomLanguageKey", in: controller).accessibilityValue != "英文输入" {
      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    }
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    for id in ["nineKey1", "nineKeyMiddleKey", "nineKey5", "nineKeyZero"] {
      try button(id, in: controller).sendActions(for: .primaryActionTriggered)
    }
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "运算符 +" && shown($0) } as? UIButton)
      .sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "1.50+")
    let closing = try button("nineKeyClosingMark", in: controller)
    XCTAssertEqual(closing.configuration?.title, "!")
    closing.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(document.text.hasPrefix("1.50+!"), document.text)
    XCTAssertFalse(document.text.contains("！"), document.text)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.viewDidDisappear(false)

    // 韩文：ㅎ ㅏ 拼成「하」，在数字九宫格上按 1 先把它原样上屏再写 1。
    document.text = ""
    select(.korean)
    if InputSchemePreference.scheme == .korean {
      controller = keyboard()
      if try button("bottomLanguageKey", in: controller).accessibilityValue == "英文输入" {
        try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      }
      for letter in ["G", "K"] {
        let label = "字母 \(DubeolsikKeyLayout.keycap(for: letter, shifted: false) ?? letter)"
        try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == label } as? UIButton, label)
          .sendActions(for: .primaryActionTriggered)
      }
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "한")
      try button("nineKey1", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(document.text, "하1")
      XCTAssertEqual(try button("nineKeyClosingMark", in: controller).configuration?.title, "!")
      controller.viewDidDisappear(false)
    }

    // 本地模式：U 模式把数字当码点的拼写，交给会话，不写进文档，数字九宫格还在。
    document.text = ""
    select(.quanpin)
    controller = keyboard()
    if try button("bottomLanguageKey", in: controller).accessibilityValue == "英文输入" {
      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    }
    controller.openLocalInputMode("U")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "ABC")
    for id in ["nineKey4", "nineKeyZero"] {
      try button(id, in: controller).sendActions(for: .primaryActionTriggered)
    }
    XCTAssertEqual(document.text, "", "the code point digits stay in the composition")
    let codePoint = try button("preeditButton", in: controller).accessibilityValue ?? ""
    XCTAssertTrue(codePoint.contains("40"), codePoint)
    XCTAssertFalse(try button("exitLocalModeButton", in: controller).isHidden)
    XCTAssertTrue(shown(try button("nineKey4", in: controller)))
    try button("exitLocalModeButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.viewDidDisappear(false)
  }

  /// 自定义表的项按字面上屏，不经 Engine 的标点路由：先结束组字，表项正好是一对成对符号时光标退到两者中间，其余的项不动光标。字母层和数字层都一样。
  func testCustomSidebarItemsTypeLiterally() throws {
    let document = SidebarDocument()
    try attach(document)
    select(.nineKey)
    XCTAssertTrue(write(TouchKeyLayoutStyle(nineKeySidebar: ["（）", "、", ":-)"], digitSidebar: ["()", "="])))
    let controller = keyboard()
    try button("nineKeySidebarSymbol0", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "（）")
    XCTAssertEqual(document.caretMoves, [-1], "the caret sits between the pair")
    try button("nineKeySidebarSymbol2", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "（）:-)", "typed as it is, not as full-width marks")
    XCTAssertEqual(document.caretMoves, [-1])
    // 组字中点一项：组字按首选上屏，再接上这一项。
    let idlePreedit = try button("preeditButton", in: controller).accessibilityValue
    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    try button("nineKey4", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse((try button("preeditButton", in: controller).accessibilityValue ?? "").isEmpty, "typing opens a composition")
    try button("nineKeySidebarSymbol1", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("preeditButton", in: controller).accessibilityValue, idlePreedit, "the composition ends")
    XCTAssertTrue(document.text.hasSuffix("、"))
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    try button("numberPadSidebarSymbol0", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(document.text.hasSuffix("、()"))
    XCTAssertEqual(document.caretMoves, [-1, -1])
    try button("numberPadSidebarSymbol1", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(document.text.hasSuffix("、()="))
    XCTAssertEqual(document.caretMoves, [-1, -1])
    controller.viewDidDisappear(false)
  }

  /// 半角语境的数字九宫格左列：出厂表的四个运算符之后按 `@ : ; ( ) ' " $` 的顺序追加半角符号，表里已有的跳过、不挪位置；中文（和日文）不追加；用户自定义过数字层的表时原样用用户的表，什么语境都不追加。追加的项读作「符号 …」，按数字页的路径原样上屏，英文邮箱里的 `@` 由此在键盘层上。
  func testAsciiNumberPadSidebarAppendsHalfWidthSymbols() throws {
    let extras = ["@", ":", ";", "(", ")", "'", "\"", "$"]
    XCTAssertEqual(NineKeySidebar.asciiDigitExtras, extras)
    XCTAssertEqual(NineKeySidebar.appendingAsciiDigitExtras(to: ["+", "(", "@"]), ["+", "(", "@", ":", ";", ")", "'", "\"", "$"],
                   "items already in the table are skipped and keep their place")
    XCTAssertEqual(TouchKeyLayoutStyle().numberPadSidebarSymbols(ascii: true), ["+", "-", "*", "/"] + extras)
    XCTAssertEqual(TouchKeyLayoutStyle().numberPadSidebarSymbols(ascii: false), ["+", "-", "*", "/"])
    XCTAssertEqual(TouchKeyLayoutStyle(digitSidebar: ["=", "@"]).numberPadSidebarSymbols(ascii: true), ["=", "@"],
                   "a custom table is used as it is")

    let document = SidebarDocument()
    try attach(document)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    select(.quanpin)
    func titles(_ controller: KeyboardViewController) throws -> [String?] {
      let stack = try XCTUnwrap(try view("numberPadOperatorScroll", in: controller).subviews.compactMap { $0 as? UIStackView }.first)
      return stack.arrangedSubviews.map { ($0 as? UIButton)?.configuration?.title }
    }
    func openNumberPad(english: Bool) throws -> KeyboardViewController {
      let controller = keyboard()
      if try (button("bottomLanguageKey", in: controller).accessibilityValue == "英文输入") != english {
        try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      }
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      return controller
    }

    // 英文：运算符之后接上 8 个半角符号，#+= 仍在最后；每一项与右列的 ⌫ 同高。
    var controller = try openNumberPad(english: true)
    XCTAssertEqual(try titles(controller), ["+", "-", "*", "/"] + extras + ["#+="])
    let delete = try button("nineKeyDelete", in: controller)
    for symbol in extras {
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "符号 \(symbol)" } as? UIButton, symbol)
      XCTAssertEqual(key.bounds.height, delete.bounds.height, accuracy: 0.5, symbol)
      key.sendActions(for: .primaryActionTriggered)
    }
    XCTAssertEqual(document.text, extras.joined(), "typed as drawn, half-width")
    XCTAssertEqual(document.caretMoves, [], "the parentheses are typed one by one, the caret stays at the end")
    // 切回中文，同一块数字九宫格的左列当场换回四个运算符。
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    if !shown(try button("nineKey1", in: controller)) {
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    }
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(try titles(controller), ["+", "-", "*", "/", "#+="], "Chinese adds nothing")
    controller.viewDidDisappear(false)

    // 自定义表：英文里也原样是用户的表。
    XCTAssertTrue(write(TouchKeyLayoutStyle(digitSidebar: ["=", "%"])))
    controller = try openNumberPad(english: true)
    XCTAssertEqual(try titles(controller), ["=", "%", "#+="], "a custom table gets no extras")
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.viewDidDisappear(false)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
  }

  /// 藏文数字九宫格左列的 `'`（achung）与原来 123 页上的一样交给组字：没有组字时开头一个音节（`'od`），组字中收进原文（`dga'`），都不先上屏组字、不按字面写出 `'`。测试宿主没带藏文时跳过。
  func testTibetanNumberPadApostropheGoesToTheComposition() throws {
    select(.tibetan)
    guard InputSchemePreference.scheme == .tibetan else { return }
    let document = SidebarDocument()
    try attach(document)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    let controller = keyboard()
    if try button("bottomLanguageKey", in: controller).accessibilityValue == "英文输入" {
      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    }
    func letters(_ word: String) throws {
      for letter in word {
        try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(letter.uppercased())" } as? UIButton,
                      String(letter)).sendActions(for: .primaryActionTriggered)
      }
    }
    func achung() throws {
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "符号 '" && shown($0) } as? UIButton)
        .sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(document.text, "", "nothing is committed and no literal ' is written")
      XCTAssertFalse(controller.inlineMarkedText.isEmpty, "the composition goes on")
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try layoutAtRequestedHeight(controller)
    }
    // 开头：' o d，空格上屏藏文并加音节点。
    try achung()
    try letters("od")
    try button("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "\u{0F60}\u{0F7C}\u{0F51}\u{0F0B}", "'od")
    // 组字中：d g a '。
    document.text = ""
    try letters("dga")
    try achung()
    try button("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "\u{0F51}\u{0F42}\u{0F60}\u{0F0B}", "dga'")
    controller.viewDidDisappear(false)
    select(.quanpin)
  }

  /// 大千注音的 #+= 是原来每排十键的共享符号行：键按键面直接写出，`1` 不当作注音键 ㄅ；底行的切换键印 123、回到数字九宫格，数字九宫格的返回键回到注音键盘。测试宿主没带注音词库时跳过。
  func testDachenMoreSymbolsAreTheSharedRowsAndReturnToTheNumberPad() throws {
    select(.zhuyin)
    guard InputSchemePreference.scheme == .zhuyin else { return }
    let document = SidebarDocument()
    try attach(document)
    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    let controller = keyboard()
    if try button("bottomLanguageKey", in: controller).accessibilityValue == "英文输入" {
      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    }
    let toggle = try button("layoutToggleButton", in: controller)
    toggle.sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(toggle.configuration?.title, "注")
    try button("numberPadMoreSymbols", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertFalse(shown(try button("nineKey1", in: controller)))
    XCTAssertFalse(shown(try view("symbolLayerRow0", in: controller)), "not the design #+= layer")
    XCTAssertEqual(toggle.configuration?.title, "123")
    for symbol in ["1", "@", "-"] {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "符号 \(symbol)" && shown($0) } as? UIButton, symbol)
        .sendActions(for: .primaryActionTriggered)
    }
    XCTAssertEqual(document.text, "1@-", "the shared rows type what they show, not bopomofo")
    XCTAssertEqual(controller.inlineMarkedText, "")
    attach(controller, "大千 #+= 共享符号行")
    toggle.sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(try button("nineKey1", in: controller)), "123 returns to the number pad")
    XCTAssertEqual(toggle.configuration?.title, "注")
    toggle.sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(toggle.configuration?.title, "123")
    XCTAssertTrue(shown(try view("zhuyinKeyq", in: controller)), "back on the Dachen keys")
    controller.viewDidDisappear(false)
    select(.quanpin)
  }

  /// 符号栏里显示着的键，自上而下，帧换算到符号栏里。
  private func sidebarMarks(_ sidebar: UIView) -> [(title: String, frame: CGRect)] {
    descendants(sidebar).compactMap { $0 as? UIButton }.filter { shown($0) }
      .map { ($0.configuration?.title ?? "", $0.convert($0.bounds, to: sidebar)) }
      .sorted { $0.frame.minY < $1.frame.minY }
  }

  /// 内存里的一份文档：测试宿主里的 `textDocumentProxy` 没有文档，上屏看不到（见 `EmailSuffixKeyboardTests`）。光标总在末尾，`adjustTextPosition` 只记下挪了几格。
  @MainActor
  private final class SidebarDocument: NSObject, UITextDocumentProxy {
    var text = ""
    var caretMoves: [Int] = []
    var documentContextBeforeInput: String? { text }
    var documentContextAfterInput: String? { nil }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    var documentIdentifier = UUID()
    var hasText: Bool { !text.isEmpty }
    func insertText(_ inserted: String) { text += inserted }
    func deleteBackward() { if !text.isEmpty { text.removeLast() } }
    func adjustTextPosition(byCharacterOffset offset: Int) { caretMoves.append(offset) }
    func setMarkedText(_ markedText: String, selectedRange: NSRange) {}
    func unmarkText() {}
  }

  private static let proxySelector = #selector(getter: UIInputViewController.textDocumentProxy)

  /// 让 `KeyboardViewController` 的 `textDocumentProxy` 返回 `document`，用例结束时换回父类的实现。
  private func attach(_ document: SidebarDocument) throws {
    let inherited = try XCTUnwrap(class_getInstanceMethod(UIInputViewController.self, Self.proxySelector))
    let original = method_getImplementation(inherited)
    let block: @convention(block) (AnyObject) -> AnyObject = { _ in document }
    let replacement = imp_implementationWithBlock(block)
    if !class_addMethod(KeyboardViewController.self, Self.proxySelector, replacement, method_getTypeEncoding(inherited)) {
      method_setImplementation(try XCTUnwrap(class_getInstanceMethod(KeyboardViewController.self, Self.proxySelector)), replacement)
    }
    addTeardownBlock {
      if let own = class_getInstanceMethod(KeyboardViewController.self, Self.proxySelector) {
        method_setImplementation(own, original)
      }
      imp_removeBlock(replacement)
    }
  }

  // MARK: - 中/英键放在空格左边（#10228）

  /// 读法：只有恰好写着字符串 `left` 时在空格左边，缺键、`right`、不认识的字符串和不是字符串的值都在右边，与 client-core 的宽松读取和 Android、鸿蒙的宿主读法相同。线上写法和设置页文字三端逐字相同。
  func testLanguageKeySideParsing() {
    XCTAssertEqual(TouchKeyLayoutStyle().languageKeySide, .right)
    XCTAssertEqual(TouchKeyLayoutStyle(in: ["touch_language_key_side": "left"]).languageKeySide, .left)
    for value: Any in ["right", "middle", "LEFT", 1, true, ["left"]] {
      XCTAssertEqual(TouchKeyLayoutStyle(in: ["touch_language_key_side": value]).languageKeySide, .right, "\(value)")
    }
    XCTAssertEqual(TouchKeyLayoutStyle(in: [:]).languageKeySide, .right)
    XCTAssertEqual(TouchKeyLayoutStyle.languageKeySideDocumentKey, "touch_language_key_side")
    XCTAssertEqual(TouchLanguageKeySide.allCases.map(\.rawValue), ["right", "left"])
    XCTAssertEqual(TouchKeyLayoutOffering.languageLeftTitle, "中/英键放在空格左边")
    XCTAssertEqual(TouchKeyLayoutOffering.languageLeftSubtitle, "中/英 排到空格左边，逗号、句号仍紧挨空格。")
  }

  /// 保存：打开写 `left`，关上写 `right`、由 client-core 从文档里省掉；文档接受了才更新镜像，其余各项不动。
  func testLanguageKeySideSaving() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(TouchKeyLayoutStyle.save(nineKey: .emoji, stateRoot: state))
    XCTAssertTrue(TouchKeyLayoutStyle.save(languageKeySide: .left, stateRoot: state))
    var document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_language_key_side"] as? String, "left")
    XCTAssertEqual(TouchKeyLayoutStyle(in: document), TouchKeyLayoutStyle(nineKey: .emoji, languageKeySide: .left))
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.languageKeySide, .left)
    XCTAssertTrue(TouchKeyLayoutStyle.save(languageKeySide: .right, stateRoot: state))
    document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertNil(document["touch_language_key_side"], "the default is not written")
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.languageKeySide, .right)
    XCTAssertEqual(TouchKeyLayoutStyle(in: document), TouchKeyLayoutStyle(nineKey: .emoji))
    // 镜像跟着文档走。
    TouchKeyLayoutStyle.mirror(["touch_language_key_side": "left"])
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored, TouchKeyLayoutStyle(languageKeySide: .left))
  }

  /// 规格第 2.4 节的每一行：中/英越过它左边紧挨着的那一串空格、0、逗号、句号，排到这一串的最左边；份额不变、只换次序；没有中/英（隐藏中/英键）时原样返回；再排一次不变。地球键照旧紧跟 123 或互换后的 123。
  func testLanguageBeforeSpaceTables() {
    let move: ([TouchBottomRowLayout.Key]) -> [TouchBottomRowLayout.Key] = TouchBottomRowLayout.languageBeforeSpace
    func slots(_ keys: [TouchBottomRowLayout.Key]) -> [Slot] { move(keys).map(\.slot) }
    // 中文 26 键四种样式，互换前后。
    XCTAssertEqual(slots(TouchBottomRowLayout.twentySixKey(.standard, globe: false)), [.layer, .symbols, .language, .space, .comma, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.twentySixKey(.commaPeriod, globe: false)),
                   [.layer, .symbols, .language, .comma, .space, .period, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.twentySixKey(.longSpace, globe: false)), [.layer, .language, .comma, .space, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.twentySixKey(.emoji, globe: false)), [.layer, .emoji, .language, .space, .comma, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.symbolFirst(TouchBottomRowLayout.twentySixKey(.standard, globe: false))),
                   [.symbols, .layer, .language, .space, .comma, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.symbolFirst(TouchBottomRowLayout.twentySixKey(.commaPeriod, globe: true))),
                   [.symbols, .layer, .globe, .language, .comma, .space, .period, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.twentySixKey(.standard, globe: true)),
                   [.layer, .globe, .symbols, .language, .space, .comma, .returnKey])
    // 14 / 18 键。
    XCTAssertEqual(slots(TouchBottomRowLayout.letterGrid(.standard, globe: false)), [.layer, .language, .comma, .space, .period, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.letterGrid(.longSpace, globe: false)), [.layer, .language, .comma, .space, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.letterGrid(.emoji, globe: false)), [.layer, .emoji, .language, .space, .comma, .returnKey])
    // 九键外框：标准、九键双拼的 0、底栏带标点、表情键。
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKey(.standard, globe: false)), [.symbols, .layer, .language, .space, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKey(.standard, globe: false, zero: true)),
                   [.symbols, .layer, .language, .space, .zero, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKey(.commaPeriod, globe: false)),
                   [.symbols, .layer, .language, .comma, .space, .period, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKey(.emoji, globe: false)), [.symbols, .layer, .emoji, .language, .space, .returnKey])
    // 右侧大回车：符 和回车不动，九宫格下方 123 | 中 | 空格，数字层 切换 | 中 | 0 | 空格。
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKeyTallReturn(globe: false)), [.symbols, .layer, .language, .space, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKeyTallReturn(globe: true, zero: true)),
                   [.symbols, .layer, .globe, .language, .space, .zero, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKeyTallReturn(globe: false, digits: true)),
                   [.symbols, .layer, .language, .zero, .space, .returnKey])
    // 数字九宫格三种空格位置。
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKeyDigits(globe: false, space: .afterZero)),
                   [.symbols, .layer, .language, .zero, .space, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKeyDigits(globe: false, space: .beforeZero)),
                   [.symbols, .layer, .language, .space, .zero, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.nineKeyDigits(globe: false, space: .rightColumn)), [.symbols, .layer, .language, .zero, .returnKey])
    XCTAssertEqual(slots(TouchBottomRowLayout.borrowedNumberPad(globe: true, space: .beforeZero)),
                   [.layer, .globe, .language, .space, .zero, .returnKey])

    var rows: [[TouchBottomRowLayout.Key]] = []
    for globe in [false, true] {
      for language in [true, false] {
        rows += TouchTwentySixKeyStyle.allCases.map { TouchBottomRowLayout.twentySixKey($0, globe: globe, language: language) }
        rows += TouchFourteenKeyStyle.allCases.map { TouchBottomRowLayout.letterGrid($0, globe: globe, language: language) }
        rows += TouchNineKeyStyle.allCases.flatMap { style in
          [false, true].map { TouchBottomRowLayout.nineKey(style, globe: globe, zero: $0, language: language) }
        }
        rows += [false, true].map { TouchBottomRowLayout.nineKeyTallReturn(globe: globe, language: language, digits: $0) }
        rows += TouchNumberPadSpace.allCases.flatMap { space in
          [TouchBottomRowLayout.nineKeyDigits(globe: globe, language: language, space: space),
           TouchBottomRowLayout.borrowedNumberPad(globe: globe, language: language, space: space)]
        }
      }
    }
    for row in rows {
      let moved = move(row)
      let label = "\(row.map(\.slot.rawValue))"
      XCTAssertEqual(moved.count, row.count, label)
      XCTAssertEqual(Set(moved), Set(row), "\(label) keeps every key with its share")
      XCTAssertEqual(move(moved), moved, "\(label) is stable")
      guard let language = moved.firstIndex(where: { $0.slot == .language }) else {
        XCTAssertEqual(moved, row, "\(label) without 中/英 is unchanged")
        continue
      }
      // 中/英右边紧接着的是空格、0 或标点；左边的不是。
      XCTAssertTrue(TouchBottomRowLayout.languageRunSlots.contains(moved[language + 1].slot), label)
      XCTAssertFalse(language > 0 && TouchBottomRowLayout.languageRunSlots.contains(moved[language - 1].slot), label)
      // 除了中/英，其余键的相对次序不变。
      XCTAssertEqual(moved.filter { $0.slot != .language }, row.filter { $0.slot != .language }, label)
    }
    // 中/英左边紧挨的不是这四种键时原样返回。
    let notBesideSpace = [TouchBottomRowLayout.Key(.layer, 1.25), TouchBottomRowLayout.Key(.language, 1.05),
                          TouchBottomRowLayout.Key(.space, 4), TouchBottomRowLayout.Key(.returnKey, 1.9)]
    XCTAssertEqual(move(notBesideSpace), notBesideSpace)
  }

  /// 预览：中/英键放在空格左边时三类底行都照键盘重排，在样式高亮之外再高亮中/英；隐藏了中/英键时底行与右边时相同。
  func testSettingsPreviewFollowsTheLanguageKeySide() {
    let styles = TouchTwentySixKeyStyle.allCases.flatMap { twentySix in
      TouchNineKeyStyle.allCases.flatMap { nine in
        [false, true].map { TouchKeyLayoutStyle(twentySixKey: twentySix, fourteenKey: .emoji, nineKey: nine, symbolFirst: $0) }
      }
    }
    for right in styles {
      var left = right
      left.languageKeySide = .left
      for kind in TouchKeyLayoutKind.allCases {
        let label = "\(kind.rawValue) \(right)"
        XCTAssertEqual(kind.bottomRow(left), TouchBottomRowLayout.languageBeforeSpace(kind.bottomRow(right)), label)
        XCTAssertEqual(kind.bottomRow(left, language: false), kind.bottomRow(right, language: false), "\(label) hidden 中/英")
        XCTAssertEqual(kind.highlightedSlots(left), kind.highlightedSlots(right).union([.language]), label)
        XCTAssertTrue(kind.highlightedSlots(left).isSubset(of: Set(kind.bottomRow(left).map(\.slot))), label)
        XCTAssertFalse(kind.highlightedSlots(right).contains(.language), label)
      }
    }
    // 右侧大回车 + 左：高亮回车和中/英。
    let tall = TouchKeyLayoutStyle(nineKey: .tallReturn, languageKeySide: .left)
    XCTAssertEqual(TouchKeyLayoutKind.nineKey.highlightedSlots(tall), [.returnKey, .language])
    XCTAssertEqual(TouchKeyLayoutKind.nineKey.bottomRow(tall).map(\.slot), [.symbols, .layer, .language, .space, .returnKey])
  }

  /// 统计热力图跟着画：26 键和九键（含右侧大回车）的底行与键盘同一次序，键的 id 和宽度不变。
  func testHeatmapFollowsTheLanguageKeySide() {
    let left = TouchKeyLayoutStyle(languageKeySide: .left)
    XCTAssertEqual(TypingKeyHeatmap.letterRows(for: left).last?.map(\.id),
                   [TypingKeyID.layer, TypingKeyID.symbol, TypingKeyID.language, TypingKeyID.space, TypingKeyID.quickPunctuation, TypingKeyID.enter])
    XCTAssertEqual(Set(TypingKeyHeatmap.letterRows(for: left).last!.map(\.width)),
                   Set(TypingKeyHeatmap.letterRows(for: TouchKeyLayoutStyle()).last!.map(\.width)))
    XCTAssertEqual(TypingKeyHeatmap.nineKeyBottomCells(for: .standard, languageLeft: true).map(\.id),
                   [TypingKeyID.symbol, TypingKeyID.layer, TypingKeyID.language, TypingKeyID.space, TypingKeyID.enter])
    XCTAssertEqual(TypingKeyHeatmap.nineKeyBottomCells(for: .tallReturn, languageLeft: true).map(\.id),
                   [TypingKeyID.symbol, TypingKeyID.layer, TypingKeyID.language, TypingKeyID.space, TypingKeyID.enter])
    XCTAssertEqual(TypingKeyHeatmap.cells(nineKey: true, style: TouchKeyLayoutStyle(languageKeySide: .left)).suffix(5).map(\.id),
                   TypingKeyHeatmap.nineKeyBottomCells(for: .standard, languageLeft: true).map(\.id))
    XCTAssertEqual(TypingKeyHeatmap.nineKeyBottomCells(for: .emoji), TypingKeyHeatmap.nineKeyBottomCells(for: .emoji, languageLeft: false))
  }

  /// 真键盘：中/英键放在空格左边时，中文 26 键四种样式（含互换）、英文 26 键、14 键、九键各样式、右侧大回车、九键自己的数字层、借来的数字九宫格三种空格位置和手写，底行的可见键序照规则，每个键的宽度与右边时相同；隐藏了中/英键时与右边时逐键相同；回车不再把左边让给中/英。
  func testLanguageLeftOnTheRealKeyboard() throws {
    /// 同一块键面在右、左两种偏好下的可见底行；`prepare` 把新建的键盘带到要比的那一层。
    func rows(_ style: TouchKeyLayoutStyle, _ label: String,
              prepare: (KeyboardViewController) throws -> Void = { _ in }) throws -> (right: [(slot: Slot, width: CGFloat)], left: [(slot: Slot, width: CGFloat)]) {
      var right = style
      right.languageKeySide = .right
      var left = style
      left.languageKeySide = .left
      XCTAssertTrue(write(right))
      var controller = keyboard()
      try prepare(controller)
      let before = visibleBottomRow(controller)
      controller.viewDidDisappear(false)
      XCTAssertTrue(write(left))
      controller = keyboard()
      try prepare(controller)
      let after = visibleBottomRow(controller)
      attach(controller, "中/英在空格左边 \(label)")
      controller.viewDidDisappear(false)
      return (before, after)
    }
    /// 左边时的可见键序是右边时的键序套上规则，键宽逐键相同。
    func assertMoved(_ row: (right: [(slot: Slot, width: CGFloat)], left: [(slot: Slot, width: CGFloat)]), _ label: String,
                     file: StaticString = #filePath, line: UInt = #line) {
      let expected = TouchBottomRowLayout.languageBeforeSpace(row.right) { $0.slot }
      XCTAssertEqual(row.left.map(\.slot), expected.map(\.slot), label, file: file, line: line)
      XCTAssertNotEqual(row.left.map(\.slot), row.right.map(\.slot), "\(label) moves 中/英", file: file, line: line)
      for key in row.right {
        guard let width = row.left.first(where: { $0.slot == key.slot })?.width else {
          XCTFail("\(label) lost \(key.slot.rawValue)", file: file, line: line)
          continue
        }
        XCTAssertEqual(width, key.width, accuracy: 1, "\(label) \(key.slot.rawValue) keeps its width", file: file, line: line)
      }
    }
    let toNumbers: (KeyboardViewController) throws -> Void = { controller in
      try self.button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      try self.layoutAtRequestedHeight(controller)
    }

    select(.quanpin)
    for style in TouchTwentySixKeyStyle.allCases {
      for symbolFirst in [false, true] {
        let label = "26 键 \(style.rawValue) symbolFirst=\(symbolFirst)"
        let row = try rows(TouchKeyLayoutStyle(twentySixKey: style, symbolFirst: symbolFirst), label)
        assertMoved(row, label)
        let globe = row.right.contains { $0.slot == .globe }
        let table = TouchBottomRowLayout.twentySixKey(style, globe: globe)
        XCTAssertEqual(row.left.map(\.slot),
                       TouchBottomRowLayout.languageBeforeSpace(symbolFirst ? TouchBottomRowLayout.symbolFirst(table) : table).map(\.slot), label)
      }
    }
    // 英文 26 键：中/英在 . 左边，与中文 26 键停在同一格。
    let english = try rows(TouchKeyLayoutStyle(), "英文 26 键") { controller in
      try self.button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
    }
    assertMoved(english, "英文 26 键")
    let globe = english.right.contains { $0.slot == .globe }
    XCTAssertEqual(english.left.map(\.slot), [.layer] + (globe ? [.globe] : []) + [.symbols, .language, .period, .space, .comma, .returnKey])
    // 全拼借来的数字九宫格三种空格位置：返回 |（🌐）| 中 | 0、空格 | 回车。
    for space in TouchNumberPadSpace.allCases {
      let label = "全拼数字九宫格 \(space.rawValue)"
      let row = try rows(TouchKeyLayoutStyle(numberPadSpace: space), label, prepare: toNumbers)
      assertMoved(row, label)
      XCTAssertEqual(row.left.firstIndex(where: { $0.slot == .language }), globe ? 2 : 1, label)
    }
    // 隐藏中/英键：偏好不起作用。
    KeyboardLayoutPreference.hideLanguageKey = true
    for style in [TouchTwentySixKeyStyle.standard, .commaPeriod] {
      let row = try rows(TouchKeyLayoutStyle(twentySixKey: style), "隐藏中/英 \(style.rawValue)")
      XCTAssertEqual(row.left.map(\.slot), row.right.map(\.slot), style.rawValue)
      XCTAssertFalse(row.left.contains { $0.slot == .language })
    }
    KeyboardLayoutPreference.hideLanguageKey = false

    select(.fourteenKey)
    for style in TouchFourteenKeyStyle.allCases {
      let label = "14 键 \(style.rawValue)"
      let row = try rows(TouchKeyLayoutStyle(fourteenKey: style), label)
      assertMoved(row, label)
      XCTAssertEqual(row.left.map(\.slot), TouchBottomRowLayout.languageBeforeSpace(TouchBottomRowLayout.letterGrid(
        style, globe: row.right.contains { $0.slot == .globe })).map(\.slot), label)
    }

    select(.nineKey)
    for style in TouchNineKeyStyle.allCases {
      let label = "九键 \(style.rawValue)"
      let row = try rows(TouchKeyLayoutStyle(nineKey: style), label)
      assertMoved(row, label)
      XCTAssertEqual(row.left.map(\.slot), TouchBottomRowLayout.languageBeforeSpace(TouchBottomRowLayout.nineKey(
        style, globe: row.right.contains { $0.slot == .globe })).map(\.slot), label)
    }
    let tallDigits = try rows(TouchKeyLayoutStyle(nineKey: .tallReturn), "右侧大回车数字层", prepare: toNumbers)
    assertMoved(tallDigits, "右侧大回车数字层")
    for space in TouchNumberPadSpace.allCases {
      let label = "九键数字层 \(space.rawValue)"
      let row = try rows(TouchKeyLayoutStyle(numberPadSpace: space), label, prepare: toNumbers)
      assertMoved(row, label)
      XCTAssertEqual(row.left.map(\.slot), TouchBottomRowLayout.languageBeforeSpace(TouchBottomRowLayout.nineKeyDigits(
        globe: row.right.contains { $0.slot == .globe }, space: space)).map(\.slot), label)
    }

    // 手写：中/英挪到 ， 左边，重写、⌫、回车不动（这两个键不在 `slotByIdentifier` 里，不列）。
    select(.handwriting)
    let handwriting = try rows(TouchKeyLayoutStyle(), "手写")
    assertMoved(handwriting, "手写")
    XCTAssertEqual(handwriting.left.map(\.slot).filter { $0 != .globe }, [.layer, .language, .comma, .space, .period, .returnKey])
  }

  /// 中/英不再紧挨回车时回车不让出左边：回车键帽左缘那一段归回车自己，中/英照常接自己的触摸；读屏顺序跟着排列次序走。
  func testReturnKeepsItsEdgeWhenTheLanguageKeyMovesLeft() throws {
    for scheme in [ChineseInputScheme.quanpin, .nineKey] {
      select(scheme)
      XCTAssertTrue(write(TouchKeyLayoutStyle(languageKeySide: .left)))
      let controller = keyboard()
      let language = try button("bottomLanguageKey", in: controller), enter = try button("returnKey", in: controller)
      let space = try button("spaceKey", in: controller)
      let languageFrame = language.convert(language.bounds, to: controller.view)
      let enterFrame = enter.convert(enter.bounds, to: controller.view)
      XCTAssertLessThan(languageFrame.maxX, space.convert(space.bounds, to: controller.view).minX, "\(scheme) 中/英 left of space")
      let y = enterFrame.midY
      XCTAssertTrue(controller.view.hitTest(CGPoint(x: enterFrame.minX + 1, y: y), with: nil) === enter, "\(scheme)")
      XCTAssertTrue(controller.view.hitTest(CGPoint(x: enterFrame.minX + KeyGapRouting.returnYield - 1, y: y), with: nil) === enter,
                    "\(scheme)")
      XCTAssertTrue(controller.view.hitTest(CGPoint(x: languageFrame.midX, y: y), with: nil) === language, "\(scheme)")
      let row = try XCTUnwrap(space.superview as? UIStackView)
      XCTAssertLessThan(try XCTUnwrap(row.arrangedSubviews.firstIndex(of: language)), try XCTUnwrap(row.arrangedSubviews.firstIndex(of: space)))
      controller.viewDidDisappear(false)
    }
  }

  /// 密码框和只收数字的框：样式退回标准，中/英的位置照用户选的，与「符」与「123」互换相同。iPad 全尺寸 26 键底行不受影响。
  func testLanguageLeftSurvivesPasswordFieldsAndSkipsTheTabletRow() throws {
    select(.quanpin)
    XCTAssertTrue(write(TouchKeyLayoutStyle(twentySixKey: .longSpace, languageKeySide: .left)))
    let controller = keyboard()
    let globe = controller.needsInputModeSwitchKey
    controller.applyInputContext(keyboardType: .numberPad, documentIdentifier: UUID())
    try layoutAtRequestedHeight(controller)
    XCTAssertEqual(visibleBottomRow(controller).map(\.slot),
                   TouchBottomRowLayout.languageBeforeSpace(TouchBottomRowLayout.twentySixKey(.standard, globe: globe)).map(\.slot),
                   "a digit-only field keeps the standard style with 中/英 on the left")
    controller.viewDidDisappear(false)

    XCTAssertTrue(write(TouchKeyLayoutStyle()))
    var tablet = self.tablet()
    let right = visibleBottomRow(tablet).map(\.slot)
    tablet.viewDidDisappear(false)
    XCTAssertTrue(write(TouchKeyLayoutStyle(languageKeySide: .left)))
    tablet = self.tablet()
    XCTAssertEqual(visibleBottomRow(tablet).map(\.slot), right, "the full-size iPad row has its own order")
    tablet.viewDidDisappear(false)
  }

  // MARK: - 辅助

  private func write(_ style: TouchKeyLayoutStyle) -> Bool {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { document in
      document[TouchKeyLayoutStyle.twentySixKeyDocumentKey] = style.twentySixKey.rawValue
      document[TouchKeyLayoutStyle.fourteenKeyDocumentKey] = style.fourteenKey.rawValue
      document[TouchKeyLayoutStyle.nineKeyDocumentKey] = style.nineKey.rawValue
      document[TouchKeyLayoutStyle.numberRowDocumentKey] = style.numberRow
      document[TouchKeyLayoutStyle.symbolFirstDocumentKey] = style.symbolFirst
      document[TouchKeyLayoutStyle.numberPadSpaceDocumentKey] = style.numberPadSpace.rawValue
      document[TouchKeyLayoutStyle.nineKeySidebarDocumentKey] = style.nineKeySidebar
      document[TouchKeyLayoutStyle.digitSidebarDocumentKey] = style.digitSidebar
      document[TouchKeyLayoutStyle.languageKeySideDocumentKey] = style.languageKeySide.rawValue
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

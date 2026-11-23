import XCTest
import UIKit

/// 字母键的角标符号（ios-kb-11）：与 Android 共用的符号表和滑动阈值，以及 26 键怎样绘制和输入它们。
@MainActor
final class LetterCornerHintTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func button(_ identifier: String, in controller: UIViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }

  private func key(labelled label: String, in controller: UIViewController) throws -> KeyboardKeyButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == label } as? KeyboardKeyButton,
                  "No key labelled \(label).")
  }

  private func cornerLabel(of key: UIButton) -> UILabel? {
    key.subviews.first { $0.accessibilityIdentifier == "letterCornerHint" } as? UILabel
  }

  private func makeController(_ scheme: ChineseInputScheme) -> KeyboardViewController {
    let previous = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  func testTableMatchesTheDesignRows() {
    let rows = [("qwertyuiop", "1234567890"), ("asdfghjkl", "@#¥%&*()\""), ("zxcvbnm", "~…、?!-/")]
    for (letters, hints) in rows {
      for (letter, hint) in zip(letters, hints) {
        XCTAssertEqual(LetterHintTable.hint(for: String(letter)), String(hint), String(letter))
        XCTAssertEqual(LetterHintTable.hint(for: String(letter).uppercased()), String(hint), "upper-case \(letter)")
      }
    }
    for other in [";", "1", "", "qq", "é", "ㅂ"] {
      XCTAssertNil(LetterHintTable.hint(for: other), other)
    }
  }

  func testSwipeNeedsMoreThanFourteenPointsInTheSelectedDirection() {
    XCTAssertFalse(SwipeHintPolicy.swiped(downY: 40, currentY: 26))
    XCTAssertTrue(SwipeHintPolicy.swiped(downY: 40, currentY: 25.5))
    XCTAssertFalse(SwipeHintPolicy.swiped(downY: 10, currentY: 40))
    XCTAssertFalse(SwipeHintPolicy.swiped(direction: .down, downY: 10, currentY: 24))
    XCTAssertTrue(SwipeHintPolicy.swiped(direction: .down, downY: 10, currentY: 24.5))
    XCTAssertFalse(SwipeHintPolicy.swiped(direction: .down, downY: 40, currentY: 10))
  }

  func testSwipeDirectionDefaultsUpAndRefreshesExistingLetterKeys() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let name = KeyboardLayoutPreference.swipeSymbolsDirectionKey
    let stored = defaults.object(forKey: name)
    addTeardownBlock {
      if let stored { defaults.set(stored, forKey: name) }
      else { defaults.removeObject(forKey: name) }
    }
    defaults.removeObject(forKey: name)
    XCTAssertEqual(KeyboardLayoutPreference.swipeSymbolsDirection, .up)
    let controller = makeController(.quanpin)
    let q = try key(labelled: "字母 Q", in: controller)
    XCTAssertEqual(q.cornerHintSwipeDirection, .up)
    KeyboardLayoutPreference.swipeSymbolsDirection = .down
    controller.viewWillAppear(false)
    XCTAssertEqual(q.cornerHintSwipeDirection, .down)
    XCTAssertEqual(defaults.string(forKey: name), "down")
    defaults.set("sideways", forKey: name)
    controller.viewWillAppear(false)
    XCTAssertEqual(q.cornerHintSwipeDirection, .up)
  }

  func testQwertyKeysDrawAndCarryTheirCornerSymbol() throws {
    let controller = makeController(.quanpin)
    let q = try key(labelled: "字母 Q", in: controller)
    let l = try key(labelled: "字母 L", in: controller)
    XCTAssertEqual(q.cornerHint, "1")
    XCTAssertEqual(cornerLabel(of: q)?.text, "1")
    XCTAssertEqual(cornerLabel(of: q)?.isHidden, false)
    XCTAssertEqual(l.cornerHint, "\"")
    XCTAssertEqual(cornerLabel(of: q)?.isAccessibilityElement, false, "the key still reads as its letter")

    // 微软双拼的 `;` 键没有自己的角标符号。
    let semicolon = try XCTUnwrap(try button("microsoftFinalKey", in: controller) as? KeyboardKeyButton)
    XCTAssertNil(semicolon.cornerHint)
    XCTAssertEqual(cornerLabel(of: semicolon)?.isHidden, true)
  }

  func testKoreanJamoFacesHaveNoCornerSymbol() throws {
    let controller = makeController(.korean)
    let q = try key(labelled: "字母 ㅂ", in: controller)
    XCTAssertNil(q.cornerHint)
    XCTAssertEqual(cornerLabel(of: q)?.isHidden, true)
  }

  /// 双拼键位提示和右上角的上滑符号同时画着时，字母上下各让出一条提示行的 11pt：只让下边距的话，
  /// 22pt 字母会在 44pt 键上整体上移 5.5pt，字顶从 14.2 抬到 8.7，钻进 10pt 角标的字底（约 12.5）。
  func testShuangpinHintKeepsTheLetterClearOfTheCornerSymbol() throws {
    let q = try shuangpinLetterKey()
    let hint = try XCTUnwrap(q.subviews.compactMap { $0 as? UILabel }.first { $0.font.pointSize == 9 })
    XCTAssertFalse(hint.isHidden, "the double-pinyin hint is drawn")
    XCTAssertEqual(try XCTUnwrap(cornerLabel(of: q)).isHidden, false)

    XCTAssertEqual(q.configuration?.contentInsets.top, 11)
    XCTAssertEqual(q.configuration?.contentInsets.bottom, 11, "上下一样多，字母仍落在按键中心")

    // 字顶在角标字底之下：角标距上边 3pt，10pt 字体的字底就是基线（上边到 ascent）。
    let letter = UIFont.systemFont(ofSize: KeyboardViewController.letterFontSize(.phone))
    let capTop = q.bounds.height / 2 - letter.capHeight / 2
    let corner = try XCTUnwrap(cornerLabel(of: q)?.font)
    XCTAssertGreaterThan(capTop, 3 + corner.ascender, "字母压进了右上角的上滑符号")
  }

  /// 角标关掉（或本来就没有）时不占那一条：字母只为底部的双拼提示让位，不留一段空白的上边距。
  func testTheCornerBandGoesAwayWithTheCornerSymbol() throws {
    let q = try shuangpinLetterKey(cornerHints: false)
    XCTAssertEqual(try XCTUnwrap(cornerLabel(of: q)).isHidden, true)
    XCTAssertEqual(q.configuration?.contentInsets.top, 0)
    XCTAssertEqual(q.configuration?.contentInsets.bottom, 11)
  }

  /// 手机横屏的键只有 34pt：上下都让 11pt 只剩 12pt，22pt 字母会被挤到下面那条提示上，
  /// 所以那里仍然只让下边距。
  func testPhoneLandscapeKeepsOnlyTheBottomHintBand() throws {
    let storedHints = KeyboardLayoutPreference.shuangpinKeyHints
    let storedCorner = KeyboardLayoutPreference.letterCornerHints
    let previousScheme = InputSchemePreference.scheme
    addTeardownBlock {
      KeyboardLayoutPreference.shuangpinKeyHints = storedHints
      KeyboardLayoutPreference.letterCornerHints = storedCorner
      InputSchemePreference.scheme = previousScheme
    }
    KeyboardLayoutPreference.shuangpinKeyHints = true
    KeyboardLayoutPreference.letterCornerHints = true
    InputSchemePreference.scheme = .shuangpin
    let controller = KeyboardViewController()
    controller.traitOverrides.verticalSizeClass = .compact
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 844, height: 200)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()

    let q = try key(labelled: "字母 Q", in: controller)
    XCTAssertEqual(q.configuration?.contentInsets.top, 0)
    XCTAssertEqual(q.configuration?.contentInsets.bottom, 11)
  }

  /// 双拼方案下字母键带着引擎给的键位提示、角标按开关画：返回 Q 键。
  private func shuangpinLetterKey(cornerHints: Bool = true) throws -> KeyboardKeyButton {
    let storedHints = KeyboardLayoutPreference.shuangpinKeyHints
    let storedCorner = KeyboardLayoutPreference.letterCornerHints
    addTeardownBlock {
      KeyboardLayoutPreference.shuangpinKeyHints = storedHints
      KeyboardLayoutPreference.letterCornerHints = storedCorner
    }
    KeyboardLayoutPreference.shuangpinKeyHints = true
    KeyboardLayoutPreference.letterCornerHints = cornerHints
    let controller = makeController(.shuangpin)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return try key(labelled: "字母 Q", in: controller)
  }

  func testCornerSymbolFinishesTheCompositionInsteadOfJoiningIt() throws {
    let controller = makeController(.quanpin)
    controller.viewWillAppear(false)
    for label in ["字母 N", "字母 I"] {
      try key(labelled: label, in: controller).sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    let preedit = try button("preeditButton", in: controller)
    XCTAssertTrue(preedit.accessibilityValue?.contains("ni") == true, "ni is composing")

    // 在 q 上滑、松手时交给控制器的内容。
    let q = try key(labelled: "字母 Q", in: controller)
    let typeHint = try XCTUnwrap(q.onCornerHint)
    typeHint("1")
    controller.view.layoutIfNeeded()
    XCTAssertEqual(preedit.configuration?.title, "水杉输入法",
                   "the composition was committed before the symbol, not extended by it")
  }

  /// 「滑动输入符号」关掉后字母键不再响应滑动，长按仍输入角标（与 Android 一致）；键盘再次出现时读到新值。
  func testSwipeSymbolsSwitchReachesTheLetterKeys() throws {
    let stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.swipeSymbolsKey)
    addTeardownBlock {
      if let stored { KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
    }
    KeyboardLayoutPreference.swipeSymbols = false
    let controller = makeController(.quanpin)
    let q = try key(labelled: "字母 Q", in: controller)
    XCTAssertFalse(q.swipesCornerHint)
    XCTAssertEqual(q.cornerHint, "1", "the hold still types the corner symbol")
    KeyboardLayoutPreference.swipeSymbols = true
    controller.viewWillAppear(false)
    XCTAssertTrue(q.swipesCornerHint)
  }

  /// 「显示字母键角标」（#10587）默认开；关掉后角标那一行藏起，按键仍带着角标符号，长按和滑动照样输入它；键盘再次出现时已画好的键面当场跟着收起或画回。
  func testCornerHintSwitchHidesTheSymbolButKeepsItsGestures() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let name = KeyboardLayoutPreference.letterCornerHintsKey
    let stored = defaults.object(forKey: name)
    addTeardownBlock {
      if let stored { defaults.set(stored, forKey: name) }
      else { defaults.removeObject(forKey: name) }
    }
    defaults.removeObject(forKey: name)
    XCTAssertTrue(KeyboardLayoutPreference.letterCornerHints, "shown unless the user turns it off")

    KeyboardLayoutPreference.letterCornerHints = false
    let controller = makeController(.quanpin)
    let q = try key(labelled: "字母 Q", in: controller)
    XCTAssertEqual(cornerLabel(of: q)?.isHidden, true)
    XCTAssertEqual(q.cornerHint, "1", "the hold and the swipe still type the symbol")
    XCTAssertNotNil(q.onCornerHint)
    XCTAssertEqual(q.accessibilityLabel, "字母 Q")

    KeyboardLayoutPreference.letterCornerHints = true
    controller.viewWillAppear(false)
    XCTAssertEqual(cornerLabel(of: q)?.isHidden, false)
    XCTAssertEqual(cornerLabel(of: q)?.text, "1")

    KeyboardLayoutPreference.letterCornerHints = false
    controller.viewWillAppear(false)
    XCTAssertEqual(cornerLabel(of: q)?.isHidden, true)
    XCTAssertEqual(q.cornerHint, "1")
  }

  // MARK: - 每键上下滑

  /// 字母键两个方向各判一次：严格超过 14pt 才算，方向互斥；14/18 键还要求纵向占优。
  func testLetterSwipeDirectionPicksUpOrDown() {
    let down = CGPoint(x: 100, y: 100)
    XCTAssertNil(LetterSwipePolicy.direction(down: down, current: CGPoint(x: 100, y: 86)), "14pt 不算")
    XCTAssertEqual(LetterSwipePolicy.direction(down: down, current: CGPoint(x: 100, y: 85)), .up)
    XCTAssertEqual(LetterSwipePolicy.direction(down: down, current: CGPoint(x: 100, y: 115)), .down)
    XCTAssertNil(LetterSwipePolicy.direction(down: down, current: CGPoint(x: 100, y: 100)), "没动")
    XCTAssertNil(LetterSwipePolicy.direction(down: down, current: CGPoint(x: CGFloat.nan, y: 0)), "非有限值")
    // 横向为主的斜滑在 14/18 键上不算纵向滑动；26 键没有这条限制。
    let diagonal = CGPoint(x: 160, y: 80)
    XCTAssertNil(LetterSwipePolicy.direction(down: down, current: diagonal, requiresVerticalDominance: true))
    XCTAssertEqual(LetterSwipePolicy.direction(down: down, current: diagonal), .up)
  }

  /// 没单独配置的键按「滑动方向」回落：那个方向是角标，另一边不接；显式存下的「无」要压住默认。
  func testLetterSwipeBindingsFallBackToTheDirectionSetting() {
    let defaults = KeyboardLayoutPreference.defaults
    let name = LetterSwipePreference.key
    let storedDirection = defaults.string(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey)
    let storedTable = defaults.data(forKey: name)
    addTeardownBlock {
      if let storedDirection { defaults.set(storedDirection, forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
      if let storedTable { defaults.set(storedTable, forKey: name) } else { defaults.removeObject(forKey: name) }
    }
    LetterSwipePreference.reset()
    KeyboardLayoutPreference.swipeSymbolsDirection = .up
    XCTAssertEqual(LetterSwipePreference.binding(letter: "q", direction: .up).action, .symbol)
    XCTAssertEqual(LetterSwipePreference.binding(letter: "q", direction: .down).action, .none)

    KeyboardLayoutPreference.swipeSymbolsDirection = .down
    XCTAssertEqual(LetterSwipePreference.binding(letter: "q", direction: .down).action, .symbol)
    XCTAssertEqual(LetterSwipePreference.binding(letter: "q", direction: .up).action, .none)

    // 「上滑+下滑」：两个方向都回落成角标。
    KeyboardLayoutPreference.swipeSymbolsDirection = .both
    XCTAssertEqual(LetterSwipePreference.binding(letter: "q", direction: .up).action, .symbol)
    XCTAssertEqual(LetterSwipePreference.binding(letter: "q", direction: .down).action, .symbol)

    // 显式把上滑改成「无」：存下去，不再回落成角标。
    LetterSwipePreference.set(LetterSwipeBinding(action: .none), letter: "q", direction: .up)
    XCTAssertEqual(LetterSwipePreference.binding(letter: "q", direction: .up).action, .none)
    // 这里的期望值必须写全 `LetterSwipeAction.none`：左边是 `LetterSwipeAction?`，裸写 `.none`
    // 会被解析成 `Optional.none`（即 nil），拿 some(.none) 去和 nil 比，断言必错。
    XCTAssertEqual(LetterSwipePreference.stored(letter: "q", direction: .up)?.action, LetterSwipeAction.none)

    LetterSwipePreference.set(LetterSwipeBinding(action: .paste), letter: "v", direction: .down)
    XCTAssertEqual(LetterSwipePreference.binding(letter: "v", direction: .down).action, .paste)
    XCTAssertEqual(LetterSwipePreference.binding(letter: "V", direction: .down).action, .paste, "大小写同一个键")
    XCTAssertEqual(LetterSwipePreference.binding(letter: "v", direction: .up).action, .none,
                   "另一个方向仍按「滑动方向」回落")

    LetterSwipePreference.set(LetterSwipeBinding(action: .text, text: "hi@example.com"), letter: "v", direction: .up)
    XCTAssertEqual(LetterSwipePreference.binding(letter: "v", direction: .up).trimmedText, "hi@example.com")
    XCTAssertTrue(LetterSwipePreference.binding(letter: "v", direction: .up).isActive)

    // 选了自定义文本却没填：这条配置不算数，滑动不做事。
    LetterSwipePreference.set(LetterSwipeBinding(action: .text, text: "   "), letter: "v", direction: .up)
    XCTAssertFalse(LetterSwipePreference.binding(letter: "v", direction: .up).isActive)

    LetterSwipePreference.clear(letter: "v")
    XCTAssertNil(LetterSwipePreference.stored(letter: "v", direction: .down))
  }

  /// 列表行把「角标符号」写成真实符号：没给符号时回落成「角标符号」，给了就带上（如 Q 键「角标符号 1」）。
  func testLetterSwipeSummaryShowsTheRealHintSymbol() {
    XCTAssertEqual(LetterSwipeBinding(action: .symbol).summary(), "角标符号")
    XCTAssertEqual(LetterSwipeBinding(action: .symbol).summary(symbol: "1"), "角标符号 1")
    XCTAssertEqual(LetterSwipeBinding(action: .paste).summary(symbol: "1"), "粘贴")
    XCTAssertEqual(LetterSwipeBinding(action: .text, text: "hi@example.com").summary(symbol: "1"), "文本：hi@example.com")
  }

  /// 预览图键面徽标用短名：动作本身，不写方向，长名字压缩（复制选中文字 → 复制）。
  func testLetterSwipeShortTitleAbbreviatesForBadges() {
    XCTAssertEqual(LetterSwipeAction.copy.shortTitle(), "复制")
    XCTAssertEqual(LetterSwipeAction.cut.shortTitle(), "剪切")
    XCTAssertEqual(LetterSwipeAction.paste.shortTitle(), "粘贴")
    XCTAssertEqual(LetterSwipeAction.clipboard.shortTitle(), "剪贴板")
    XCTAssertEqual(LetterSwipeAction.language.shortTitle(), "中英")
    XCTAssertEqual(LetterSwipeAction.cursorLeft.shortTitle(), "左移")
    XCTAssertEqual(LetterSwipeAction.symbol.shortTitle(symbol: "1"), "1")
    XCTAssertEqual(LetterSwipeAction.text.shortTitle(), "文本")
  }

  /// 控制器把每键的上滑、下滑配置交给按键；总开关关掉后两个方向都不接。
  func testLetterKeyCarriesBothDirectionsFromThePerKeyTable() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let storedTable = defaults.data(forKey: LetterSwipePreference.key)
    let storedSymbols = defaults.object(forKey: KeyboardLayoutPreference.swipeSymbolsKey)
    let storedDirection = defaults.string(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey)
    addTeardownBlock {
      if let storedTable { defaults.set(storedTable, forKey: LetterSwipePreference.key) }
      else { defaults.removeObject(forKey: LetterSwipePreference.key) }
      if let storedSymbols { defaults.set(storedSymbols, forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      if let storedDirection { defaults.set(storedDirection, forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
    }
    LetterSwipePreference.reset()
    KeyboardLayoutPreference.swipeSymbolsDirection = .up
    KeyboardLayoutPreference.swipeSymbols = true
    LetterSwipePreference.set(LetterSwipeBinding(action: .paste), letter: "v", direction: .down)

    let controller = makeController(.quanpin)
    controller.viewWillAppear(false)
    let v = try key(labelled: "字母 V", in: controller)
    XCTAssertEqual(v.swipeActions?(.up), .symbol)
    XCTAssertEqual(v.swipeActions?(.down), .paste)
    XCTAssertNotNil(v.onSwipeAction)

    KeyboardLayoutPreference.swipeSymbols = false
    controller.viewWillAppear(false)
    XCTAssertNil(v.swipeActions?(.up), "总开关关着时两个方向都不接")
    XCTAssertNil(v.swipeActions?(.down))
  }
}

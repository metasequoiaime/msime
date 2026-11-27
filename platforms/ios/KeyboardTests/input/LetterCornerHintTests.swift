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

  /// 自己和每一层父视图都没被隐藏，才算画在屏幕上。
  private func shown(_ view: UIView) -> Bool {
    sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden }
  }

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

  /// 「滑动输入符号」关掉后字母键不再响应滑动，长按选项照常（长按与滑动互不依赖）；键盘再次出现时读到新值。
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
    XCTAssertEqual(q.cornerHint, "1", "the corner symbol stays on the key face")
    // Q 默认没有声调，长按不再弹选项；换 E 验证长按与滑动开关互不依赖。
    XCTAssertEqual(q.holdOptions?().count ?? 0, 0)
    let e = try key(labelled: "字母 E", in: controller)
    XCTAssertNotNil(e.holdOptions?().first, "long press does not depend on the swipe switch")
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
    // 此时「滑动方向」已是 .both（上文第 278 行设置），v 的上滑没单独配过，
    // 所以按 .both 回落成角标，而不是 .none。
    XCTAssertEqual(LetterSwipePreference.binding(letter: "v", direction: .up).action, .symbol,
                   "另一个方向仍按「滑动方向」回落成角标")

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
    // 空格上滑默认动作：名字里写清「候选译文」，徽标只写「译文」。
    XCTAssertEqual(LetterSwipeAction.commitTranslation.shortTitle(), "译文")
    XCTAssertEqual(LetterSwipeAction.commitTranslation.title(), "上屏候选译文")
    XCTAssertEqual(LetterSwipeAction.symbol.shortTitle(symbol: "1"), "1")
    XCTAssertEqual(LetterSwipeAction.text.shortTitle(), "文本")
    // 切别的键盘的四个动作：气泡与徽标都放不下「切换到 123 键盘」这样的完整名字，只写键面或面板的短名。
    XCTAssertEqual(LetterSwipeAction.numberPad.shortTitle(), "123")
    XCTAssertEqual(LetterSwipeAction.moreSymbols.shortTitle(), "#+=")
    XCTAssertEqual(LetterSwipeAction.symbolPanel.shortTitle(), "符")
    XCTAssertEqual(LetterSwipeAction.phrases.shortTitle(), "短语")
    XCTAssertEqual(LetterSwipeAction.numberPad.title(), "切换到 123 键盘")
    XCTAssertEqual(LetterSwipeAction.moreSymbols.title(), "切换到 #+= 键盘")
    XCTAssertEqual(LetterSwipeAction.symbolPanel.title(), "打开符号面板（符）")
    XCTAssertEqual(LetterSwipeAction.phrases.title(), "常用语")
    // 水杉图标菜单的快捷操作也进了选项，徽标用短名。
    XCTAssertEqual(LetterSwipeAction.fullWidth.shortTitle(), "全半角")
    XCTAssertEqual(LetterSwipeAction.chinesePunctuation.shortTitle(), "标点")
    XCTAssertEqual(LetterSwipeAction.traditional.shortTitle(), "繁简")
    XCTAssertEqual(LetterSwipeAction.handwriting.shortTitle(), "手写")
    XCTAssertEqual(LetterSwipeAction.oneHand.shortTitle(), "单手")
    XCTAssertEqual(LetterSwipeAction.incognito.shortTitle(), "隐私")
    XCTAssertEqual(LetterSwipeAction.emoji.shortTitle(), "表情")
    XCTAssertEqual(LetterSwipeAction.aiAssist.shortTitle(), "润色")
    XCTAssertEqual(LetterSwipeAction.reply.shortTitle(), "回复")
    XCTAssertEqual(LetterSwipeAction.voiceInput.shortTitle(), "语音")
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

  /// 滑动可以切到别的键盘：滑到 123 打开数字九宫格，再滑到 #+= 进符号页，不按当前层取反。
  func testSwipeSwitchesToNumberPadAndMoreSymbols() throws {
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
    KeyboardLayoutPreference.swipeSymbols = true
    KeyboardLayoutPreference.swipeSymbolsDirection = .up
    LetterSwipePreference.set(LetterSwipeBinding(action: .numberPad), letter: "v", direction: .down)
    LetterSwipePreference.set(LetterSwipeBinding(action: .moreSymbols), letter: "v", direction: .up)

    let controller = makeController(.quanpin)
    controller.viewWillAppear(false)
    let v = try key(labelled: "字母 V", in: controller)
    XCTAssertEqual(v.swipeActions?(.down), .numberPad)
    XCTAssertEqual(v.swipeActions?(.up), .moreSymbols)

    v.onSwipeAction?(.numberPad, .down)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(try button("numberPadSymbolPanelKey", in: controller)), "下滑到 123 画数字九宫格")

    v.onSwipeAction?(.moreSymbols, .up)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(try button("numberPadSymbolPanelKey", in: controller)), "上滑到 #+= 离开数字九宫格")
  }

  /// 全选的组字重绘只对短文本、且光标已在末尾时做：光标后还有内容时挪不到真正的末尾，删不干净；
  /// 长文逐字删除太慢，改走「只记全文」那一档。
  func testSelectAllOnlyRemarksShortTextWithCursorAtTheEnd() {
    XCTAssertTrue(SelectAllPolicy.canRemark(documentLength: 12, caretAtEnd: true))
    XCTAssertTrue(SelectAllPolicy.canRemark(documentLength: SelectAllPolicy.remarkLimit, caretAtEnd: true))
    XCTAssertFalse(SelectAllPolicy.canRemark(documentLength: 0, caretAtEnd: true), "空输入框没什么可选的")
    XCTAssertFalse(SelectAllPolicy.canRemark(documentLength: SelectAllPolicy.remarkLimit + 1, caretAtEnd: true))
    XCTAssertFalse(SelectAllPolicy.canRemark(documentLength: 12, caretAtEnd: false))
  }

  // MARK: - 下滑动作提示

  /// 字母键底部的短行：这个键下滑配了动作（粘贴、123 等）就写动作短名；没配、回落成角标或只靠上滑时不写。
  /// 开关关掉后那一行收回，下滑本身照旧执行。
  func testSwipeActionHintWritesTheDownwardsActionName() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let storedTable = defaults.data(forKey: LetterSwipePreference.key)
    let storedSymbols = defaults.object(forKey: KeyboardLayoutPreference.swipeSymbolsKey)
    let storedDirection = defaults.string(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey)
    let storedSwipeHint = defaults.object(forKey: KeyboardLayoutPreference.letterSwipeHintKey)
    addTeardownBlock {
      if let storedTable { defaults.set(storedTable, forKey: LetterSwipePreference.key) }
      else { defaults.removeObject(forKey: LetterSwipePreference.key) }
      if let storedSymbols { defaults.set(storedSymbols, forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      if let storedDirection { defaults.set(storedDirection, forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
      if let storedSwipeHint { defaults.set(storedSwipeHint, forKey: KeyboardLayoutPreference.letterSwipeHintKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.letterSwipeHintKey) }
    }
    LetterSwipePreference.reset()
    KeyboardLayoutPreference.swipeSymbols = true
    KeyboardLayoutPreference.swipeSymbolsDirection = .up
    KeyboardLayoutPreference.letterSwipeHint = true
    LetterSwipePreference.set(LetterSwipeBinding(action: .paste), letter: "v", direction: .down)
    LetterSwipePreference.set(LetterSwipeBinding(action: .numberPad), letter: "b", direction: .down)

    let controller = makeController(.quanpin)
    controller.viewWillAppear(false)
    let v = try key(labelled: "字母 V", in: controller)
    XCTAssertEqual(try bottomHint(of: v).text, "粘贴")
    XCTAssertFalse(try bottomHint(of: v).isHidden)
    XCTAssertEqual(try bottomHint(of: try key(labelled: "字母 B", in: controller)).text, "123")
    // Q 没配下滑：默认方向是上滑，下滑不接，底部那行留空。
    let q = try key(labelled: "字母 Q", in: controller)
    XCTAssertTrue(try bottomHint(of: q).isHidden)
    XCTAssertEqual(q.configuration?.contentInsets.bottom, 0, "没有底部提示就不让位")

    KeyboardLayoutPreference.letterSwipeHint = false
    controller.viewWillAppear(false)
    XCTAssertTrue(try bottomHint(of: v).isHidden, "关掉开关只收起提示")
    XCTAssertEqual(v.swipeActions?(.down), .paste, "下滑本身照旧")
    XCTAssertEqual(v.configuration?.contentInsets.bottom, 0)
  }

  /// 双拼键位提示和下滑动作提示占同一行：双拼提示在时以它为准，不叠字；关掉双拼提示后下滑短名才出现。
  func testShuangpinHintWinsOverTheSwipeActionHint() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let storedTable = defaults.data(forKey: LetterSwipePreference.key)
    let storedSymbols = defaults.object(forKey: KeyboardLayoutPreference.swipeSymbolsKey)
    let storedDirection = defaults.string(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey)
    let storedSwipeHint = defaults.object(forKey: KeyboardLayoutPreference.letterSwipeHintKey)
    let storedShuangpin = defaults.object(forKey: KeyboardLayoutPreference.shuangpinKeyHintsKey)
    let previousScheme = InputSchemePreference.scheme
    addTeardownBlock {
      if let storedTable { defaults.set(storedTable, forKey: LetterSwipePreference.key) }
      else { defaults.removeObject(forKey: LetterSwipePreference.key) }
      if let storedSymbols { defaults.set(storedSymbols, forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      if let storedDirection { defaults.set(storedDirection, forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsDirectionKey) }
      if let storedSwipeHint { defaults.set(storedSwipeHint, forKey: KeyboardLayoutPreference.letterSwipeHintKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.letterSwipeHintKey) }
      if let storedShuangpin { defaults.set(storedShuangpin, forKey: KeyboardLayoutPreference.shuangpinKeyHintsKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.shuangpinKeyHintsKey) }
      InputSchemePreference.scheme = previousScheme
    }
    LetterSwipePreference.reset()
    KeyboardLayoutPreference.swipeSymbols = true
    KeyboardLayoutPreference.swipeSymbolsDirection = .up
    KeyboardLayoutPreference.letterSwipeHint = true
    LetterSwipePreference.set(LetterSwipeBinding(action: .paste), letter: "v", direction: .down)

    // 双拼提示开关改的是共享文档（`touch_shuangpin_key_hints`），键盘出现时按文档重读，所以两段都走同一条路：
    // 先写文档再建控制器。只改 App Group 里的镜像不够——`viewWillAppear` 会用文档里的值盖回去。
    let storedDocumentHint = MetasequoiaInputSessionBridge.loadSharedPreferences()?[
      KeyboardLayoutPreference.shuangpinKeyHintsDocumentKey] as? Bool
    addTeardownBlock {
      KeyboardLayoutPreference.saveShuangpinKeyHints(storedDocumentHint ?? KeyboardLayoutPreference.shuangpinKeyHints)
    }

    XCTAssertTrue(KeyboardLayoutPreference.saveShuangpinKeyHints(true))
    let withHint = makeController(.shuangpin)
    withHint.viewWillAppear(false)
    let hint = try bottomHint(of: try key(labelled: "字母 V", in: withHint))
    XCTAssertFalse(hint.isHidden)
    XCTAssertNotEqual(hint.text, "粘贴", "双拼提示在时下滑短名让位")

    XCTAssertTrue(KeyboardLayoutPreference.saveShuangpinKeyHints(false))
    let withoutHint = makeController(.shuangpin)
    withoutHint.viewWillAppear(false)
    XCTAssertEqual(try bottomHint(of: try key(labelled: "字母 V", in: withoutHint)).text, "粘贴",
                   "关掉双拼提示后换回下滑短名")
  }

  // MARK: - 底行按键上下滑

  /// 底行按键默认只有空格上滑上屏候选译文，其余都是「无」；显式存的「无」要压住默认，清掉之后又回到默认。
  func testBottomKeySwipeDefaultsAndStoredBindings() {
    let defaults = KeyboardLayoutPreference.defaults
    let stored = defaults.data(forKey: BottomKeySwipePreference.key)
    addTeardownBlock {
      if let stored { defaults.set(stored, forKey: BottomKeySwipePreference.key) }
      else { defaults.removeObject(forKey: BottomKeySwipePreference.key) }
    }
    BottomKeySwipePreference.reset()
    XCTAssertEqual(BottomKeySwipePreference.binding(key: .space, direction: .up).action, .commitTranslation)
    XCTAssertEqual(BottomKeySwipePreference.binding(key: .space, direction: .down).action, .none)
    for key in [BottomKeySwipeKey.language, .symbols, .emoji, .enter, .punctuation, .period] {
      XCTAssertEqual(BottomKeySwipePreference.binding(key: key, direction: .up).action, .none, key.rawValue)
      XCTAssertEqual(BottomKeySwipePreference.binding(key: key, direction: .down).action, .none, key.rawValue)
    }

    // 空格上滑改成「无」：存下去，不再回落成译文。
    BottomKeySwipePreference.set(LetterSwipeBinding(action: .none), key: .space, direction: .up)
    XCTAssertEqual(BottomKeySwipePreference.binding(key: .space, direction: .up).action, .none)
    XCTAssertEqual(BottomKeySwipePreference.stored(key: .space, direction: .up)?.action, LetterSwipeAction.none)

    // 选了自定义文本却没填：这条配置不算数。
    BottomKeySwipePreference.set(LetterSwipeBinding(action: .text, text: "  "), key: .enter, direction: .down)
    XCTAssertFalse(BottomKeySwipePreference.binding(key: .enter, direction: .down).isActive)
    BottomKeySwipePreference.set(LetterSwipeBinding(action: .paste), key: .punctuation, direction: .down)
    XCTAssertEqual(BottomKeySwipePreference.binding(key: .punctuation, direction: .down).action, .paste)

    BottomKeySwipePreference.clear(key: .space)
    XCTAssertEqual(BottomKeySwipePreference.binding(key: .space, direction: .up).action, .commitTranslation,
                   "恢复默认后空格上滑又是译文")
    BottomKeySwipePreference.clear(key: .punctuation)
    XCTAssertNil(BottomKeySwipePreference.stored(key: .punctuation, direction: .down))
  }

  /// 空格键挂上调好的上下滑：上滑默认是「上屏候选译文」，下滑不接；底行不跟字母键的「滑动输入符号」总开关。
  func testSpaceKeyCarriesTheTranslationSwipe() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let storedTable = defaults.data(forKey: BottomKeySwipePreference.key)
    let storedSymbols = defaults.object(forKey: KeyboardLayoutPreference.swipeSymbolsKey)
    addTeardownBlock {
      if let storedTable { defaults.set(storedTable, forKey: BottomKeySwipePreference.key) }
      else { defaults.removeObject(forKey: BottomKeySwipePreference.key) }
      if let storedSymbols { defaults.set(storedSymbols, forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      else { defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
    }
    BottomKeySwipePreference.reset()
    KeyboardLayoutPreference.swipeSymbols = false

    let controller = makeController(.quanpin)
    controller.viewWillAppear(false)
    let space = try XCTUnwrap(try button("spaceKey", in: controller) as? KeyboardKeyButton)
    XCTAssertEqual(space.swipeActions?(.up), .commitTranslation, "总开关关着时底行照常")
    XCTAssertNil(space.swipeActions?(.down), "下滑默认不接")
    XCTAssertNotNil(space.onSwipeAction)

    BottomKeySwipePreference.set(LetterSwipeBinding(action: .none), key: .space, direction: .up)
    controller.viewWillAppear(false)
    let reset = try XCTUnwrap(try button("spaceKey", in: controller) as? KeyboardKeyButton)
    XCTAssertNil(reset.swipeActions?(.up), "存了「无」之后上滑不接")
  }

  /// 键面底部那行提示：9pt 的 `KeyboardBottomHintLabel`，与 10pt 的右上角标分开。
  private func bottomHint(of key: UIButton) throws -> UILabel {
    try XCTUnwrap(key.subviews.compactMap { $0 as? UILabel }.first { $0.font.pointSize == 9 },
                  "No bottom hint label on this key.")
  }

  // MARK: - 长按选项

  /// 默认只列拼音声调：中文 a/e/i/o/u 各四个，英文同表、另含 ê 与 v/n/m；字母本身、角标和上下标不再列。
  func testLetterHoldDefaultsFollowTheLanguage() {
    XCTAssertEqual(LetterHoldPreference.defaults(letter: "e", chinese: true, shifted: false),
                   ["ē", "é", "ě", "è"])
    XCTAssertEqual(LetterHoldPreference.defaults(letter: "e", chinese: true, shifted: true),
                   ["ē", "é", "ě", "è"], "声调不随大小写变")
    XCTAssertEqual(LetterHoldPreference.defaults(letter: "q", chinese: true, shifted: false), [],
                   "q 没有声调，默认不弹，长按回到角标")
    let english = LetterHoldPreference.defaults(letter: "e", chinese: false, shifted: false)
    XCTAssertEqual(english, ["ê", "ē", "é", "ě", "è"], "英文比中文多一个 ê")
    XCTAssertEqual(LetterHoldPreference.defaults(letter: "v", chinese: true, shifted: false), [],
                   "中文只给 a/e/i/o/u 的声调")
    XCTAssertEqual(LetterHoldPreference.defaults(letter: "v", chinese: false, shifted: false),
                   ["ü", "ǖ", "ǘ", "ǚ", "ǜ"])
    XCTAssertEqual(LetterHoldPreference.defaults(letter: "1", chinese: true, shifted: false), [])
  }

  /// 逐键自定义覆盖默认；空数组是「这个键不弹」，与缺键（跟随默认）分开。
  func testLetterHoldOptionsStoreCustomListsAndEmptyMeansNoPopup() {
    let defaults = KeyboardLayoutPreference.defaults
    let stored = defaults.object(forKey: LetterHoldPreference.key)
    let storedEnabled = defaults.object(forKey: LetterHoldPreference.enabledKey)
    addTeardownBlock {
      if let stored { defaults.set(stored, forKey: LetterHoldPreference.key) }
      else { defaults.removeObject(forKey: LetterHoldPreference.key) }
      if let storedEnabled { defaults.set(storedEnabled, forKey: LetterHoldPreference.enabledKey) }
      else { defaults.removeObject(forKey: LetterHoldPreference.enabledKey) }
    }
    LetterHoldPreference.reset()
    LetterHoldPreference.enabled = true

    XCTAssertEqual(LetterHoldPreference.options(letter: "E", chinese: true, shifted: false),
                   ["ē", "é", "ě", "è"], "没配过时用默认，大小写字母都认")
    LetterHoldPreference.set(["ā", "ā", "ab", " ", "😀", "1"], letter: "e")
    XCTAssertEqual(LetterHoldPreference.stored(letter: "E"), ["ā", "😀", "1"], "单字素、去重、过滤")
    XCTAssertEqual(LetterHoldPreference.options(letter: "e", chinese: false, shifted: false),
                   ["ā", "😀", "1"], "配过之后不再跟语言变")

    LetterHoldPreference.set([], letter: "e")
    XCTAssertEqual(LetterHoldPreference.options(letter: "e", chinese: true, shifted: false), [],
                   "空数组是「不弹」，不能回落成默认")

    LetterHoldPreference.clear(letter: "e")
    XCTAssertNil(LetterHoldPreference.stored(letter: "e"))
    LetterHoldPreference.enabled = false
    XCTAssertEqual(LetterHoldPreference.options(letter: "e", chinese: true, shifted: false), [],
                   "总开关关掉后长按回到输入角标")
  }

  /// 手指到哪项就高亮哪项：按气泡里按钮的真实中心取最近项，并列取靠前的。
  func testHoldOptionHighlightPicksTheNearestRealButtonCenter() {
    let centers: [CGFloat] = [10, 50, 90, 130]
    XCTAssertEqual(KeyboardHoldOptionsLayout.nearestIndex(to: 10, centers: centers), 0)
    XCTAssertEqual(KeyboardHoldOptionsLayout.nearestIndex(to: 29, centers: centers), 0, "靠前一项更近")
    XCTAssertEqual(KeyboardHoldOptionsLayout.nearestIndex(to: 31, centers: centers), 1, "过中点换到后一项")
    XCTAssertEqual(KeyboardHoldOptionsLayout.nearestIndex(to: 30, centers: centers), 0, "正中间取靠前项")
    XCTAssertEqual(KeyboardHoldOptionsLayout.nearestIndex(to: 130, centers: centers), 3)
    XCTAssertEqual(KeyboardHoldOptionsLayout.nearestIndex(to: 999, centers: centers), 3, "越界仍取最近的中心")
    XCTAssertEqual(KeyboardHoldOptionsLayout.nearestIndex(to: 50, centers: []), 0)
  }

  /// 字母键长按弹出后高亮离按下手指最近的选项，按住左右滑动按真实按钮位置换高亮、松手上屏该项；九键长按不接这套。
  func testLetterHoldOptionsHighlightAndSlideToCommit() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let stored = defaults.object(forKey: LetterHoldPreference.key)
    let storedEnabled = defaults.object(forKey: LetterHoldPreference.enabledKey)
    addTeardownBlock {
      if let stored { defaults.set(stored, forKey: LetterHoldPreference.key) }
      else { defaults.removeObject(forKey: LetterHoldPreference.key) }
      if let storedEnabled { defaults.set(storedEnabled, forKey: LetterHoldPreference.enabledKey) }
      else { defaults.removeObject(forKey: LetterHoldPreference.enabledKey) }
    }
    LetterHoldPreference.reset()
    LetterHoldPreference.enabled = true

    let controller = makeController(.quanpin)
    let e = try key(labelled: "字母 E", in: controller)
    let options = try XCTUnwrap(e.holdOptions?())
    e.onHoldOptions?(options)

    let buttons = ["ē", "é", "ě", "è"].compactMap { option in
      nodes(controller.view).first { $0.accessibilityIdentifier == "letterHoldOption-\(option)" } as? UIButton
    }
    XCTAssertEqual(buttons.count, 4)
    let coordinateView = try XCTUnwrap(e.superview)
    func fingerX(_ index: Int) -> CGFloat {
      let button = buttons[index]
      return button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: coordinateView).x
    }
    XCTAssertNotNil(e.onHoldOptionHighlight, "字母键接上滑动高亮")
    XCTAssertNotNil(e.onHoldOptionCommit, "字母键接上松手提交")

    // 按下位置靠近第二项：弹出时高亮第二项，而不是固定第一项。
    e.onHoldOptionHighlight?(CGPoint(x: fingerX(1), y: 0))
    XCTAssertFalse(buttons[0].accessibilityTraits.contains(.selected))
    XCTAssertTrue(buttons[1].accessibilityTraits.contains(.selected), "默认高亮离手指最近的选项")

    // 手指滑到第四项：按真实按钮位置换高亮，第二项取消。
    e.onHoldOptionHighlight?(CGPoint(x: fingerX(3), y: 0))
    XCTAssertFalse(buttons[1].accessibilityTraits.contains(.selected))
    XCTAssertTrue(buttons[3].accessibilityTraits.contains(.selected))

    // 松手提交当前高亮项后气泡收起。
    e.onHoldOptionCommit?()
    XCTAssertNil(nodes(controller.view).first { $0.accessibilityIdentifier == "nineKeyHoldOptions" })
  }

  /// 26 键字母键带着长按选项和弹窗接线；中文 e 弹四项声调，弹窗里有对应的按钮。
  func testLetterKeyLongPressShowsItsOptions() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let stored = defaults.object(forKey: LetterHoldPreference.key)
    let storedEnabled = defaults.object(forKey: LetterHoldPreference.enabledKey)
    addTeardownBlock {
      if let stored { defaults.set(stored, forKey: LetterHoldPreference.key) }
      else { defaults.removeObject(forKey: LetterHoldPreference.key) }
      if let storedEnabled { defaults.set(storedEnabled, forKey: LetterHoldPreference.enabledKey) }
      else { defaults.removeObject(forKey: LetterHoldPreference.enabledKey) }
    }
    LetterHoldPreference.reset()
    LetterHoldPreference.enabled = true

    let controller = makeController(.quanpin)
    let e = try key(labelled: "字母 E", in: controller)
    let options = try XCTUnwrap(e.holdOptions?(), "字母键要带上长按选项")
    XCTAssertEqual(options, ["ē", "é", "ě", "è"])
    XCTAssertNotNil(e.onHoldOptions)
    XCTAssertEqual(e.accessibilityCustomActions?.first?.name, "显示长按选项", "读屏也要能打开同一份选项")

    e.onHoldOptions?(options)
    let popup = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "nineKeyHoldOptions" })
    XCTAssertNotNil(nodes(popup).first { $0.accessibilityIdentifier == "letterHoldOption-é" },
                    "弹窗按选项建按钮")
    XCTAssertNotNil(nodes(controller.view).first { $0.accessibilityIdentifier == "nineKeyHoldBackdrop" })

    // 设成空列表之后这个键不弹，按键自己回到「长按输入角标」。
    LetterHoldPreference.set([], letter: "e")
    XCTAssertEqual(e.holdOptions?(), [])
  }
}

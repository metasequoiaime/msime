import XCTest

/// The keyboard extension driven the way a person drives it: as the system keyboard, inside
/// another view's editor, through the real input system.
///
/// Every other suite in this repo builds `KeyboardViewController` directly in a test host. That
/// exercises the Engine session, the layout and the candidate strip, but it never puts the
/// extension behind `UIInputViewController` in a host app's text field, so nothing covered the
/// part that only exists at run time: the system deciding to load the extension at all, the
/// shared App Group container being reachable from that process, and committed text actually
/// reaching a `UITextDocumentProxy` that belongs to someone else.
///
/// The keyboard has to be enabled in Settings first; on a simulator that means the
/// `AppleKeyboards` global preference. When it is not enabled the tests skip rather than fail -
/// a machine without that setup is not a product defect, and reporting it as one would bury the
/// runs where it is set up and something really did break.
@MainActor
final class KeyboardExtensionEditorUITests: XCTestCase {
  private let keyboardIdentifier = "app.msime.ios.keyboard"

  override func setUp() {
    super.setUp()
    continueAfterFailure = false
  }

  /// Add the keyboard in Settings, the way a person does, if it is not already there.
  ///
  /// Writing `AppleKeyboards` into the simulator's global preferences looks like it should be
  /// enough and is not: iOS 27 rebuilds the ring from its own store, and the written entries -
  /// including the stock Pinyin one - never appear. Enabling the extension with `pluginkit` is
  /// also not the gate; it stays enabled and the ring is unchanged. Settings is what actually
  /// registers a third-party keyboard, so the test walks it.
  ///
  /// Every step is optional: a run where the keyboard is already added skips straight through,
  /// and a Settings layout this does not recognise leaves the ring alone rather than failing here
  /// - the caller's skip then says the keyboard was never reachable, which is the honest report.
  private func addKeyboardInSettingsIfNeeded() {
    let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
    settings.launch()
    defer { settings.terminate() }

    guard settings.wait(for: .runningForeground, timeout: 15) else { return }

    func row(_ label: String) -> XCUIElement {
      let cell = settings.cells.staticTexts[label].firstMatch
      return cell.exists ? cell : settings.buttons[label].firstMatch
    }
    func tapRow(_ label: String, scrolling: Bool = true) -> Bool {
      let target = row(label)
      guard target.waitForExistence(timeout: 6) else { return false }
      if scrolling { for _ in 0..<8 where !target.isHittable { settings.swipeUp() } }
      guard target.isHittable else { return false }
      target.tap()
      return true
    }

    // 键盘 has been a top-level row and a row under 通用 in different releases, and the
    // App-prefs deep link does not navigate on this runtime. Try where it is now, then where it
    // used to be, rather than pinning one layout.
    if !tapRow("键盘") {
      guard tapRow("通用"), tapRow("键盘") else { return }
    }
    // 有些布局里面板和其中的列表标题相同。iOS 27 上列表行读作「键盘、<数量>」，按标签匹配不到，改用它的标识符 `KEYBOARDS`。
    let ring = settings.cells["KEYBOARDS"].firstMatch
    if ring.waitForExistence(timeout: 5) { ring.tap() } else { _ = tapRow("键盘", scrolling: false) }
    // Already added: the keyboard list names it.
    if settings.staticTexts["水杉输入法"].waitForExistence(timeout: 3) { return }

    // iOS 27 上这一行写作「添加新键盘」，没有省略号，而且是按钮不是文本；只按前缀找，哪种都认。原来写死「添加新键盘…」找不到它，于是键盘从未被添加，本套件在 iOS 27 上每次都整体跳过。
    let add = settings.descendants(matching: .any)
      .matching(NSPredicate(format: "label BEGINSWITH %@", "添加新键盘")).firstMatch
    guard add.waitForExistence(timeout: 8) else { return }
    add.tap()

    let ours = settings.descendants(matching: .any)
      .matching(NSPredicate(format: "label BEGINSWITH %@", "水杉输入法")).firstMatch
    for _ in 0..<8 where !ours.exists { settings.swipeUp() }
    guard ours.waitForExistence(timeout: 8) else { return }
    ours.tap()

    // Full access is a second, separate confirmation. The keyboard runs without it; only the
    // features that need a network or the shared container do.
    let entry = settings.staticTexts["水杉输入法"].firstMatch
    if entry.waitForExistence(timeout: 5) {
      entry.tap()
      let fullAccess = settings.switches["允许完全访问"].firstMatch
      if fullAccess.waitForExistence(timeout: 5), fullAccess.value as? String == "0" {
        // iOS 27 上开关占满整行，点中心会落在标签上而不会切换，所以点行尾的开关圆钮。
        fullAccess.coordinate(withNormalizedOffset: CGVector(dx: 0.94, dy: 0.5)).tap()
        let allow = settings.alerts.buttons["允许"].firstMatch
        if allow.waitForExistence(timeout: 5) { allow.tap() }
      }
    }
  }

  /// 聚焦原生编辑器，再通过系统键盘列表或地球键切换到水杉。
  /// 新编辑器的初始键盘由系统决定，列表选择失败时轮转一圈确认扩展是否可用。
  private func focusedTryoutKeyboard(_ app: XCUIApplication, toolbarTest: Bool = false) throws -> XCUIElement {
    #if targetEnvironment(simulator)
    // 首次安装后先启动宿主，让系统完成键盘扩展注册。
    app.launch()
    app.terminate()
    addKeyboardInSettingsIfNeeded()
    #endif
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    if toolbarTest { app.launchArguments.append("--keyboard-toolbar-ui-test") }
    app.launch()
    let settings = app.tabBars.buttons["设置"]
    XCTAssertTrue(settings.waitForExistence(timeout: 10))
    settings.tap()
    settings.tap()
    let tryout = app.buttons["keyboardTryoutLink"]
    XCTAssertTrue(tryout.waitForExistence(timeout: 20), "试用入口必须可用")
    tryout.tap()

    let field = app.textFields["keyboardTryoutField"]
    XCTAssertTrue(field.waitForExistence(timeout: 15), "试用编辑器必须可用")
    if toolbarTest { field.tap() }
    // This screen focuses the field on appearance; tapping an already focused field while the
    // keyboard animates can land on a key instead.
    let focused = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "hasKeyboardFocus == true"), object: field)
    if XCTWaiter.wait(for: [focused], timeout: 10) != .completed { field.tap() }

    // 品牌按钮不随方案或自动大写改变，用它确认当前是水杉键盘。
    func readyKeyboard() -> XCUIElement {
      if toolbarTest {
        let back = app.buttons["dismissShortcut"]
        if back.exists && back.label == "返回键盘" { back.tap() }
      }
      return field
    }
    let ours = app.buttons["moreShortcut"]
    if ours.waitForExistence(timeout: 6) { return readyKeyboard() }

    // 沿键盘列表轮转一圈。地球键没有标识符，只有随系统语言变化的标签：英文是 `Next keyboard`，中文是「下一个键盘」。
    let globe = app.buttons.matching(NSPredicate(format: "label IN %@", ["Next keyboard", "下一个键盘"])).firstMatch
    if globe.waitForExistence(timeout: 6) {
      // 从系统列表直接选中扩展，避免同名系统键盘影响轮转。
      globe.press(forDuration: 1)
      let entry = app.descendants(matching: .any)
        .matching(NSPredicate(format: "label BEGINSWITH %@", "水杉输入法")).firstMatch
      if entry.waitForExistence(timeout: 3) {
        entry.tap()
        if ours.waitForExistence(timeout: 6) { return readyKeyboard() }
      }
      // 全新的 iOS 27 模拟器默认启用十四个键盘，其中几个显示名相同，所以轮转次数要覆盖整圈，而不是只点寥寥几次。
      for _ in 0..<20 {
        globe.tap()
        // 新模拟器第一次切换键盘会出现系统引导，先关闭它再检查扩展。
        for host in [app, XCUIApplication(bundleIdentifier: "com.apple.springboard")] {
          let next = host.buttons.matching(NSPredicate(format: "label IN %@", ["Continue", "继续"])).firstMatch
          if next.exists { next.tap() }
        }
        if ours.waitForExistence(timeout: 3) { return readyKeyboard() }
      }
    }
    // A skip that only says "not enabled" is indistinguishable from a keyboard that loaded and
    // then crashed, so record what was actually on screen before giving up.
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = "Keyboard not reached"
    shot.lifetime = .keepAlways
    add(shot)
    let keys = app.keyboards.count
    let buttons = app.keyboards.buttons.allElementsBoundByIndex.prefix(12)
      .map { "\($0.identifier)|\($0.label)" }.joined(separator: ", ")
    throw XCTSkip(
      "\(keyboardIdentifier) never became the active keyboard. keyboards=\(keys) globe="
        + "\(globe.exists) keys=[\(buttons)]")
  }

  /// 在系统加载的扩展里点按英文按键，校验原文、默认纠错和明确拒绝纠错。
  func testEnglishCorrectionCommitsThroughTheHostDocument() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app, toolbarTest: true)
    app.buttons["schemeButton"].tap()
    let quanpin = app.buttons["schemeCard-quanpin"]
    XCTAssertTrue(quanpin.waitForExistence(timeout: 5))
    quanpin.tap()
    let language = app.buttons["bottomLanguageKey"]
    if language.label == "切换到英文输入" { language.tap() }
    XCTAssertEqual(language.value as? String, "英文输入")
    var committed = (field.value as? String) ?? ""
    if committed == "输入消息，试试键盘" { committed = "" }
    for (raw, corrected) in [("whatisthat", "what is that"), ("langiage", "language"),
                             ("imchinese", "I'm Chinese"), ("speskebglish", "speak English"),
                             ("hello", "hello"), ("thx", "thx")] {
      for letter in raw { app.buttons["字母 \(String(letter).uppercased())"].tap() }
      XCTAssertEqual(field.value as? String, committed + raw, "按键必须先按原文输入")
      let original = app.buttons["candidate-1"]
      XCTAssertTrue(original.waitForExistence(timeout: 5))
      XCTAssertTrue(original.label.contains(raw))
      if raw != corrected {
        XCTAssertTrue(app.buttons["candidate-2"].label.contains(corrected))
      } else {
        XCTAssertFalse(app.buttons["candidate-2"].exists, "正确词不提供普通前缀补全")
      }
      app.buttons["spaceKey"].tap()
      committed += corrected + " "
      XCTAssertEqual(field.value as? String, committed)
    }
    for letter in "languge" { app.buttons["字母 \(String(letter).uppercased())"].tap() }
    app.buttons["candidate-1"].tap()
    app.buttons["spaceKey"].tap()
    XCTAssertEqual(field.value as? String, committed + "languge ", "点原文后空格不能重新纠错")
    committed += "languge "
    for letter in "languge" { app.buttons["字母 \(String(letter).uppercased())"].tap() }
    app.buttons["bottomPeriodKey"].tap()
    committed += "language. "
    XCTAssertEqual(field.value as? String, committed, "标点紧随纠错词，随后追加 ASCII 空格")
    app.buttons["shiftButton"].tap()
    app.buttons["大写 L"].tap()
    for letter in "anguaage" { app.buttons["字母 \(String(letter).uppercased())"].tap() }
    app.buttons["spaceKey"].tap()
    XCTAssertEqual(field.value as? String, committed + "Language ", "手动首字母大写必须保留")
  }

  /// 从系统键盘真实点按工具栏，确认每次只打开或关闭一次面板。
  func testToolbarTapsTogglePanelsInHostEditor() throws {
    let app = XCUIApplication()
    _ = try focusedTryoutKeyboard(app, toolbarTest: true)
    let scheme = app.buttons["schemeButton"]
    XCTAssertTrue(scheme.isHittable, "顶部方案按钮必须能被系统命中")
    scheme.tap()
    let nineKey = app.buttons["schemeCard-nineKey"]
    XCTAssertTrue(nineKey.waitForExistence(timeout: 5), "九键方案必须可用")
    nineKey.tap()
    for (buttonID, panelID) in [
      ("moreShortcut", "keyboardMorePicker"),
      ("emojiShortcut", "keyboardEmojiPicker"),
      ("skinShortcut", "keyboardSkinPicker"),
      ("schemeButton", "keyboardSchemePicker"),
    ] {
      let button = app.buttons[buttonID]
      let panel = app.descendants(matching: .any)[panelID].firstMatch
      XCTAssertTrue(button.waitForExistence(timeout: 5), buttonID)
      XCTAssertTrue(button.isHittable, "工具栏必须能被系统命中：\(buttonID)")
      for x in [0.02, 0.5, 0.98] {
        let point = button.coordinate(withNormalizedOffset: CGVector(dx: x, dy: 0.5))
        point.tap()
        XCTAssertTrue(panel.waitForExistence(timeout: 5), "点按应打开面板：\(buttonID)")
        point.tap()
        let closed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: panel)
        XCTAssertEqual(XCTWaiter.wait(for: [closed], timeout: 5), .completed, "再次点按应关闭面板：\(buttonID)")
      }
    }
  }

  /// 添加语言必须进入宿主的输入设置，不能只把键盘面板关掉。
  func testAddLanguageOpensInputSettingsFromKeyboard() throws {
    let app = XCUIApplication()
    _ = try focusedTryoutKeyboard(app, toolbarTest: true)
    app.buttons["schemeButton"].tap()
    XCTAssertTrue(app.descendants(matching: .any)["keyboardSchemePicker"].firstMatch.waitForExistence(timeout: 5))
    let add = app.buttons["schemeAddLanguageCard"]
    let grid = app.descendants(matching: .any)["schemePickerScroll"].firstMatch
    for _ in 0..<5 where !add.isHittable { grid.swipeLeft() }
    XCTAssertTrue(add.isHittable, "添加语言卡片必须可点击")
    add.tap()
    XCTAssertTrue(app.navigationBars["输入"].waitForExistence(timeout: 8),
                  "添加语言应打开应用的输入设置")
  }

  /// 数字页只接受字面输入，不能在结束拼音时悄悄选中首选候选。
  func testNumberPadCommitsRawCompositionBeforeEveryLiteral() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app)
    app.buttons["schemeButton"].tap()
    let card = app.buttons["schemeCard-quanpin"]
    XCTAssertTrue(card.waitForExistence(timeout: 5))
    card.tap()
    func key(_ id: String) -> XCUIElement {
      app.descendants(matching: .any)[id].firstMatch
    }
    var expected = ""
    for (identifier, literal) in [("nineKey1", "1"), ("nineKeyZero", "0"),
                                  ("nineKeyMiddleKey", "."), ("运算符 +", "+"),
                                  ("运算符 -", "-"), ("运算符 *", "*"), ("运算符 /", "/")] {
      key("字母 N").tap()
      key("字母 I").tap()
      XCTAssertTrue(app.buttons["candidate-1"].exists, "必须先建立可选词的拼音组合")
      key("layoutToggleButton").tap()
      let target = key(identifier)
      revealNumberPadSidebarKey(target, in: app)
      XCTAssertTrue(target.isHittable)
      target.tap()
      expected += "ni" + literal
      XCTAssertEqual(field.value as? String, expected, "数字页必须原样结束组合，再写入 \(literal)")
      XCTAssertTrue(key("numberPadSymbolPanelKey").isHittable, "输入后必须留在数字页")
      key("layoutToggleButton").tap()
    }
  }

  /// 常规中文和原九键的数字页在系统编辑器里连续输入，扩展符号往返不丢失原方案。
  func testNumberPadTypesContinuousArithmeticIntoTheHostEditor() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app)
    func key(_ id: String) -> XCUIElement {
      app.descendants(matching: .any)[id].firstMatch
    }
    var expected = ""
    for scheme in ["quanpin", "nineKey"] {
      app.buttons["schemeButton"].tap()
      let card = app.buttons["schemeCard-\(scheme)"]
      XCTAssertTrue(card.waitForExistence(timeout: 5), "默认中文方案必须可用：\(scheme)")
      card.tap()
      let toggle = key("layoutToggleButton")
      XCTAssertTrue(toggle.isHittable)
      toggle.tap()
      for digit in 1...9 {
        XCTAssertTrue(key("nineKey\(digit)").isHittable, "数字 \(digit) 必须可点")
      }
      let one = key("nineKey1").frame
      let two = key("nineKey2").frame
      let four = key("nineKey4").frame
      XCTAssertEqual(one.minY, two.minY, accuracy: 1)
      XCTAssertEqual(one.minX, four.minX, accuracy: 1)
      XCTAssertLessThan(one.maxX, two.minX)
      XCTAssertLessThan(one.maxY, four.minY)
      let zero = key("nineKeyZero").frame
      let space = key("spaceKey").frame
      XCTAssertGreaterThanOrEqual(zero.width, key("nineKey8").frame.width)
      if !key("inputModeSwitchButton").exists {
        XCTAssertEqual(zero.minX, key("nineKey7").frame.minX, accuracy: 1)
        XCTAssertEqual(zero.maxX, key("nineKey7").frame.maxX, accuracy: 1)
        XCTAssertEqual(space.minX, key("nineKey8").frame.minX, accuracy: 1)
        XCTAssertEqual(space.maxX, key("nineKey8").frame.maxX, accuracy: 1)
      }
      XCTAssertLessThan(zero.maxX, space.minX)
      for character in "12.3+4-5*6/7001234567890123456789" {
        let target: XCUIElement
        switch character {
        case "0": target = key("nineKeyZero")
        case "1"..."9": target = key("nineKey\(character)")
        case ".": target = key("nineKeyMiddleKey")
        default: target = app.buttons["运算符 \(character)"]
        }
        revealNumberPadSidebarKey(target, in: app)
        XCTAssertTrue(target.isHittable, "数字页必须能输入 \(character)")
        target.tap()
        expected.append(character)
      }
      XCTAssertEqual(field.value as? String, expected, "数字与运算符必须按字面跨进程上屏")
      key("nineKeyDelete").tap()
      expected.removeLast()
      XCTAssertEqual(field.value as? String, expected)
      let symbols = key("numberPadSymbolPanelKey")
      XCTAssertTrue(symbols.isHittable, "滚动后符号入口仍固定可点")
      symbols.tap()
      XCTAssertTrue(key("keyboardSymbolPanel").waitForExistence(timeout: 5))
      key("closeSymbolPanel").tap()
      XCTAssertTrue(key("nineKeyZero").isHittable)
      let more = key("numberPadMoreSymbols")
      revealNumberPadSidebarKey(more, in: app)
      more.tap()
      XCTAssertTrue(key("symbolLayerToggle").waitForExistence(timeout: 5))
      key("symbolLayerToggle").tap()
      XCTAssertTrue(key("nineKey1").isHittable)
      toggle.tap()
      XCTAssertTrue((scheme == "quanpin" ? key("字母 A") : key("nineKey2")).isHittable,
                    "返回后必须恢复原中文方案")
    }
  }

  /// 侧栏每项保持数字排高度，屏幕外的运算符通过真实滑动触达。
  private func revealNumberPadSidebarKey(_ key: XCUIElement, in app: XCUIApplication) {
    let scroll = app.scrollViews["numberPadOperatorScroll"]
    guard scroll.exists else { return }
    for _ in 0..<4 where !key.isHittable { scroll.swipeDown() }
    for _ in 0..<4 where !key.isHittable { scroll.swipeUp() }
  }

  /// 用系统键盘在真实编辑器中验收全拼分词和英文大写，覆盖跨进程上屏。
  func testQuanpinSeparatorCommitsXianAndRestoresEnglishShift() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app)
    app.buttons["schemeButton"].tap()
    let card = app.buttons["schemeCard-quanpin"]
    XCTAssertTrue(card.waitForExistence(timeout: 5))
    card.tap()
    let separator = app.buttons["shiftButton"]
    XCTAssertTrue(separator.waitForExistence(timeout: 5))
    XCTAssertEqual(separator.label, "拼音分词")
    let idle = field.value as? String
    separator.tap()
    XCTAssertEqual(field.value as? String, idle)
    func letter(_ label: String) throws {
      let key = app.descendants(matching: .any)[label].firstMatch
      XCTAssertTrue(key.waitForExistence(timeout: 5), label)
      key.tap()
    }
    try letter("字母 X")
    try letter("字母 I")
    separator.tap()
    try letter("字母 A")
    try letter("字母 N")
    let candidate = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'candidate-' AND label CONTAINS %@", "西安")).firstMatch
    XCTAssertTrue(candidate.waitForExistence(timeout: 8))
    candidate.tap()
    XCTAssertEqual(field.value as? String, "西安")
    app.buttons["bottomLanguageKey"].tap()
    XCTAssertNotEqual(separator.label, "拼音分词")
    separator.tap()
    try letter("大写 A")
    XCTAssertEqual(field.value as? String, "西安A")
    app.buttons["bottomLanguageKey"].tap()
    XCTAssertEqual(separator.label, "拼音分词")
  }

  /// Pinyin typed on the real keyboard reaches a text field the extension does not own.
  ///
  /// The candidate is committed through `UITextDocumentProxy`, which is a cross-process call the
  /// in-host suites never make: there, `render` writes into a proxy the test itself provided.
  func testTypingPinyinCommitsCandidateTextIntoTheHostEditor() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app)

    for letter in ["N", "I", "H", "A", "O"] {
      let key = app.keys["字母 \(letter)"]
      XCTAssertTrue(key.waitForExistence(timeout: 5), "字母 \(letter) is missing from the key face")
      key.tap()
    }

    // The strip names its chips; the first is the Engine's leading candidate.
    let leading = app.buttons["candidate-1"]
    XCTAssertTrue(leading.waitForExistence(timeout: 8),
                  "the candidate strip stayed empty for nihao — the Engine session did not answer "
                    + "inside the extension process")
    let candidate = leading.label
    leading.tap()

    let value = (field.value as? String) ?? ""
    XCTAssertFalse(value.isEmpty, "nothing reached the host editor")
    XCTAssertTrue(value.contains(candidate) || candidate.contains(value),
                  "the editor holds \(value) but the chip that was tapped said \(candidate)")
  }

  /// 从原生设置保存方案，再由系统加载扩展，在真实编辑器里用九键双拼提交候选。
  func testNineKeyShuangpinTypesIntoTheHostEditor() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["设置"].tap()
    let input = app.buttons["inputSettingsLink"].firstMatch
    for _ in 0..<6 where !input.isHittable { app.swipeUp() }
    XCTAssertTrue(input.waitForExistence(timeout: 10))
    input.tap()
    let settings = app.buttons["nineKeyShuangpinSettingsLink"]
    XCTAssertTrue(settings.waitForExistence(timeout: 10))
    settings.tap()
    let save = app.buttons["nineKeyShuangpinSave"]
    for _ in 0..<8 where !save.isHittable { app.swipeUp() }
    XCTAssertTrue(save.isEnabled)
    save.tap()
    XCTAssertTrue(app.staticTexts["已保存，键盘将使用这张方案表"].waitForExistence(timeout: 5))
    app.terminate()

    let field = try focusedTryoutKeyboard(app)
    XCTAssertTrue((app.buttons["schemeButton"].value as? String)?.contains("九键双拼") == true)
    XCTAssertTrue(app.buttons["nineKeyZero"].exists)
    for digit in "6447" {
      let key = app.buttons["nineKey\(digit)"]
      XCTAssertTrue(key.waitForExistence(timeout: 5))
      key.tap()
    }
    let shown = (1...5).map { app.buttons["candidate-\($0)"] }.filter(\.exists)
    let nihao = try XCTUnwrap(shown.first { $0.label.contains("你好") }, "九键双拼应给出你好候选")
    nihao.tap()
    XCTAssertTrue((field.value as? String ?? "").contains("你好"))
  }

  /// 全拼 14 键当成系统键盘来用：点方案卡片切到 14 键，按 BN UI GH AS OP 打出「你好」，从候选条上选它上屏到别人的编辑框里。bu 和 ni 同组码，锁定词库里「不好」的权重高于「你好」，所以只要求「你好」在前几个候选里，与 Android 设备验收和 FourteenKeyKeyboardTests 相同。14 键要用户自己打开，没打开时跳过。
  func testFourteenKeysTypeNihaoIntoTheHostEditor() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app)

    let scheme = app.buttons["schemeButton"]
    XCTAssertTrue(scheme.waitForExistence(timeout: 5), "the scheme button is missing")
    scheme.tap()
    let card = app.buttons["schemeCard-fourteenKey"]
    guard card.waitForExistence(timeout: 5) else {
      throw XCTSkip("全拼 14 键 is not among the enabled schemes on this device.")
    }
    card.tap()

    for label in ["按键 B N", "按键 U I", "按键 G H", "按键 A S", "按键 O P"] {
      let key = app.buttons[label].exists ? app.buttons[label] : app.keys[label]
      XCTAssertTrue(key.waitForExistence(timeout: 5), "\(label) is missing from the 14-key face")
      key.tap()
    }
    XCTAssertTrue(app.buttons["candidate-1"].waitForExistence(timeout: 8), "the candidate strip stayed empty for the 14-key nihao")
    let shown = (1...3).map { app.buttons["candidate-\($0)"] }.filter(\.exists)
    let nihao = try XCTUnwrap(shown.first { $0.label.contains("你好") },
                              "你好 is not among the first candidates: \(shown.map(\.label))")
    nihao.tap()
    let value = (field.value as? String) ?? ""
    XCTAssertTrue(value.contains("你好"), "the editor holds \(value)")
  }

  /// The double-pinyin face labels its keys with the units the running profile puts on them.
  ///
  /// These hints are read out of the Engine through the shared ABI rather than from a table kept
  /// in the host, which is only true at run time: the strings do not exist in the binary, so no
  /// static check can see this. Xiaohe's K carries both `ing` and `uai`, and a host-side copy had
  /// lost the second one.
  func testDoublePinyinKeyFaceCarriesTheEngineHints() throws {
    let app = XCUIApplication()
    _ = try focusedTryoutKeyboard(app)

    let scheme = app.buttons["schemeButton"]
    XCTAssertTrue(scheme.waitForExistence(timeout: 5), "the scheme button is missing")
    scheme.tap()

    let card = app.buttons["schemeCard-shuangpin"]
    guard card.waitForExistence(timeout: 5) else {
      throw XCTSkip("Double pinyin is not among the enabled schemes on this device.")
    }
    card.tap()

    // The hint rides on the key's accessibility value, so it is readable without pixels.
    let k = app.keys["字母 K"]
    XCTAssertTrue(k.waitForExistence(timeout: 5), "字母 K is missing from the double-pinyin face")
    let hint = (k.value as? String) ?? ""
    XCTAssertTrue(hint.contains("uai"),
                  "K reads \(hint.isEmpty ? "nothing" : hint) but Xiaohe types uai on it")
    XCTAssertTrue(hint.contains("ing"), "K reads \(hint) but Xiaohe types ing on it too")
  }
}

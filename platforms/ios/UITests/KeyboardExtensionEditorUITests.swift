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

  /// 失败时保留实际界面，区分入口、遮挡和产品行为，不能只看断言行号。
  private func keepScreenshot(_ name: String, in app: XCUIApplication) {
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = name
    shot.lifetime = .keepAlways
    add(shot)
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
    // 已登记的扩展仍需核对完全访问，不能在这里提前返回。
    if !settings.staticTexts["水杉输入法"].waitForExistence(timeout: 3) {
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
    }

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
    let settings = app.tabBars.buttons["首页"]
    XCTAssertTrue(settings.waitForExistence(timeout: 10))
    settings.tap()
    settings.tap()
    let tryout = app.buttons["keyboardTryoutLink"]
    XCTAssertTrue(tryout.waitForExistence(timeout: 20), "试用入口必须可用")
    tryout.tap()

    let field = app.textFields["keyboardTryoutField"]
    XCTAssertTrue(field.waitForExistence(timeout: 15), "试用编辑器必须可用")
    if toolbarTest { field.tap() }
    return try activeSystemKeyboard(in: app, field: field, toolbarTest: toolbarTest)
  }

  /// 共用系统键盘识别与切换流程，编辑器可以属于产品宿主或独立测试应用。
  private func activeSystemKeyboard(in app: XCUIApplication, field: XCUIElement,
                                    toolbarTest: Bool = false) throws -> XCUIElement {
    // 编辑器出现时自动聚焦，键盘动画期间再次点击可能误触按键。
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
    func waitForKeyboard(timeout: TimeInterval) throws -> Bool {
      let ready = ours.waitForExistence(timeout: timeout)
      let sharedDirectoryError = app.staticTexts["无法访问键盘共享目录，请检查完全访问权限。"]
      if !ready && sharedDirectoryError.exists {
        keepScreenshot("键盘共享目录不可用", in: app)
        // 扩展已经加载，不能把权限或签名错误当作未启用键盘而跳过。
        throw NSError(domain: "KeyboardExtensionEditorUITests", code: 1,
                      userInfo: [NSLocalizedDescriptionKey: sharedDirectoryError.label])
      }
      return ready
    }
    if try waitForKeyboard(timeout: 6) { return readyKeyboard() }

    // 沿键盘列表轮转一圈。地球键没有标识符，只有随系统语言变化的标签：英文是 `Next keyboard`，中文是「下一个键盘」。
    let globe = app.buttons.matching(NSPredicate(format: "label IN %@", ["Next keyboard", "下一个键盘"])).firstMatch
    if globe.waitForExistence(timeout: 6) {
      // 从系统列表直接选中扩展，避免同名系统键盘影响轮转。
      globe.press(forDuration: 1)
      let entry = app.descendants(matching: .any)
        .matching(NSPredicate(format: "label BEGINSWITH %@", "水杉输入法")).firstMatch
      if entry.waitForExistence(timeout: 3) {
        entry.tap()
        if try waitForKeyboard(timeout: 6) { return readyKeyboard() }
      }
      // 全新的 iOS 27 模拟器默认启用十四个键盘，其中几个显示名相同，所以轮转次数要覆盖整圈，而不是只点寥寥几次。
      for _ in 0..<20 {
        globe.tap()
        // 新模拟器第一次切换键盘会出现系统引导，先关闭它再检查扩展。
        for host in [app, XCUIApplication(bundleIdentifier: "com.apple.springboard")] {
          let next = host.buttons.matching(NSPredicate(format: "label IN %@", ["Continue", "继续"])).firstMatch
          if next.exists { next.tap() }
        }
        if try waitForKeyboard(timeout: 3) { return readyKeyboard() }
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

  /// 关闭语音长按后，经系统空格手势检查独立编辑器里的光标与松手行为。
  func testHoldingSpaceWithoutVoiceMovesCaretInSystemEditor() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    var original: [String: String] = [:]
    func configure(restoring: Bool = false) {
      app.terminate()
      app.launch()
      let settings = app.tabBars.buttons["首页"]
      XCTAssertTrue(settings.waitForExistence(timeout: 10))
      settings.tap()
      let layout = app.buttons["keyboardLayoutLink"]
      for _ in 0..<8 where !layout.isHittable { app.swipeUp() }
      XCTAssertTrue(layout.waitForExistence(timeout: 5))
      layout.tap()
      for identifier in ["appGlideTypingSwitch", "appCursorSwipeSwitch", "appSpaceVoiceSwitch"] {
        if restoring && original[identifier] == nil { continue }
        let toggle = app.switches[identifier]
        for _ in 0..<8 where !toggle.isHittable {
          let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.84))
          start.press(forDuration: 0.05, thenDragTo:
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.6)))
        }
        XCTAssertTrue(toggle.exists && toggle.isHittable, identifier)
        if original[identifier] == nil { original[identifier] = toggle.value as? String }
        let value = restoring ? original[identifier, default: "0"] : "0"
        if toggle.value as? String != value {
          toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
        }
        let saved = XCTNSPredicateExpectation(
          predicate: NSPredicate(format: "value == %@", value), object: toggle)
        XCTAssertEqual(XCTWaiter.wait(for: [saved], timeout: 5), .completed)
      }
      app.terminate()
    }
    addTeardownBlock { @MainActor in configure(restoring: true) }
    configure()
    addKeyboardInSettingsIfNeeded()
    let editor = XCUIApplication(bundleIdentifier: "app.msime.ios.keyboard-test-host")
    editor.launchArguments = ["--system-keyboard-editor"]
    addTeardownBlock { @MainActor in editor.terminate() }
    editor.launch()
    let field = editor.textFields["systemKeyboardEditor"]
    XCTAssertTrue(field.waitForExistence(timeout: 10))
    _ = try activeSystemKeyboard(in: editor, field: field)
    editor.buttons["schemeButton"].tap()
    let quanpin = editor.buttons["schemeCard-quanpin"]
    XCTAssertTrue(quanpin.waitForExistence(timeout: 5))
    quanpin.tap()
    let language = editor.buttons["bottomLanguageKey"]
    if language.label == "切换到英文输入" { language.tap() }
    XCTAssertEqual(language.value as? String, "英文输入")
    func key(_ letter: String) -> XCUIElement {
      editor.buttons.matching(NSPredicate(format: "label IN %@",
        ["字母 \(letter.uppercased())", "字母 \(letter.lowercased())"])).firstMatch
    }
    let text = String(repeating: "abc", count: 10)
    for letter in text { key(String(letter)).tap() }
    XCTAssertEqual(field.value as? String, text)
    let candidate = editor.buttons["candidate-1"]
    if candidate.exists { candidate.tap() }
    let space = editor.buttons["spaceKey"]
    XCTAssertTrue(space.isHittable)
    space.press(forDuration: 0.7)
    XCTAssertEqual(field.value as? String, text, "长按松手不能插入空格")
    let start = space.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
    start.press(forDuration: 0.7, thenDragTo: start.withOffset(CGVector(dx: -48, dy: 0)),
                withVelocity: 120, thenHoldForDuration: 0.1)
    XCTAssertEqual(field.value as? String, text, "移动光标不能修改已有文字")
    key("z").tap()
    let left = try XCTUnwrap(field.value as? String)
    XCTAssertEqual(left.replacingOccurrences(of: "z", with: ""), text)
    XCTAssertFalse(left.hasPrefix("z"))
    XCTAssertFalse(left.hasSuffix("z"), "长按左拖必须把真实文档光标移到文本内部")
    keepScreenshot("SpaceCursor-Left", in: editor)
    start.press(forDuration: 0.7, thenDragTo: start.withOffset(CGVector(dx: 24, dy: 0)),
                withVelocity: 120, thenHoldForDuration: 0.1)
    XCTAssertEqual(field.value as? String, left)
    key("v").tap()
    let right = try XCTUnwrap(field.value as? String)
    XCTAssertEqual(right.replacingOccurrences(of: "v", with: ""), left)
    let previous = try XCTUnwrap(right.firstIndex(of: "z"))
    let current = try XCTUnwrap(right.firstIndex(of: "v"))
    XCTAssertGreaterThan(right.distance(from: previous, to: current), 1,
                         "长按右拖必须越过左拖后的插入位置")
    keepScreenshot("SpaceCursor-Right", in: editor)
  }

  /// 从真实系统扩展横滑后插入标记，观察宿主文档的光标位置及起始按键取消。
  func testCursorSwipeMovesInSystemEditorAndPreservesTaps() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    var original: [String: String] = [:]
    func configure(_ enabled: Bool, restoring: Bool = false) {
      app.terminate()
      app.launch()
      app.tabBars.buttons["首页"].tap()
      let layout = app.buttons["keyboardLayoutLink"]
      for _ in 0..<8 where !layout.isHittable { app.swipeUp() }
      XCTAssertTrue(layout.waitForExistence(timeout: 5))
      layout.tap()
      for (identifier, desired) in [("appGlideTypingSwitch", "0"),
                                     ("appCursorSwipeSwitch", enabled ? "1" : "0")] {
        if restoring && original[identifier] == nil { continue }
        let toggle = app.switches[identifier]
        for _ in 0..<8 where !toggle.isHittable {
          let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.84))
          start.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.6)))
        }
        XCTAssertTrue(toggle.exists && toggle.isHittable)
        if original[identifier] == nil { original[identifier] = toggle.value as? String }
        let value = restoring ? original[identifier, default: desired] : desired
        if toggle.value as? String != value {
          toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
        }
        let saved = XCTNSPredicateExpectation(
          predicate: NSPredicate(format: "value == %@", value), object: toggle)
        XCTAssertEqual(XCTWaiter.wait(for: [saved], timeout: 5), .completed)
      }
      app.terminate()
    }
    addTeardownBlock { @MainActor in configure(false, restoring: true) }
    configure(true)
    app.launch()
    app.terminate()
    addKeyboardInSettingsIfNeeded()
    let editor = XCUIApplication(bundleIdentifier: "app.msime.ios.keyboard-test-host")
    editor.launchArguments = ["--system-keyboard-editor"]
    addTeardownBlock { @MainActor in editor.terminate() }

    func key(_ letter: String) -> XCUIElement {
      editor.buttons.matching(NSPredicate(format: "label IN %@",
        ["字母 \(letter.uppercased())", "字母 \(letter.lowercased())"])).firstMatch
    }
    for enabled in [true, false] {
      if !enabled { configure(false) }
      editor.launch()
      let field = editor.textFields["systemKeyboardEditor"]
      XCTAssertTrue(field.waitForExistence(timeout: 10))
      _ = try activeSystemKeyboard(in: editor, field: field)
      editor.buttons["schemeButton"].tap()
      let quanpin = editor.buttons["schemeCard-quanpin"]
      XCTAssertTrue(quanpin.waitForExistence(timeout: 5))
      quanpin.tap()
      let language = editor.buttons["bottomLanguageKey"]
      if language.label == "切换到英文输入" { language.tap() }
      XCTAssertEqual(language.value as? String, "英文输入")
      let text = String(repeating: "abc", count: 10)
      for letter in text { key(String(letter)).tap() }
      XCTAssertEqual(field.value as? String, text, "连续点按不能丢字或移动光标")
      let candidate = editor.buttons["candidate-1"]
      if candidate.exists { candidate.tap() }
      let start = key("L").coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
      let end = key("G").coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
      start.press(forDuration: 0.05, thenDragTo: end, withVelocity: 300, thenHoldForDuration: 0.1)
      if enabled {
        XCTAssertEqual(field.value as? String, text, "横滑抬手不能输入起始字母")
        key("Q").tap()
        let moved = try XCTUnwrap(field.value as? String)
        XCTAssertEqual(moved.replacingOccurrences(of: "q", with: ""), text)
        XCTAssertTrue(moved.contains("q"))
        XCTAssertFalse(moved.hasPrefix("q"), "光标应停在合成文本内部")
        XCTAssertFalse(moved.hasSuffix("q"), "真实文档光标必须向左移动")
      } else {
        let afterDrag = try XCTUnwrap(field.value as? String)
        XCTAssertTrue(afterDrag.hasPrefix(text), "关闭后横滑不能移动已有文本中的光标")
        key("Q").tap()
        XCTAssertEqual(field.value as? String, afterDrag + "q")
      }
      keepScreenshot(enabled ? "CursorSwipe-Enabled" : "CursorSwipe-Disabled", in: editor)
      editor.terminate()
    }
  }

  /// 宿主反复切换四种方案后，在系统键盘验证候选展开、收起和上屏。
  func testCandidatePanelAfterHostSchemeChanges() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    let editor = XCUIApplication(bundleIdentifier: "app.msime.ios.keyboard-test-host")
    editor.launchArguments = ["--system-keyboard-editor"]
    defer { editor.terminate() }

    func selectInHost(_ scheme: String) {
      if app.state == .notRunning { app.launch() } else { app.activate() }
      let settings = app.tabBars.buttons["首页"]
      XCTAssertTrue(settings.waitForExistence(timeout: 10))
      settings.tap()
      settings.tap()
      let input = app.buttons["inputSettingsLink"].firstMatch
      for _ in 0..<6 where !input.isHittable { app.swipeUp() }
      XCTAssertTrue(input.waitForExistence(timeout: 10))
      input.tap()
      let language = app.buttons["inputLanguage_mandarin"]
      XCTAssertTrue(language.waitForExistence(timeout: 10))
      language.tap()
      if scheme == "shuangpin" {
        let group = app.buttons["inputSchemeGroup_shuangpin"]
        XCTAssertTrue(group.waitForExistence(timeout: 5))
        group.tap()
      }
      let option = app.buttons["inputScheme_\(scheme)"]
      XCTAssertTrue(option.waitForExistence(timeout: 5), "宿主设置必须提供被测方案")
      option.tap()
    }

    func typeSyllable(_ scheme: String) {
      let titles = ["shuangpin": "小鹤双拼", "quanpin": "全拼 26 键",
                    "nineKey": "全拼 9 键", "fourteenKey": "全拼 14 键"]
      let loaded = XCTNSPredicateExpectation(
        predicate: NSPredicate(format: "value == %@", titles[scheme] ?? "未知方案"),
        object: editor.buttons["schemeButton"])
      XCTAssertEqual(XCTWaiter.wait(for: [loaded], timeout: 8), .completed,
                     "系统扩展必须加载宿主保存的方案，不能继续使用旧键面")
      let language = editor.buttons["bottomLanguageKey"]
      if language.label == "切换到中文输入" { language.tap() }
      switch scheme {
      case "nineKey":
        for digit in "64" { editor.buttons["nineKey\(digit)"].tap() }
      case "fourteenKey":
        for label in ["按键 B N", "按键 U I"] {
          let key = editor.buttons[label].exists ? editor.buttons[label] : editor.keys[label]
          XCTAssertTrue(key.waitForExistence(timeout: 5))
          key.tap()
        }
      default:
        for label in ["字母 N", "字母 I"] {
          let key = editor.buttons[label]
          XCTAssertTrue(key.waitForExistence(timeout: 5))
          key.tap()
        }
      }
      XCTAssertTrue(editor.buttons["candidate-1"].waitForExistence(timeout: 8), "组字必须产生候选")
    }

    selectInHost("quanpin")
    #if targetEnvironment(simulator)
    addKeyboardInSettingsIfNeeded()
    #endif
    editor.launch()
    let field = editor.textFields["systemKeyboardEditor"]
    XCTAssertTrue(field.waitForExistence(timeout: 10), "独立编辑器测试应用必须已安装")
    _ = try activeSystemKeyboard(in: editor, field: field)
    let initial = field.value as? String ?? ""
    XCTAssertTrue(initial.isEmpty || initial == field.placeholderValue, "独立编辑器必须从空文档开始")
    var committedPrefix = ""
    typeSyllable("quanpin")
    for round in 0..<3 {
      for scheme in ["shuangpin", "quanpin", "nineKey", "fourteenKey"] {
        // 保持独立编辑器进程和未完成组字，切到宿主保存方案后再切回来。
        selectInHost(scheme)
        editor.activate()
        _ = try activeSystemKeyboard(in: editor, field: field)
        // 输入框的 value 包含预编辑，正文基线只记录此前已选词提交的文字。
        let prefix = committedPrefix
        typeSyllable(scheme)
        let expand = editor.buttons["expandCandidates"]
        XCTAssertTrue(expand.waitForExistence(timeout: 5))
        expand.tap()
        let candidate = editor.buttons["panelCandidate-1"]
        XCTAssertTrue(candidate.waitForExistence(timeout: 8), "切换方案后候选必须能够展开")
        XCTAssertTrue(candidate.isHittable, "展开的候选必须能被真实触摸命中")
        XCTAssertEqual(expand.label, "收起候选")
        expand.tap()
        XCTAssertTrue(candidate.waitForNonExistence(timeout: 5), "收起后不能留下旧面板")
        expand.tap()
        XCTAssertTrue(candidate.waitForExistence(timeout: 8), "收起后必须能再次展开")
        let separator = try XCTUnwrap(candidate.label.range(of: "："))
        let selected = String(candidate.label[separator.upperBound...].prefix { $0 != "，" })
        XCTAssertNotNil(selected.range(of: "^[\\p{Han}]+$", options: .regularExpression))
        keepScreenshot("候选面板-第\(round + 1)轮-\(scheme)", in: editor)
        candidate.tap()
        let expected = prefix + selected
        let received = XCTNSPredicateExpectation(
          predicate: NSPredicate(format: "value == %@", expected), object: field)
        XCTAssertEqual(XCTWaiter.wait(for: [received], timeout: 5), .completed,
                       "选中的候选必须精确追加到已提交正文，不能保留预编辑或覆盖旧文")
        let committed = field.value as? String ?? ""
        XCTAssertTrue(committed.hasPrefix(prefix), "选词不能覆盖已有正文")
        let appended = String(committed.dropFirst(prefix.count))
        XCTAssertNotNil(appended.range(of: "^[\\p{Han}]+$", options: .regularExpression),
                        "完整候选必须跨进程提交汉字，不能残留本次原始编码")
        XCTAssertNotEqual(editor.buttons["returnKey"].label, "确认", "单音节选词后必须结束组字")
        committedPrefix = expected
        keepScreenshot("候选上屏-第\(round + 1)轮-\(scheme)", in: editor)
        typeSyllable(scheme)
      }
      print("CANDIDATE_PANEL_SCHEME_SWITCH round=\(round + 1) schemes=4 completed=1")
    }
  }

  /// 在真机系统键盘中重复组字、选词和提交，输入始终使用合成拼音。
  func testNineKeyRepeatedChineseInputOnDevice() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app)
    app.buttons["schemeButton"].tap()
    let card = app.buttons["schemeCard-nineKey"]
    XCTAssertTrue(card.waitForExistence(timeout: 5), "九键方案必须可用")
    card.tap()
    let language = app.buttons["bottomLanguageKey"]
    if language.label == "切换到中文输入" { language.tap() }
    let digitsByLetter = Dictionary(uniqueKeysWithValues:
      zip(["abc", "def", "ghi", "jkl", "mno", "pqrs", "tuv", "wxyz"], "23456789")
        .flatMap { letters, digit in letters.map { ($0, digit) } })
    for round in 0..<3 {
      for spelling in ["nihao", "zhongguo", "shurufa", "jintiantianqihenhao"] {
        let digits = spelling.compactMap { digitsByLetter[$0] }
        for digit in digits {
          let key = app.buttons["nineKey\(digit)"]
          XCTAssertTrue(key.exists, "九键按键必须存在")
          key.tap()
        }
        let candidate = app.buttons["candidate-1"]
        XCTAssertTrue(candidate.waitForExistence(timeout: 5), "组字之后必须有汉字候选")
        let before = field.value as? String ?? ""
        if round.isMultiple(of: 2) { app.buttons["spaceKey"].tap() } else { candidate.tap() }
        // 首选可能只消费前几个音节，继续提交剩余组字，避免把下一例接进上一例。
        for _ in 0..<12 {
          guard app.buttons["returnKey"].label == "确认", candidate.exists else { break }
          candidate.tap()
        }
        let after = field.value as? String ?? ""
        XCTAssertFalse(after.isEmpty, "汉字必须跨进程上屏")
        XCTAssertNotEqual(after, before, "提交必须替换预编辑")
        XCTAssertNotEqual(app.buttons["returnKey"].label, "确认", "提交后不能残留组字")
        XCTAssertNotNil(after.range(of: "^[\\p{Han}]+$", options: .regularExpression),
                        "合成拼音提交后只能留下汉字，不能残留数字或拼音")
      }
      print("NINEKEY_DEVICE round=\(round + 1) phrases=4 completed=1")
    }
  }

  /// 提交后重新组字，侧栏中复用的拼音按钮仍能被真实触摸命中。
  func testNineKeySpellingSidebarRemainsTappableAfterCommit() throws {
    let app = XCUIApplication()
    let field = try focusedTryoutKeyboard(app)
    app.buttons["schemeButton"].tap()
    let card = app.buttons["schemeCard-nineKey"]
    XCTAssertTrue(card.waitForExistence(timeout: 5))
    card.tap()
    let language = app.buttons["bottomLanguageKey"]
    if language.label == "切换到中文输入" { language.tap() }
    for _ in 0..<3 {
      for digit in "64426" { app.buttons["nineKey\(digit)"].tap() }
      let spelling = app.buttons["nineKeySpelling_ni"]
      XCTAssertTrue(spelling.waitForExistence(timeout: 5))
      XCTAssertTrue(spelling.isHittable, "重新显示的拼音按钮必须位于可点按的侧栏内")
      spelling.tap()
      let candidate = app.buttons["candidate-1"]
      XCTAssertTrue(candidate.waitForExistence(timeout: 5))
      candidate.tap()
      for _ in 0..<12 {
        guard app.buttons["returnKey"].label == "确认", candidate.exists else { break }
        candidate.tap()
      }
      XCTAssertNotEqual(app.buttons["returnKey"].label, "确认")
      XCTAssertFalse(app.scrollViews["nineKeySpellingStrip"].isHittable)
      let committed = field.value as? String ?? ""
      XCTAssertNotNil(committed.range(of: "^[\\p{Han}]+$", options: .regularExpression))
    }
    print("NINEKEY_SPELLING_DEVICE rounds=3 completed=1")
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
    // 手机 26 键的句号在逗号键长按上，独立句号仅用于其他布局。
    app.buttons["quickPunctuationKey"].press(forDuration: 0.7)
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
    let opened = app.navigationBars["输入"].waitForExistence(timeout: 8)
    if !opened { keepScreenshot("添加语言跳转失败", in: app) }
    XCTAssertTrue(opened, "添加语言应打开应用的输入设置")
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
      if !target.isHittable { keepScreenshot("数字侧栏不可点击-\(literal)", in: app) }
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
      // 三倍屏的像素取整允许相邻等宽键相差三分之一点，与布局单测容差一致。
      XCTAssertGreaterThanOrEqual(zero.width, key("nineKey8").frame.width - 0.5)
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
    // 侧栏只显示两项，高速 swipe 会直接甩到底并越过乘号。按目标位置短拖动，停稳后重新判断。
    for _ in 0..<12 where !key.isHittable {
      let upward = key.frame.midY > scroll.frame.midY
      let start = scroll.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: upward ? 0.8 : 0.2))
      let end = scroll.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: upward ? 0.3 : 0.7))
      start.press(forDuration: 0.05, thenDragTo: end, withVelocity: 100, thenHoldForDuration: 0.2)
    }
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

    // 前一项可能留下九键或英文模式，本项显式选择全拼中文。
    app.buttons["schemeButton"].tap()
    app.buttons["schemeCard-quanpin"].tap()
    let language = app.buttons["bottomLanguageKey"]
    if language.label == "切换到中文输入" { language.tap() }

    for letter in ["N", "I", "H", "A", "O"] {
      let key = app.buttons["字母 \(letter)"]
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
    app.tabBars.buttons["首页"].tap()
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

  /// 先通过原生设置启用全拼 14 键，再按 BN UI GH AS OP，在真实编辑器中选择「你好」。bu 和 ni 同组码，只要求「你好」在前几个候选里。
  func testFourteenKeysTypeNihaoIntoTheHostEditor() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    let input = app.buttons["inputSettingsLink"]
    for _ in 0..<6 where !input.isHittable { app.swipeUp() }
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    input.tap()
    app.buttons["inputLanguage_mandarin"].tap()
    let option = app.buttons["inputScheme_fourteenKey"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    option.tap()
    // 选项在退场动画完成后才保存，不能在回调之前终止宿主。
    let dismissed = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "exists == false"), object: app.buttons["designOptionSheetCancel"])
    XCTAssertEqual(XCTWaiter.wait(for: [dismissed], timeout: 5), .completed)
    app.buttons["inputLanguage_mandarin"].tap()
    XCTAssertTrue(option.isSelected, "十四键选择必须实际保存后再启动扩展")
    app.buttons["designOptionSheetCancel"].tap()
    let closed = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "exists == false"), object: app.buttons["designOptionSheetCancel"])
    XCTAssertEqual(XCTWaiter.wait(for: [closed], timeout: 5), .completed)
    app.terminate()
    let field = try focusedTryoutKeyboard(app)

    let scheme = app.buttons["schemeButton"]
    XCTAssertTrue(scheme.waitForExistence(timeout: 5), "the scheme button is missing")
    scheme.tap()
    let card = app.buttons["schemeCard-fourteenKey"]
    let grid = app.descendants(matching: .any)["schemePickerScroll"].firstMatch
    for _ in 0..<5 where !card.isHittable { grid.swipeLeft() }
    if !card.isHittable { keepScreenshot("十四键方案不可点击", in: app) }
    XCTAssertTrue(card.isHittable, "已在原生设置启用的十四键必须能在方案分页中找到")
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

  /// 原生设置保存双拼 14 键后，系统加载的扩展在独立编辑器中完成四种方案的选词上屏。
  func testFourteenKeyShuangpinProfilesCommitIntoTheSystemEditor() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    let editor = XCUIApplication(bundleIdentifier: "app.msime.ios.keyboard-test-host")
    editor.launchArguments = ["--system-keyboard-editor"]
    let profiles = ["shuangpin", "ziranma", "microsoft", "shoudao"]

    func openSettings(_ identifier: String) {
      app.terminate()
      app.launch()
      let settings = app.tabBars.buttons["首页"]
      XCTAssertTrue(settings.waitForExistence(timeout: 10))
      settings.tap()
      settings.tap()
      let link = app.buttons[identifier].firstMatch
      for _ in 0..<6 where !link.isHittable { app.swipeUp() }
      XCTAssertTrue(link.waitForExistence(timeout: 10))
      link.tap()
    }
    func openMandarin() {
      openSettings("inputSettingsLink")
      let row = app.buttons["inputLanguage_mandarin"]
      XCTAssertTrue(row.waitForExistence(timeout: 5))
      row.tap()
      XCTAssertTrue(app.buttons["designOptionSheetCancel"].waitForExistence(timeout: 5))
    }
    func waitForSelectionToSave() {
      XCTAssertTrue(app.buttons["designOptionSheetCancel"].waitForNonExistence(timeout: 5),
                    "选项退场后才会保存，不能提前终止宿主")
    }
    func selectScheme(_ identifier: String) {
      openMandarin()
      let raw = String(identifier.dropFirst("inputScheme_".count))
      let group = profiles.contains(raw) ? "shuangpin" : raw.hasPrefix("wubi_") ? "wubi" : nil
      if let group { app.buttons["inputSchemeGroup_\(group)"].tap() }
      let option = app.buttons[identifier]
      XCTAssertTrue(option.waitForExistence(timeout: 5))
      option.tap()
      waitForSelectionToSave()
    }
    func openLayout() {
      openSettings("keyboardLayoutLink")
      let picker = app.buttons["shuangpinKeyboardPicker"]
      XCTAssertTrue(picker.waitForExistence(timeout: 5))
      picker.tap()
    }

    openMandarin()
    if let group = ["shuangpin", "wubi"].map({ app.buttons["inputSchemeGroup_\($0)"] })
      .first(where: { $0.exists && $0.isSelected }) {
      group.tap()
    }
    let previousScheme = try XCTUnwrap(app.buttons.allElementsBoundByIndex.first {
      $0.identifier.hasPrefix("inputScheme_") && $0.isSelected
    }?.identifier, "此专项需要已启用的普通话方案")
    addTeardownBlock { @MainActor in
      editor.terminate()
      selectScheme(previousScheme)
      app.terminate()
    }
    app.buttons["designOptionSheetCancel"].tap()
    selectScheme("inputScheme_shuangpin")
    openLayout()
    let previousLayout = try XCTUnwrap(app.buttons.allElementsBoundByIndex.first {
      $0.identifier.hasPrefix("shuangpinKeyboard") && $0.isSelected
    }?.identifier)
    addTeardownBlock { @MainActor in
      editor.terminate()
      openLayout()
      app.buttons[previousLayout].tap()
      waitForSelectionToSave()
    }
    app.buttons["shuangpinKeyboardfourteen_key"].tap()
    waitForSelectionToSave()
    app.terminate()
    #if targetEnvironment(simulator)
    addKeyboardInSettingsIfNeeded()
    #endif
    editor.launch()
    let field = editor.textFields["systemKeyboardEditor"]
    XCTAssertTrue(field.waitForExistence(timeout: 10))
    var expected = ""
    for (profile, title, lastKey) in [("shuangpin", "小鹤双拼", "CV"),
                                      ("ziranma", "自然码双拼", "JK"),
                                      ("microsoft", "微软双拼", "JK"),
                                      ("shoudao", "首道双拼", "DF")] {
      selectScheme("inputScheme_\(profile)")
      editor.activate()
      _ = try activeSystemKeyboard(in: editor, field: field)
      let loaded = XCTNSPredicateExpectation(
        predicate: NSPredicate(format: "value == %@", title), object: editor.buttons["schemeButton"])
      XCTAssertEqual(XCTWaiter.wait(for: [loaded], timeout: 8), .completed)
      let language = editor.buttons["bottomLanguageKey"]
      if language.label == "切换到中文输入" { language.tap() }
      func key(_ face: String) -> XCUIElement {
        editor.descendants(matching: .any)["fourteenKey\(face)"].firstMatch
      }
      XCTAssertTrue(key("QW").waitForExistence(timeout: 5), "换双拼方案必须保留 14 键布局")
      XCTAssertEqual(key("L").label, profile == "microsoft" ? "字母 L 和分号" : "字母 L")
      for face in ["BN", "UI", "GH", lastKey] {
        XCTAssertTrue(key(face).isHittable)
        key(face).tap()
      }
      let ni = editor.buttons["readingSpelling_ni"]
      XCTAssertTrue(ni.waitForExistence(timeout: 5))
      ni.tap()
      XCTAssertTrue(editor.buttons["candidate-1"].waitForExistence(timeout: 8))
      let candidates = (1...5).map { editor.buttons["candidate-\($0)"] }.filter(\.exists)
      let nihao = try XCTUnwrap(candidates.first { $0.label.contains("你好") }, profile)
      nihao.tap()
      expected += "你好"
      let committed = XCTNSPredicateExpectation(
        predicate: NSPredicate(format: "value == %@", expected), object: field)
      XCTAssertEqual(XCTWaiter.wait(for: [committed], timeout: 5), .completed,
                     "系统文档必须整段收到汉字，不能覆盖既有文本或残留组码")
      XCTAssertNotEqual(editor.buttons["returnKey"].label, "确认")
      editor.buttons["layoutToggleButton"].tap()
      editor.buttons["layoutToggleButton"].tap()
      XCTAssertTrue(key("QW").isHittable, "数字页往返必须保留双拼 14 键")
      keepScreenshot("系统双拼十四键-\(profile)", in: editor)
    }
  }

  /// The double-pinyin face labels its keys with the units the running profile puts on them.
  ///
  /// These hints are read out of the Engine through the shared ABI rather than from a table kept
  /// in the host, which is only true at run time: the strings do not exist in the binary, so no
  /// static check can see this. Xiaohe's K carries both `ing` and `uai`, and a host-side copy had
  /// lost the second one.
  func testDoublePinyinKeyFaceCarriesTheEngineHints() throws {
    let app = XCUIApplication()
    // 提示缺省关闭，先通过原生设置开启，结束后恢复原值。
    func openHints() -> XCUIElement {
      app.launchArguments = ["-hasCompletedOnboarding", "YES"]
      app.launch()
      app.buttons["keyboardLayoutLink"].tap()
      let toggle = app.switches["appShuangpinKeyHintsToggle"]
      for _ in 0..<4 where !toggle.isHittable {
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.03, dy: 0.84))
        let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.03, dy: 0.55))
        start.press(forDuration: 0.05, thenDragTo: end)
      }
      XCTAssertTrue(toggle.waitForExistence(timeout: 5))
      return toggle
    }
    let hints = openHints()
    let previousHints = try XCTUnwrap(hints.value as? String)
    if previousHints == "0" {
      hints.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    }
    let enabled = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == '1'"), object: hints)
    XCTAssertEqual(XCTWaiter.wait(for: [enabled], timeout: 5), .completed)
    addTeardownBlock { @MainActor in
      app.terminate()
      let restored = openHints()
      if restored.value as? String != previousHints {
        restored.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
      }
      app.terminate()
    }
    app.terminate()
    _ = try focusedTryoutKeyboard(app)

    let scheme = app.buttons["schemeButton"]
    XCTAssertTrue(scheme.waitForExistence(timeout: 5), "the scheme button is missing")
    scheme.tap()

    let card = app.buttons["schemeCard-shuangpin"]
    guard card.waitForExistence(timeout: 5) else {
      throw XCTSkip("Double pinyin is not among the enabled schemes on this device.")
    }
    card.tap()

    let language = app.buttons["bottomLanguageKey"]
    if language.label == "切换到中文输入" { language.tap() }
    // 键位提示位于按键的无障碍值，不依赖截图中的字形识别。
    let k = app.buttons["字母 K"]
    XCTAssertTrue(k.waitForExistence(timeout: 5), "字母 K is missing from the double-pinyin face")
    let hint = (k.value as? String) ?? ""
    XCTAssertTrue(hint.contains("uai"),
                  "K reads \(hint.isEmpty ? "nothing" : hint) but Xiaohe types uai on it")
    XCTAssertTrue(hint.contains("ing"), "K reads \(hint) but Xiaohe types ing on it too")
  }
}

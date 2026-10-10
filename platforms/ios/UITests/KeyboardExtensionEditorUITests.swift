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
      .matching(NSPredicate(format: "label == %@", "水杉输入法")).firstMatch
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

  /// Bring the editor up and switch to our keyboard, or skip if it is not installed as one.
  ///
  /// The globe cycles through the enabled keyboards; which one a fresh editor opens on is a
  /// system decision, so this taps around the ring instead of assuming. The ring is finite, and
  /// one lap is enough to prove ours is not in it.
  private func focusedTryoutKeyboard(_ app: XCUIApplication) throws -> XCUIElement {
    addKeyboardInSettingsIfNeeded()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()

    let tryout = app.buttons["keyboardTryoutLink"]
    XCTAssertTrue(tryout.waitForExistence(timeout: 20), "the tryout entry never appeared")
    tryout.tap()

    let field = app.textFields["keyboardTryoutField"]
    XCTAssertTrue(field.waitForExistence(timeout: 15), "the tryout editor never appeared")
    // This screen focuses the field on appearance; tapping an already focused field while the
    // keyboard animates can land on a key instead.
    let focused = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "hasKeyboardFocus == true"), object: field)
    if XCTWaiter.wait(for: [focused], timeout: 10) != .completed { field.tap() }

    let ours = app.keys["字母 N"]
    if ours.waitForExistence(timeout: 6) { return field }

    // 沿键盘列表轮转一圈。地球键没有标识符，只有随系统语言变化的标签：英文是 `Next keyboard`，中文是「下一个键盘」。
    let globe = app.buttons.matching(NSPredicate(format: "label IN %@", ["Next keyboard", "下一个键盘"])).firstMatch
    if globe.waitForExistence(timeout: 6) {
      // 全新的 iOS 27 模拟器默认启用十四个键盘，其中几个显示名相同，所以轮转次数要覆盖整圈，而不是只点寥寥几次。
      for _ in 0..<20 {
        globe.tap()
        if ours.waitForExistence(timeout: 3) { return field }
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

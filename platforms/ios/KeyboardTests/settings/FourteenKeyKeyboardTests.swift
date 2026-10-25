import XCTest
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// 全拼和双拼 14 键：桥接层、键面、读屏、读音选择、布局热加载与 iPad 排布保持一致。
@MainActor
final class FourteenKeyKeyboardTests: XCTestCase {
  private var previousScheme = InputSchemePreference.scheme

  // 认领全部方案，免得对 `InputSchemePreference.scheme` 的赋值落到模拟器上残留的别的方案。14 键要用户自己打开，不先启用就选不上。见 InputSchemeTestSupport。
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    previousScheme = InputSchemePreference.scheme
    InputSchemePreference.scheme = .fourteenKey
  }

  override func tearDown() {
    InputSchemePreference.scheme = previousScheme
    super.tearDown()
  }

  // MARK: - 键表

  /// 三端同一张键表：14 个键，组码是每组的首字母，26 个字母依次落在引擎的编码表 `abcdedggujjlmbooqeatucqztz` 上。
  func testTheKeyTableIsTheEngineGrid() {
    XCTAssertEqual(FourteenKeyLayout.rows.map { $0.map(\.face) },
                   [["QW", "ER", "TY", "UI", "OP"], ["AS", "DF", "GH", "JK", "L"], ["ZX", "CV", "BN", "M"]])
    XCTAssertEqual(FourteenKeyLayout.rows.joined().map(\.code).joined(), "qetuoadgjlzcbm")
    XCTAssertEqual(FourteenKeyLayout.codeTable, "abcdedggujjlmbooqeatucqztz")
    let qw = FourteenKeyLayout.rows[0][0], l = FourteenKeyLayout.rows[1][4]
    XCTAssertEqual(qw.accessibilityLabel, "按键 Q W")
    XCTAssertEqual(l.accessibilityLabel, "字母 L")
    XCTAssertEqual(qw.holdLetters, ["q", "w"])
    XCTAssertTrue(l.holdLetters.isEmpty)
    XCTAssertTrue(FourteenKeyLayout.rows[2][3].holdLetters.isEmpty, "M has no hold menu")
    XCTAssertEqual(qw.keyID, "FourteenQW")
    // 每个键都记到 client-services 认得的 id 上，否则整批按键统计会被拒收。
    for key in FourteenKeyLayout.rows.joined() {
      XCTAssertNotNil(key.keyID, key.face)
    }
  }

  // MARK: - 桥接

  /// 桥接层切到 14 键后，按组码打出「你好」；14 键是会话状态，会话重建后仍在。
  func testTheBridgeTypesNihaoOnTheGridAndKeepsItAcrossARebuild() throws {
    let bridge = MetasequoiaInputSessionBridge()
    XCTAssertNil(bridge.switchToFourteenKey().diagnosticText)
    func type(_ codes: String) -> MetasequoiaInputSnapshot {
      var snapshot = bridge.cancel()
      for code in codes { snapshot = bridge.gridKey(String(code)) }
      return snapshot
    }
    var snapshot = type("bugao")
    XCTAssertTrue(snapshot.isHandled)
    XCTAssertTrue(snapshot.candidates.contains("你好"), "\(snapshot.candidates)")
    // bu 和 ni 是同一组码，读音行跟着首选走；首选是哪一个由引擎按词频定。
    XCTAssertTrue(["ni'hao", "bu'hao"].contains(snapshot.nineKeyReading), snapshot.nineKeyReading)
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    snapshot = type("bugao")
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session forgot the 14-key grid")
    _ = bridge.cancel()
    // 换回 26 键后网格关掉，组码不再被收下。
    _ = bridge.switch(toShuangpin: false)
    XCTAssertFalse(bridge.gridKey("b").isHandled)
  }

  /// 双拼 14 键沿用当前双拼 profile；同一张键面交给双拼网格后，组码仍按双拼规则解码。
  func testTheBridgeTypesShuangpinFourteenKeyWithTheSelectedProfile() throws {
    let bridge = MetasequoiaInputSessionBridge()
    for (profile, codes) in [("xiaohe", "bugc"), ("ziranma", "bugj"),
                              ("microsoft", "bugj"), ("shoudao", "bugd")] {
      XCTAssertNil(bridge.switchToFourteenKeyShuangpin(profile: profile).diagnosticText)
      for rebuilt in [false, true] {
        _ = bridge.cancel()
        if rebuilt {
          XCTAssertTrue(bridge.suspendDictionarySession())
          try bridge.resumeDictionarySession()
        }
        XCTAssertEqual(bridge.sharedPreferences?["shuangpin_profile"] as? String, profile)
        var snapshot = bridge.cancel()
        for code in codes { snapshot = bridge.gridKey(String(code)) }
        XCTAssertTrue(snapshot.isHandled, profile)
        XCTAssertTrue(snapshot.candidates.contains("你好"), "\(profile): \(snapshot.candidates)")
        // 多义组码的首选读音受词频影响；明确选 ni 后核对读音与候选，不绑定未消歧的首选。
        let ni = try XCTUnwrap(snapshot.nineKeySpellings.firstIndex(of: "ni"), profile)
        snapshot = bridge.chooseNineKeySpelling(at: UInt(ni))
        XCTAssertTrue(snapshot.nineKeyReading.hasPrefix("ni"), snapshot.nineKeyReading)
        XCTAssertTrue(snapshot.candidates.contains("你好"), "\(profile): \(snapshot.candidates)")
      }
      _ = bridge.cancel()
    }
    XCTAssertNil(bridge.switchToFourteenKeyShuangpin(profile: "microsoft").diagnosticText)
    _ = bridge.gridKey("q")
    XCTAssertTrue(bridge.gridKey("l").nineKeySpellings.contains("qing"), "微软 q; 的分号由 L 键承载")
  }

  /// 设置页会立即更新 App Group；键盘必须对照自身已应用布局，才能在方案不变时重画键面和读音行。
  func testShuangpinLayoutReloadChangesKeysAndReadingWithoutChangingProfile() throws {
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    let previousLayout = TouchShuangpinLayoutPreference.layout
    let previousHints = KeyboardLayoutPreference.shuangpinKeyHints
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = document }
      TouchShuangpinLayoutPreference.layout = previousLayout
      KeyboardLayoutPreference.shuangpinKeyHints = previousHints
    }
    XCTAssertTrue(InputSchemePreference.select(.shuangpin))
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.twentySixKey))
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    let qw = try button("fourteenKeyQW", in: controller)
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let initialHeight = height.constant
    XCTAssertFalse(shown(qw))

    func reload(until done: () -> Bool) {
      controller.viewWillAppear(false)
      let deadline = Date().addingTimeInterval(15)
      while !done() && Date() < deadline {
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
      }
      controller.view.layoutIfNeeded()
    }
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.fourteenKey))
    reload { shown(qw) }
    XCTAssertTrue(shown(qw))
    XCTAssertEqual(height.constant - initialHeight, readingRowCost)
    XCTAssertEqual(InputSchemePreference.scheme, .shuangpin)
    try tap(["BN", "UI", "GH", "CV"], in: controller)
    XCTAssertTrue(candidates(in: controller).contains { $0.contains("你好") })
    XCTAssertEqual(controller.inlineMarkedText, "")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "14键")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)

    // 组字期间读入 26 键设置，当前键面保持 14 键；提交后同一控制器立即换成 26 键。
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.twentySixKey))
    KeyboardLayoutPreference.shuangpinKeyHints = false
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences {
      $0["touch_shuangpin_key_hints"] = true
    })
    // 键位提示只由重载回调镜像，不能用已被 save 同步写入的布局镜像当作重载完成信号。
    reload { KeyboardLayoutPreference.shuangpinKeyHints }
    XCTAssertTrue(KeyboardLayoutPreference.shuangpinKeyHints)
    XCTAssertTrue(shown(qw))
    try button("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(qw))
    XCTAssertEqual(height.constant, initialHeight)
    XCTAssertEqual(InputSchemePreference.scheme, .shuangpin)
  }

  /// 微软双拼 14 键显示分号入口；平板横屏开启分离时仍画完整网格，不露出 26 键分号。
  func testMicrosoftShuangpinFourteenKeysStayWholeOnTablet() throws {
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    let previousLayout = TouchShuangpinLayoutPreference.layout
    let previousSplit = KeyboardLayoutPreference.tabletSplit
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = document }
      TouchShuangpinLayoutPreference.layout = previousLayout
      KeyboardLayoutPreference.tabletSplit = previousSplit
    }
    XCTAssertTrue(InputSchemePreference.select(.microsoft))
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.fourteenKey))
    KeyboardLayoutPreference.tabletSplit = true
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = .pad
    controller.traitOverrides.horizontalSizeClass = .regular
    controller.traitOverrides.verticalSizeClass = .compact
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 1032, height: 400)
    controller.view.layoutIfNeeded()
    defer { controller.viewDidDisappear(false) }
    let l = try button("fourteenKeyL", in: controller)
    XCTAssertTrue(shown(l))
    XCTAssertEqual(l.configuration?.title, "L;")
    XCTAssertEqual(l.accessibilityLabel, "字母 L 和分号")
    XCTAssertFalse(shown(try button("microsoftFinalKey", in: controller)))
    XCTAssertFalse(shown(try view("numberRow", in: controller)))
    XCTAssertFalse(shown(try button("tabletLayerKey", in: controller)))
    try tap(["QW", "L"], in: controller)
    XCTAssertTrue(shown(try button("readingSpelling_qing", in: controller)))
    attachScreenshot(of: controller, named: "微软双拼十四键平板横屏")
  }

  // MARK: - 键面

  /// 14 键在手机窄屏和宽屏上都不越界，四排等高，按键与 26 键一样高；键盘总高度比 26 键多出单独一行的读音（26 键的读音并入候选行，14 键的拼音选择条要排在读音右边）。字母行、数字行和 Shift 都不显示，底行是手机 26 键底行。
  func testTheKeysStayInBoundsAtTheHeightOfTheTwentySixKeys() throws {
    for width in [320.0, 414.0] {
      let controller = makeController(width: width)
      let reference = try button("returnKey", in: controller).bounds.height
      XCTAssertGreaterThanOrEqual(reference, KeyboardHeightPercent.portraitKeyHeight(tablet: false) - 0.5)
      XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant,
                     KeyboardViewController.keyboardHeight(separateReadingRow: true),
                     "the 26-key keyboard plus the reading line, with keys as tall as the 26 keys")
      XCTAssertEqual(KeyboardViewController.keyboardHeight(separateReadingRow: true) - KeyboardViewController.defaultKeyboardHeight,
                     readingRowCost)
      for key in FourteenKeyLayout.rows.joined() {
        let button = try button("fourteenKey\(key.face)", in: controller)
        XCTAssertTrue(shown(button), key.face)
        XCTAssertEqual(button.configuration?.title, key.face)
        XCTAssertEqual(button.accessibilityLabel, key.accessibilityLabel)
        XCTAssertEqual(button.bounds.height, reference, accuracy: 0.5, key.face)
        let frame = button.convert(button.bounds, to: controller.view)
        XCTAssertGreaterThanOrEqual(frame.minX, KeyboardFormFactor.phone.padding.leading - 0.5, "\(key.face) at \(width)")
        XCTAssertLessThanOrEqual(frame.maxX, width - KeyboardFormFactor.phone.padding.trailing + 0.5, "\(key.face) at \(width)")
      }
      // 第二排不缩进：AS 和 QW 左边对齐，L 与 OP 右边对齐。
      let qw = try frame("fourteenKeyQW", in: controller), asKey = try frame("fourteenKeyAS", in: controller)
      XCTAssertEqual(asKey.minX, qw.minX, accuracy: 0.5)
      XCTAssertEqual(try frame("fourteenKeyL", in: controller).maxX, try frame("fourteenKeyOP", in: controller).maxX, accuracy: 0.5)
      // 第三排两端是分词键和 ⌫，宽度与 26 键的 ⇧、⌫ 相同。
      let separator = try button("fourteenKeySeparator", in: controller)
      let delete = try button("fourteenKeyDelete", in: controller)
      XCTAssertEqual(separator.bounds.width, 44, accuracy: 0.5)
      XCTAssertEqual(delete.bounds.width, 44, accuracy: 0.5)
      XCTAssertLessThan(try frame("fourteenKeySeparator", in: controller).maxX, try frame("fourteenKeyZX", in: controller).minX)
      XCTAssertGreaterThan(try frame("fourteenKeyDelete", in: controller).minX, try frame("fourteenKeyM", in: controller).maxX)
      XCTAssertEqual(separator.configuration?.title, "符")
      XCTAssertEqual(separator.accessibilityLabel, "符号")
      // 中文 14 键没有 Shift，也不画 26 键的字母行和 iPad 的数字行。
      XCTAssertFalse(shown(try button("shiftButton", in: controller)))
      XCTAssertNil(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" && shown($0) })
      XCTAssertFalse(shown(try view("numberRow", in: controller)))
      // 底行沿用手机 26 键底行：123 | ， | 空格 | 。 | 中 | 回车。
      for id in ["layoutToggleButton", "quickPunctuationKey", "spaceKey", "bottomPeriodKey", "bottomLanguageKey", "returnKey"] {
        XCTAssertTrue(shown(try button(id, in: controller)), id)
      }
      XCTAssertFalse(shown(try button("nineKeyZero", in: controller)))
      // 第三排已有 符，底行不再重复 26 键的 符。
      XCTAssertFalse(shown(try button("symbolPanelKey", in: controller)))
      XCTAssertEqual(try button("quickPunctuationKey", in: controller).configuration?.title, "，")
      XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 14 键")
      attachScreenshot(of: controller, named: "Fourteen-key keyboard at \(Int(width))pt")
    }
  }

  /// 两个字母的键各有一个长按手势，L 和 M 没有；长按 QW 弹出 q、w，选 w 先结束组字再上屏 w，与九键的长按相同。
  func testHoldingAKeyOffersItsLettersAndFinishesTheComposition() throws {
    let controller = makeController(width: 390)
    for key in FourteenKeyLayout.rows.joined() {
      let holds = (try button("fourteenKey\(key.face)", in: controller).gestureRecognizers ?? [])
        .compactMap { $0 as? UILongPressGestureRecognizer }
      XCTAssertEqual(holds.count, key.holdLetters.isEmpty ? 0 : 1, key.face)
      XCTAssertEqual(holds.first?.minimumPressDuration ?? KeyboardViewController.nineKeyHoldDuration,
                     KeyboardViewController.nineKeyHoldDuration)
    }
    try tap(["BN"], in: controller)
    XCTAssertFalse(preedit(in: controller).isEmpty, "BN started a composition")
    let qw = try button("fourteenKeyQW", in: controller)
    let hold = try XCTUnwrap(qw.gestureRecognizers?.compactMap { $0 as? UILongPressGestureRecognizer }.first)
    hold.state = .began
    controller.perform(NSSelectorFromString("handleFourteenKeyHold:"), with: hold)
    XCTAssertTrue(shown(try button("nineKeyHoldOption-q", in: controller)))
    let w = try button("nineKeyHoldOption-w", in: controller)
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "nineKeyHoldOption-1" },
                 "a 14-key hold offers the letters only")
    w.sendActions(for: .primaryActionTriggered)
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "nineKeyHoldOptions" })
    XCTAssertFalse(composing(controller), "choosing a letter finished the composition first")
  }

  // MARK: - 输入

  /// 「你好」是 BN UI GH AS OP：读音行显示引擎给的拼音而不是组码，候选条上有你好。bu 和 ni 同一组码，你好和不好谁在前由引擎按词频定。
  func testTypingNihaoOnTheKeys() throws {
    let controller = makeController(width: 390)
    try tap(["BN", "UI", "GH", "AS", "OP"], in: controller)
    XCTAssertTrue(["ni'hao", "bu'hao"].contains(preedit(in: controller)), "the reading row shows \(preedit(in: controller))")
    XCTAssertTrue(candidates(in: controller).prefix(3).contains { $0.contains("你好") }, "\(candidates(in: controller))")
    // 组字中左端键是分词。
    let separator = try button("fourteenKeySeparator", in: controller)
    XCTAssertEqual(separator.configuration?.title, "分词")
    XCTAssertEqual(separator.accessibilityLabel, "拼音分词")
    attachScreenshot(of: controller, named: "Fourteen-key nihao")
  }

  /// 分词键在组码之间定一个音节分界：ZX UI 分词 AS BN 得到「西安」。
  func testTheSeparatorKeySplitsXian() throws {
    let controller = makeController(width: 390)
    try tap(["ZX", "UI"], in: controller)
    try button("fourteenKeySeparator", in: controller).sendActions(for: .primaryActionTriggered)
    try tap(["AS", "BN"], in: controller)
    XCTAssertTrue(preedit(in: controller).contains("'"), preedit(in: controller))
    XCTAssertTrue(candidates(in: controller).contains { $0.contains("西安") }, "\(candidates(in: controller))")
  }

  /// AS GH UI 时拼音选择条在读音行右侧，有 shi 和 shu，不盖住候选行；点 shu 锁定这个音节，退格先撤销锁定。
  func testTheSpellingStripSitsBesideTheReading() throws {
    let controller = makeController(width: 390)
    try tap(["AS", "GH", "UI"], in: controller)
    controller.view.layoutIfNeeded()
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "readingSpellingStrip" } as? UIScrollView)
    XCTAssertTrue(shown(strip))
    let shi = try button("readingSpelling_shi", in: controller)
    XCTAssertTrue(shown(try button("readingSpelling_shu", in: controller)))
    let reading = try frame("compositionRow", in: controller)
    let stripFrame = strip.convert(strip.bounds, to: controller.view)
    let preeditFrame = try frame("preeditButton", in: controller)
    let candidate = try frame("candidate-1", in: controller)
    XCTAssertGreaterThanOrEqual(stripFrame.minY, reading.minY - 0.5)
    XCTAssertLessThanOrEqual(stripFrame.maxY, reading.maxY + 0.5)
    XCTAssertLessThanOrEqual(stripFrame.maxY, candidate.minY + 0.5, "the strip does not cover the candidates")
    XCTAssertGreaterThanOrEqual(stripFrame.minX, preeditFrame.maxX - 0.5, "the strip sits to the right of the reading")
    // 九键侧栏里的拼音条没有内容：14 键的拼音条只在读音行。
    XCTAssertNil(descendants(controller.view).first { ($0.accessibilityIdentifier ?? "").hasPrefix("nineKeySpelling_") && shown($0) })
    XCTAssertEqual(shi.accessibilityLabel, KeyboardNineKeyPanelColumns.spellingAccessibilityLabel("shi"))
    attachScreenshot(of: controller, named: "Fourteen-key spelling strip")

    let unlocked = preedit(in: controller)
    try button("readingSpelling_shu", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(preedit(in: controller), "shu", "choosing shu locks the syllable")
    try button("fourteenKeyDelete", in: controller).sendActions(for: .touchDown)
    try button("fourteenKeyDelete", in: controller).sendActions(for: .touchUpInside)
    XCTAssertTrue(composing(controller), "backspace undid the lock rather than a key")
    XCTAssertEqual(preedit(in: controller), unlocked, "backspace undid the lock first")
    XCTAssertTrue(shown(try button("readingSpelling_shu", in: controller)))
  }

  /// 键盘还在时从 26 键换成 14 键：读音从候选行最左边移回候选行上面单独一行，键盘长高这一行；换回 26 键时又收回去。组字时两者的位置由上面的拼音选择条测试和 NineKeyKeyboardTests 覆盖。
  func testSwitchingSchemesMovesTheReadingAndResizesTheKeyboard() throws {
    InputSchemePreference.scheme = .quanpin
    let controller = makeController(width: 390)
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let twentySix = height.constant
    XCTAssertEqual(twentySix, KeyboardViewController.defaultKeyboardHeight)

    InputSchemePreference.scheme = .fourteenKey
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(height.constant - twentySix, readingRowCost)
    let reading = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "compositionRow" })
    XCTAssertEqual(reading.bounds.height, KeyboardViewController.readingRowHeight, accuracy: 0.5)

    InputSchemePreference.scheme = .quanpin
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(height.constant, twentySix)
  }

  /// 英文模式画 26 键，切回中文仍是 14 键；123 打开数字九宫格，返回键印「14键」，返回后仍是 14 键。
  func testEnglishAndTheSymbolLayerReturnToTheFourteenKeys() throws {
    let controller = makeController(width: 390)
    let qw = try button("fourteenKeyQW", in: controller)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(qw))
    XCTAssertNotNil(descendants(controller.view).first { ($0.accessibilityLabel ?? "").hasSuffix(" Q") && shown($0) })
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(qw))
    XCTAssertEqual(InputSchemePreference.scheme, .fourteenKey, "English does not rewrite the layout")

    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(qw))
    XCTAssertTrue(shown(try button("nineKey1", in: controller)))
    XCTAssertEqual(try button("nineKey1", in: controller).configuration?.title, "1")
    // 与 Android 一样，借用的数字页上返回键印 14 键自己的短名。
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "14键")
    XCTAssertEqual(try button("layoutToggleButton", in: controller).accessibilityLabel, "切换到字母键盘")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(qw))
  }

  /// 14 键的组码不写进输入框；本地输入模式画 26 键，组字照常标记，与全拼 26 键和 Android 相同。
  func testInlinePreeditSkipsTheCodesButMarksALocalMode() throws {
    let controller = makeController(width: 390)
    try tap(["BN", "UI"], in: controller)
    XCTAssertFalse(preedit(in: controller).isEmpty)
    XCTAssertEqual(controller.inlineMarkedText, "", "the 14-key codes are not written into the editor")
    try button("fourteenKeyDelete", in: controller).sendActions(for: .touchDown)
    try button("fourteenKeyDelete", in: controller).sendActions(for: .touchUpInside)
    try button("fourteenKeyDelete", in: controller).sendActions(for: .touchDown)
    try button("fourteenKeyDelete", in: controller).sendActions(for: .touchUpInside)
    XCTAssertFalse(composing(controller))

    controller.openLocalInputMode("U")
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(try button("fourteenKeyQW", in: controller)), "a local mode draws the 26 letters")
    let a = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 A" && shown($0) } as? UIButton)
    a.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(controller.inlineMarkedText.isEmpty, "the local mode's composition is marked")
  }

  /// 26 键之前按下、14 键之后才到的字母点按（设置在键盘开着时改了布局）不开始 26 键组字。
  func testAStaleTwentySixKeyTapIsIgnored() throws {
    let controller = makeController(width: 390)
    InputSchemePreference.scheme = .quanpin
    controller.viewWillAppear(false)
    let n = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    InputSchemePreference.scheme = .fourteenKey
    n.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(composing(controller), "a stale 26-key letter started a composition")
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 14 键")
  }

  // MARK: - iPad

  /// iPad 与九键一样提供 14 键：不分体，不画数字行，底行是手机 26 键底行，两端键按 26 键 ⇧ 的比例放宽。
  func testTheTabletDrawsTheFourteenKeysWhole() throws {
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = .pad
    controller.traitOverrides.horizontalSizeClass = .regular
    controller.loadViewIfNeeded()
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    controller.view.frame = CGRect(x: 0, y: 0, width: 1032, height: height)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(InputSchemePreference.offeredSchemes.contains(.fourteenKey))
    let qw = try button("fourteenKeyQW", in: controller)
    XCTAssertTrue(shown(qw))
    XCTAssertEqual(qw.bounds.height, KeyboardHeightPercent.portraitKeyHeight(tablet: true), accuracy: 0.5)
    XCTAssertFalse(shown(try view("numberRow", in: controller)))
    XCTAssertFalse(shown(try button("tabletLayerKey", in: controller)), "the 14 keys use the phone bottom row")
    XCTAssertTrue(shown(try button("returnKey", in: controller)))
    XCTAssertTrue(shown(try button("bottomPeriodKey", in: controller)))
    let row = try XCTUnwrap(try button("fourteenKeyDelete", in: controller).superview)
    XCTAssertEqual(try button("fourteenKeyDelete", in: controller).bounds.width,
                   row.bounds.width * KeyboardViewController.tabletFourteenKeyEdgeRatio, accuracy: 0.5)
    XCTAssertFalse(KeyboardSplitLayout.splitsLayout(scheme: .fourteenKey, chinese: true, localMode: false))
    attachScreenshot(of: controller, named: "Fourteen-key iPad keyboard")
  }

  // MARK: - 辅助

  /// 读音单独一行让顶栏多出的高度：开着释义时是整行 14pt；不开时 26 键的 38pt 候选行被 42pt 的最低高度垫高了 4pt，只多 10pt。
  private var readingRowCost: CGFloat {
    let lines = KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: false)
    return KeyboardViewController.topRowHeight(glossLines: lines, separateReadingRow: true)
      - KeyboardViewController.topRowHeight(glossLines: lines)
  }

  private func makeController(width: CGFloat) -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: width, height: KeyboardViewController.keyboardHeight(separateReadingRow: true))
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func tap(_ faces: [String], in controller: KeyboardViewController) throws {
    for face in faces { try button("fourteenKey\(face)", in: controller).sendActions(for: .primaryActionTriggered) }
    controller.view.layoutIfNeeded()
  }

  /// 正在组字：分词键只在组字时写「分词」。
  private func composing(_ controller: KeyboardViewController) -> Bool {
    (try? button("fourteenKeySeparator", in: controller).configuration?.title) == "分词"
  }

  private func preedit(in controller: KeyboardViewController) -> String {
    (try? button("preeditButton", in: controller).configuration?.title) ?? ""
  }

  private func candidates(in controller: KeyboardViewController) -> [String] {
    descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
      .compactMap { $0.configuration?.attributedTitle.map { String($0.characters) } ?? $0.configuration?.title }
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  /// 视图和它的每一层父视图都没有隐藏。
  private func shown(_ view: UIView) -> Bool {
    sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden }
  }

  private func view(_ identifier: String, in controller: KeyboardViewController) throws -> UIView {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier },
                  "No view with accessibility identifier \(identifier).")
  }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(view(identifier, in: controller) as? UIButton, "\(identifier) is not a button.")
  }

  private func frame(_ identifier: String, in controller: KeyboardViewController) throws -> CGRect {
    let view = try view(identifier, in: controller)
    return view.convert(view.bounds, to: controller.view)
  }

  private func attachScreenshot(of controller: KeyboardViewController, named name: String) {
    let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    }
    let attachment = XCTAttachment(image: image)
    attachment.name = name
    attachment.lifetime = .keepAlways
    add(attachment)
  }
}

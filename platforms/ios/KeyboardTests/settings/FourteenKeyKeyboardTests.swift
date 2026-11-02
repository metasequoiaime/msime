import XCTest
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// 全拼、双拼和五笔 14 键：桥接层、键面、读屏、读音选择、布局热加载与 iPad 排布保持一致。
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

  /// 五笔 14 键：同一张键面交给五笔，QW QW 展开成 qq qw wq ww 查码表。读音是首选编码的前几位，没有拼音选择；硬件字母在 14 键组字中不处理、空闲时照常走 26 键五笔；网格在会话重建后仍在，换回 26 键五笔或别的方案后组码不再被收下。
  func testTheBridgeTypesWubiOnTheFourteenKeys() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-wubi-fourteen-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNil(bridge.switchToFourteenKeyWubi().diagnosticText)
    func type(_ codes: String, on bridge: MetasequoiaInputSessionBridge) -> MetasequoiaInputSnapshot {
      var snapshot = bridge.cancel()
      for code in codes { snapshot = bridge.gridKey(String(code)) }
      return snapshot
    }
    func assertTypesNi(_ bridge: MetasequoiaInputSessionBridge, _ context: String) throws {
      let snapshot = type("qq", on: bridge)
      XCTAssertTrue(snapshot.isHandled, context)
      XCTAssertTrue(snapshot.candidates.contains("你"), "\(context): \(snapshot.candidates)")
      XCTAssertTrue(snapshot.nineKeySpellings.isEmpty, "wubi has no pinyin to choose: \(context)")
      XCTAssertEqual(snapshot.nineKeyReading, snapshot.preedit, context)
      XCTAssertEqual(snapshot.nineKeyReading.count, 2, context)
      XCTAssertTrue(snapshot.nineKeyReading.allSatisfy { "qw".contains($0) }, "\(context): \(snapshot.nineKeyReading)")
      let top = try XCTUnwrap(snapshot.candidateCodes.first, context)
      XCTAssertTrue(top.hasPrefix(snapshot.nineKeyReading), "\(context): the reading is the top code \(top)")
    }
    try assertTypesNi(bridge, "86")

    // 14 键组字中硬件字母不处理，组字照样能接着打。
    var snapshot = type("qq", on: bridge)
    XCTAssertFalse(bridge.handleCharacter("a").isHandled, "a hardware letter during a 14-key composition")
    snapshot = bridge.gridKey("q")
    XCTAssertTrue(snapshot.isHandled, "the grid composition went on")
    XCTAssertNil(snapshot.commitText)
    snapshot = type("qq", on: bridge)
    let ni = try XCTUnwrap(snapshot.candidates.firstIndex(of: "你"))
    XCTAssertEqual(bridge.selectCandidate(at: UInt(ni)).commitText, "你")

    // 空闲时硬件字母走 26 键五笔；26 键组字中网格键不处理。
    _ = bridge.cancel()
    snapshot = bridge.handleCharacter("a")
    XCTAssertTrue(snapshot.candidates.contains("工"), "\(snapshot.candidates)")
    XCTAssertFalse(bridge.gridKey("q").isHandled, "a grid key during a 26-key composition")
    _ = bridge.cancel()

    // 回车（原样上屏）在 14 键下由运行时改为结束组字取首选，不把组码 qq 写出去。
    snapshot = type("qq", on: bridge)
    let top = try XCTUnwrap(snapshot.candidates.first)
    XCTAssertEqual(bridge.commitRaw().commitText, top)

    // 最多四码：QW 四次是 qqqq（金）、wwww（人）等，第五键先把首选顶上屏，再开始下一个字。
    snapshot = type("qqqq", on: bridge)
    if snapshot.commitText == nil {
      XCTAssertEqual(snapshot.preedit.count, 4, snapshot.preedit)
      let fifth = bridge.gridKey("q")
      XCTAssertNotNil(fifth.commitText, "the fifth key commits the top candidate")
      XCTAssertEqual(fifth.preedit.count, 1, fifth.preedit)
    }
    _ = bridge.cancel()

    // 你好是 wqvb：QW QW CV BN。四码唯一时按偏好自动上屏，否则在候选里。
    snapshot = type("qqcb", on: bridge)
    XCTAssertTrue(snapshot.commitText == "你好" || snapshot.candidates.contains("你好"),
                  "\(snapshot.commitText ?? "") \(snapshot.candidates)")
    _ = bridge.cancel()

    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    try assertTypesNi(bridge, "rebuilt")
    _ = bridge.cancel()
    _ = bridge.switchToWubi()
    XCTAssertFalse(bridge.gridKey("q").isHandled, "26-key wubi")
    XCTAssertNil(bridge.switchToFourteenKeyWubi().diagnosticText)
    _ = bridge.switch(toShuangpin: false)
    XCTAssertFalse(bridge.gridKey("q").isHandled, "quanpin 26 after wubi 14")

    // 共享文档记着 `touch_wubi_layout` 时，新会话由 Host API 直接以五笔 14 键开始，换成 98 码表也一样。
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["scheme"] = "wubi"
      $0["touch_wubi_layout"] = "fourteen_key"
      $0["wubi_profile"] = "wubi98"
    })
    let fresh = MetasequoiaInputSessionBridge(stateRoot: state)
    snapshot = type("qq", on: fresh)
    XCTAssertTrue(snapshot.isHandled, "the document's wubi layout opened the grid")
    XCTAssertTrue(snapshot.nineKeySpellings.isEmpty)
    _ = fresh.cancel()
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["touch_wubi_layout"] = "twenty_six_key"
    })
    XCTAssertFalse(MetasequoiaInputSessionBridge(stateRoot: state).gridKey("q").isHandled, "26-key wubi document")
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
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "fourteenKeyQW" },
                   "26 键启动不预建尚未使用的 14 键行")
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let initialHeight = height.constant

    func reload(until done: () -> Bool) {
      controller.viewWillAppear(false)
      let deadline = Date().addingTimeInterval(15)
      while !done() && Date() < deadline {
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
      }
      controller.view.layoutIfNeeded()
    }
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.fourteenKey))
    reload {
      descendants(controller.view).contains { $0.accessibilityIdentifier == "fourteenKeyQW" && shown($0) }
    }
    let qw = try button("fourteenKeyQW", in: controller)
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

  // MARK: - 五笔 14 键

  /// 五笔 14 键画同一张键面，左端键总是「符」；QW QW 的读音行是首选编码的前几位，组码不写进输入框，没有拼音选择条，其余候选照样有编码提示，展开的是五笔的候选网格而不是九键三栏。长按、退格、英文、网址输入框和数字页与全拼 14 键相同。
  func testWubiFourteenKeysTypeTheTopCode() throws {
    let restore = try preserveWubiLayout()
    defer { restore() }
    XCTAssertTrue(InputSchemePreference.select(.wubi))
    XCTAssertTrue(TouchWubiLayoutPreference.save(.fourteenKey))
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant,
                   KeyboardViewController.keyboardHeight(separateReadingRow: true))
    for key in FourteenKeyLayout.rows.joined() {
      XCTAssertTrue(shown(try button("fourteenKey\(key.face)", in: controller)), key.face)
    }
    XCTAssertFalse(shown(try button("shiftButton", in: controller)))
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" && shown($0) })
    let separator = try button("fourteenKeySeparator", in: controller)
    XCTAssertEqual(separator.configuration?.title, "符")

    try tap(["QW", "QW"], in: controller)
    let reading = preedit(in: controller)
    XCTAssertEqual(reading.count, 2, reading)
    XCTAssertTrue(reading.allSatisfy { "qw".contains($0) }, "the reading row shows the top code, not the group letters: \(reading)")
    XCTAssertTrue(candidates(in: controller).contains { $0.contains("你") }, "\(candidates(in: controller))")
    XCTAssertEqual(controller.inlineMarkedText, "", "the 14-key codes are not written into the editor")
    XCTAssertEqual(separator.configuration?.title, "符", "wubi has no syllables to split")
    XCTAssertEqual(separator.accessibilityLabel, "符号")
    XCTAssertNil(descendants(controller.view).first { ($0.accessibilityIdentifier ?? "").hasPrefix("readingSpelling_") && shown($0) })
    let strip = try view("readingSpellingStrip", in: controller)
    XCTAssertFalse(shown(strip), "no pinyin strip for wubi")
    // 没有拼音选择条时读音行右边整段空着，点在那里也是点读音（#10474）。
    let preeditButton = try button("preeditButton", in: controller)
    let readingRow = try view("compositionRow", in: controller)
    XCTAssertTrue(readingRow.hitTest(CGPoint(x: readingRow.bounds.maxX - 1, y: readingRow.bounds.midY), with: nil) === preeditButton,
                  "the empty end of the reading row is the reading's to tap")
    let hinted = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden
        && ($0.accessibilityLabel ?? "").contains("还需输入") }
    XCTAssertFalse(hinted.isEmpty, "wubi code hints still show on the 14 keys")
    attachScreenshot(of: controller, named: "五笔十四键 QW QW")

    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertNotNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanelSpellingColumn" },
                 "wubi expands into its own candidate grid, not the nine-key columns")
    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })

    // 长按 QW 弹出 q、w，选中先结束组字再上屏字母。
    let qw = try button("fourteenKeyQW", in: controller)
    let hold = try XCTUnwrap(qw.gestureRecognizers?.compactMap { $0 as? UILongPressGestureRecognizer }.first)
    hold.state = .began
    controller.perform(NSSelectorFromString("handleFourteenKeyHold:"), with: hold)
    try button("nineKeyHoldOption-w", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(wubiComposing(controller), "choosing a letter finished the composition first")

    // 退格逐键删组码。
    try tap(["QW", "QW"], in: controller)
    for _ in 0..<2 {
      try button("fourteenKeyDelete", in: controller).sendActions(for: .touchDown)
      try button("fourteenKeyDelete", in: controller).sendActions(for: .touchUpInside)
    }
    XCTAssertFalse(wubiComposing(controller))

    // 回车结束组字，与全拼 14 键一样不把组码当原文上屏。
    try tap(["QW", "QW"], in: controller)
    try button("returnKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(wubiComposing(controller))

    // 组字中点「符」：先上屏首选，再打开符号面板，不送分词符。
    try tap(["QW", "QW"], in: controller)
    separator.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(wubiComposing(controller), "符 finished the wubi composition")
    XCTAssertFalse(preedit(in: controller).contains("'"))
  }

  /// 五笔 14 键的回落与全拼 14 键相同：英文模式、网址输入框和本地输入画 26 键，回到中文仍是 14 键；数字页的返回键印「14键」。
  func testWubiFourteenKeysFallBackToTwentySixKeys() throws {
    let restore = try preserveWubiLayout()
    defer { restore() }
    XCTAssertTrue(InputSchemePreference.select(.wubi))
    XCTAssertTrue(TouchWubiLayoutPreference.save(.fourteenKey))
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    let qw = try button("fourteenKeyQW", in: controller)
    XCTAssertTrue(shown(qw))

    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(qw))
    XCTAssertNotNil(descendants(controller.view).first { ($0.accessibilityLabel ?? "").hasSuffix(" Q") && shown($0) })
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(qw))

    let ordinary = UUID()
    controller.applyInputContext(keyboardType: .default, documentIdentifier: ordinary)
    controller.applyInputContext(keyboardType: .URL, documentIdentifier: UUID())
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(qw), "a URL field draws the 26 letters")
    controller.applyInputContext(keyboardType: .default, documentIdentifier: ordinary)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(qw), "leaving the URL field restores the 14 keys")

    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(qw))
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "14键")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(qw))

    XCTAssertEqual(InputSchemePreference.scheme, .wubi, "the fallbacks do not rewrite the scheme")
    XCTAssertEqual(TouchWubiLayoutPreference.layout, .fourteenKey)
  }

  /// 设置页改五笔布局：空闲时键盘重载立即换键面和读音行；组字期间读入的布局留到组字结束再应用，与双拼相同。
  func testWubiLayoutReloadIsDeferredDuringAComposition() throws {
    let restore = try preserveWubiLayout()
    let previousHints = KeyboardLayoutPreference.shuangpinKeyHints
    defer {
      restore()
      KeyboardLayoutPreference.shuangpinKeyHints = previousHints
    }
    XCTAssertTrue(InputSchemePreference.select(.wubi))
    XCTAssertTrue(TouchWubiLayoutPreference.save(.twentySixKey))
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "fourteenKeyQW" },
                   "26 键启动不预建尚未使用的 14 键行")
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let initialHeight = height.constant

    func reload(until done: () -> Bool) {
      controller.viewWillAppear(false)
      let deadline = Date().addingTimeInterval(15)
      while !done() && Date() < deadline {
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
      }
      controller.view.layoutIfNeeded()
    }
    XCTAssertTrue(TouchWubiLayoutPreference.save(.fourteenKey))
    reload {
      descendants(controller.view).contains { $0.accessibilityIdentifier == "fourteenKeyQW" && shown($0) }
    }
    let qw = try button("fourteenKeyQW", in: controller)
    XCTAssertTrue(shown(qw))
    XCTAssertEqual(height.constant - initialHeight, readingRowCost)
    XCTAssertEqual(InputSchemePreference.scheme, .wubi)
    try tap(["QW", "QW"], in: controller)
    XCTAssertTrue(candidates(in: controller).contains { $0.contains("你") }, "\(candidates(in: controller))")
    XCTAssertTrue(wubiComposing(controller))

    // 组字期间读入 26 键设置，当前键面保持 14 键；上屏后同一控制器立即换成 26 键。
    XCTAssertTrue(TouchWubiLayoutPreference.save(.twentySixKey))
    KeyboardLayoutPreference.shuangpinKeyHints = false
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences {
      $0["touch_shuangpin_key_hints"] = true
    })
    // 键位提示只由重载回调镜像，用它作重载完成的信号；布局镜像已被 save 同步写入，不能当信号。
    reload { KeyboardLayoutPreference.shuangpinKeyHints }
    XCTAssertTrue(KeyboardLayoutPreference.shuangpinKeyHints)
    XCTAssertTrue(shown(qw), "the layout waits for the composition")
    XCTAssertTrue(wubiComposing(controller))
    try button("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(qw))
    XCTAssertEqual(height.constant, initialHeight)
    XCTAssertEqual(InputSchemePreference.scheme, .wubi)
    let g = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 G" && shown($0) } as? UIButton)
    g.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(candidates(in: controller).contains { $0.hasPrefix("一") }, "back on 26-key wubi: \(candidates(in: controller))")
  }

  /// 五笔 14 键的编码提示按键比对：编码前几位逐位落在已按的键上就算打过，ZX 键在引擎组码里记作 `x`。
  func testWubiGridCodeHintsMatchCodesKeyByKey() {
    XCTAssertEqual(FourteenKeyLayout.typedPrefix(of: "wqiy", keys: "qq"), "wq")
    XCTAssertEqual(FourteenKeyLayout.typedPrefix(of: "qqqq", keys: "qq"), "qq")
    XCTAssertEqual(FourteenKeyLayout.typedPrefix(of: "xa", keys: "x"), "x")
    XCTAssertEqual(FourteenKeyLayout.typedPrefix(of: "xa", keys: "z"), "x")
    XCTAssertEqual(FourteenKeyLayout.typedPrefix(of: "vb", keys: "cb"), "vb")
    XCTAssertNil(FourteenKeyLayout.typedPrefix(of: "ab", keys: "q"))
    XCTAssertNil(FourteenKeyLayout.typedPrefix(of: "w", keys: "qq"), "a code shorter than the keys")
    XCTAssertNil(FourteenKeyLayout.typedPrefix(of: "wq", keys: ""))
    XCTAssertEqual(WubiCodeHintPreference.hint(code: "wqiy", typed: FourteenKeyLayout.typedPrefix(of: "wqiy", keys: "qq") ?? "",
                                               answeredByPinyinFallback: false), "iy")
    // 统计仍记在五笔来源上，键位 id 与全拼 14 键相同。
    XCTAssertEqual(TypingSource(rawValue: ChineseInputScheme.wubi.rawValue), .wubi)
    XCTAssertEqual(FourteenKeyLayout.key(for: "w")?.keyID, "FourteenQW")
    XCTAssertEqual(FourteenKeyLayout.key(for: "x")?.keyID, "FourteenZX")
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
    // 读音的可点区域放大到整行（#10474），但选择条叠在上面，落在选择条里的触摸仍归选择条。
    let row = try view("compositionRow", in: controller)
    let shiCenter = shi.convert(CGPoint(x: shi.bounds.midX, y: shi.bounds.midY), to: row)
    XCTAssertTrue(try XCTUnwrap(row.hitTest(shiCenter, with: nil)).isDescendant(of: strip), "the strip keeps its own touches")
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

  /// 五笔 14 键的左端键总是「符」，所以按读音行判断是否在组字：空闲时它写「水杉输入法」。
  private func wubiComposing(_ controller: KeyboardViewController) -> Bool {
    let title = preedit(in: controller)
    return !title.isEmpty && title != "水杉输入法"
  }

  /// 记下共享文档、方案和五笔布局镜像，返回恢复它们的闭包。
  private func preserveWubiLayout() throws -> () -> Void {
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    let previousLayout = TouchWubiLayoutPreference.layout
    return {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = document }
      TouchWubiLayoutPreference.layout = previousLayout
    }
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

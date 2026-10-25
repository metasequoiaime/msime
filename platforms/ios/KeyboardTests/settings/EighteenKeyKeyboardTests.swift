import XCTest
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// 全拼和双拼 18 键：键表、桥接、七个等宽键的排布、双拼横滑限定字母、布局热加载与 iPad 排布，与 Android 的十八键一致。
@MainActor
final class EighteenKeyKeyboardTests: XCTestCase {
  private var previousScheme = InputSchemePreference.scheme
  private var previousLayout = TouchShuangpinLayoutPreference.layout
  private var previousDocument: [String: Any]?

  // 认领全部方案，免得对 `InputSchemePreference.scheme` 的赋值落到模拟器上残留的别的方案。18 键要用户自己打开，不先启用就选不上。见 InputSchemeTestSupport。
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    previousScheme = InputSchemePreference.scheme
    previousLayout = TouchShuangpinLayoutPreference.layout
    previousDocument = MetasequoiaInputSessionBridge.loadSharedPreferences()
    InputSchemePreference.scheme = .eighteenKey
  }

  override func tearDown() {
    if let previousDocument {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = previousDocument }
    }
    TouchShuangpinLayoutPreference.layout = previousLayout
    InputSchemePreference.scheme = previousScheme
    super.tearDown()
  }

  // MARK: - 键表

  /// 与 Android、引擎同一张表：七、六、五个键，组码是每组的首字母，26 个字母依次落在引擎的 `EIGHTEEN_KEYPAD` 上；统计 id 是 `Eighteen` 一组。
  func testTheKeyTableIsTheEngineGrid() {
    XCTAssertEqual(EighteenKeyLayout.rows.map { $0.map(\.face) },
                   [["Q", "WE", "RT", "Y", "U", "IO", "P"], ["A", "SD", "FG", "H", "JK", "L"], ["Z", "XC", "V", "BN", "M"]])
    XCTAssertEqual(EighteenKeyLayout.codeTable, "abxswffhijjlmbipqrsruvwxyz")
    let we = EighteenKeyLayout.rows[0][1], q = EighteenKeyLayout.rows[0][0]
    XCTAssertEqual(we.code, "w")
    XCTAssertEqual(we.accessibilityLabel, "按键 W E")
    XCTAssertEqual(q.accessibilityLabel, "字母 Q")
    XCTAssertEqual(we.holdLetters, ["w", "e"])
    XCTAssertTrue(q.holdLetters.isEmpty)
    XCTAssertEqual(we.keyID, "EighteenWE")
    XCTAssertEqual(EighteenKeyLayout.rows.joined().compactMap(\.keyID).count, 18,
                   "every key counts on an id client-services accepts, or the whole batch is refused")
    XCTAssertEqual(FourteenKeyLayout.rows[0][0].keyID, "FourteenQW", "the 14 keys keep their own ids")
  }

  /// 横滑的规则与 Android 相同：超过 24pt 且横向大于纵向 1.5 倍才选字母，左滑取左字母、右滑取右字母，单字母键不选。
  func testTheSwipePolicyMatchesAndroid() {
    XCTAssertEqual(EighteenKeySwipePolicy.letter("we", dx: -30, dy: 0), "W")
    XCTAssertEqual(EighteenKeySwipePolicy.letter("we", dx: 30, dy: 5), "E")
    XCTAssertNil(EighteenKeySwipePolicy.letter("we", dx: 24, dy: 0), "the threshold itself is still a tap")
    XCTAssertEqual(EighteenKeySwipePolicy.letter("we", dx: 24.5, dy: 0), "E")
    XCTAssertNil(EighteenKeySwipePolicy.letter("we", dx: 30, dy: 20), "a mostly vertical drag is not a swipe")
    XCTAssertEqual(EighteenKeySwipePolicy.letter("we", dx: 31, dy: 20), "E")
    XCTAssertNil(EighteenKeySwipePolicy.letter("q", dx: 40, dy: 0))
    XCTAssertNil(EighteenKeySwipePolicy.letter("WE", dx: 40, dy: 0))
    XCTAssertNil(EighteenKeySwipePolicy.letter("we", dx: .nan, dy: 0))
  }

  // MARK: - 桥接

  /// 全拼 18 键按组码打出「你好」，会话重建后网格仍在；全拼不收横滑的精确大写，换回 26 键后网格关掉。
  func testTheBridgeTypesNihaoOnTheQuanpinGridAndKeepsItAcrossARebuild() throws {
    let bridge = MetasequoiaInputSessionBridge()
    XCTAssertNil(bridge.switchToEighteenKey().diagnosticText)
    func type(_ codes: String) -> MetasequoiaInputSnapshot {
      var snapshot = bridge.cancel()
      for code in codes { snapshot = bridge.gridKey(String(code)) }
      return snapshot
    }
    var snapshot = type("bihai")
    XCTAssertTrue(snapshot.isHandled)
    XCTAssertTrue(snapshot.candidates.contains("你好"), "\(snapshot.candidates)")
    XCTAssertFalse(bridge.gridKey("N").isHandled, "the exact marker belongs to the shuangpin grid only")
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    snapshot = type("bihai")
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session forgot the 18-key grid")
    _ = bridge.cancel()
    _ = bridge.switch(toShuangpin: false)
    XCTAssertFalse(bridge.gridKey("b").isHandled)
  }

  /// 双拼 18 键沿用当前 profile：整组点按有 bi 和 ni 两种读音，横滑送的大写精确字母只留 ni；会话重建后仍是双拼 18 键。
  func testTheBridgeTypesShuangpinEighteenKeyAndHonoursExactLetters() throws {
    let bridge = MetasequoiaInputSessionBridge()
    // 各方案的 hao：小鹤 hc、自然码和微软 hk、首道 hd，按 18 键分组是 hx、hj、hj、hs。
    for (profile, codes) in [("xiaohe", "bihx"), ("ziranma", "bihj"), ("microsoft", "bihj"), ("shoudao", "bihs")] {
      XCTAssertNil(bridge.switchToEighteenKeyShuangpin(profile: profile).diagnosticText)
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

        snapshot = bridge.cancel()
        for code in ["b", "i"] { snapshot = bridge.gridKey(code) }
        XCTAssertTrue(snapshot.nineKeySpellings.contains("ni"), "\(profile): \(snapshot.nineKeySpellings)")
        XCTAssertTrue(snapshot.nineKeySpellings.contains("bi"), "\(profile): \(snapshot.nineKeySpellings)")
        snapshot = bridge.cancel()
        for code in ["N", "I"] { snapshot = bridge.gridKey(code) }
        XCTAssertTrue(snapshot.isHandled, profile)
        XCTAssertTrue(snapshot.nineKeySpellings.contains("ni"), "\(profile): \(snapshot.nineKeySpellings)")
        XCTAssertFalse(snapshot.nineKeySpellings.contains("bi"), "\(profile): the exact letters still offered bi")
        for code in codes.dropFirst(2) { snapshot = bridge.gridKey(String(code)) }
        XCTAssertTrue(snapshot.candidates.contains("你好"), "\(profile): \(snapshot.candidates)")
      }
      _ = bridge.cancel()
    }
  }

  // MARK: - 键面

  /// 三排键与 Android 的七份权重一致：所有字母键、分词键和 ⌫ 等宽，第二排居中、两侧各空出半个键加半个键距，第三排两端与第一排对齐；键距改变时也只补一次键距。键不越界，与 26 键同高，没有 Shift 和数字行。
  func testEveryKeyHasTheSameWidthAndTheMiddleRowIsCentred() throws {
    let previousSpacing = KeyboardLayoutPreference.keySpacing
    defer { _ = KeyboardLayoutPreference.saveGeometry(keySpacing: previousSpacing) }
    for spacing in [3.0, 6.0] {
      XCTAssertTrue(KeyboardLayoutPreference.saveGeometry(keySpacing: spacing))
      for width in [320.0, 390.0, 414.0] {
        let controller = makeController(width: width)
        defer { controller.viewDidDisappear(false) }
        let reference = try button("returnKey", in: controller).bounds.height
        let first = try frame("eighteenKeyQ", in: controller)
        var widths: [CGFloat] = []
        for key in EighteenKeyLayout.rows.joined() {
          let button = try button("eighteenKey\(key.face)", in: controller)
          XCTAssertTrue(shown(button), key.face)
          XCTAssertEqual(button.configuration?.title, key.face)
          XCTAssertEqual(button.accessibilityLabel, key.accessibilityLabel)
          XCTAssertEqual(button.bounds.height, reference, accuracy: 0.5, key.face)
          let frame = button.convert(button.bounds, to: controller.view)
          XCTAssertGreaterThanOrEqual(frame.minX, KeyboardFormFactor.phone.padding.leading - 0.5, "\(key.face) at \(width)")
          XCTAssertLessThanOrEqual(frame.maxX, width - KeyboardFormFactor.phone.padding.trailing + 0.5, "\(key.face) at \(width)")
          // 两个字母的键面在窄屏上缩小而不越出键帽。
          if let label = button.titleLabel, !(label.text ?? "").isEmpty {
            XCTAssertLessThanOrEqual(label.frame.maxX, button.bounds.maxX + 0.5, "\(key.face) at \(width)")
            XCTAssertGreaterThanOrEqual(label.frame.minX, -0.5, "\(key.face) at \(width)")
          }
          widths.append(frame.width)
        }
        let separator = try frame("eighteenKeySeparator", in: controller)
        let delete = try frame("eighteenKeyDelete", in: controller)
        widths += [separator.width, delete.width]
        for (index, value) in widths.enumerated() {
          XCTAssertEqual(value, first.width, accuracy: 0.5, "key \(index) at \(width)pt, spacing \(spacing)")
        }
        let we = try frame("eighteenKeyWE", in: controller), p = try frame("eighteenKeyP", in: controller)
        let a = try frame("eighteenKeyA", in: controller), l = try frame("eighteenKeyL", in: controller)
        // 第二排的 A 在 Q 和 WE 正中，L 在 IO 和 P 正中：半个键加半个键距。
        XCTAssertEqual(a.minX, (first.minX + we.minX) / 2, accuracy: 0.5, "at \(width)pt, spacing \(spacing)")
        XCTAssertEqual(l.maxX, (try frame("eighteenKeyIO", in: controller).maxX + p.maxX) / 2, accuracy: 0.5)
        XCTAssertEqual(separator.minX, first.minX, accuracy: 0.5)
        XCTAssertEqual(delete.maxX, p.maxX, accuracy: 0.5)
        XCTAssertEqual(try button("eighteenKeySeparator", in: controller).configuration?.title, "符")
        XCTAssertFalse(shown(try button("shiftButton", in: controller)))
        XCTAssertFalse(shown(try button("fourteenKeyQW", in: controller)), "the 14 keys stay hidden")
        XCTAssertFalse(shown(try view("numberRow", in: controller)))
        XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 18 键")
        XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant,
                       KeyboardViewController.keyboardHeight(separateReadingRow: true))
        attachScreenshot(of: controller, named: "Eighteen-key keyboard at \(Int(width))pt, spacing \(spacing)")
      }
    }
  }

  // MARK: - 输入

  /// 全拼 18 键点 BN IO H A IO 打出「你好」：组码不写进输入框，读音行显示拼音，分词键在组字中写「分词」；全拼不接横滑。
  func testTypingNihaoOnTheQuanpinKeys() throws {
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    try tap(["BN", "IO", "H", "A", "IO"], in: controller)
    XCTAssertTrue(candidates(in: controller).contains { $0.contains("你好") }, "\(candidates(in: controller))")
    XCTAssertFalse(preedit(in: controller).isEmpty)
    XCTAssertEqual(controller.inlineMarkedText, "", "the group codes are not written into the editor")
    XCTAssertEqual(try button("eighteenKeySeparator", in: controller).configuration?.title, "分词")
    let swipe = try swipeGesture(on: "BN", in: controller)
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(swipe), "quanpin 18 keys only tap, as on Android")
    attachScreenshot(of: controller, named: "Eighteen-key quanpin nihao")
  }

  /// 双拼 18 键：BN 右滑得 N、IO 左滑得 I，读音只剩 ni；拖动时键面换成选中的字母，松手后换回。回到起点、被取消都不输入；之后的点按仍是整组。
  func testShuangpinSwipesPickTheExactLetter() throws {
    XCTAssertTrue(InputSchemePreference.select(.shuangpin))
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.eighteenKey))
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    XCTAssertTrue(shown(try button("eighteenKeyBN", in: controller)))
    let bn = try swipeGesture(on: "BN", in: controller)
    XCTAssertTrue(controller.gestureRecognizerShouldBegin(bn))
    XCTAssertTrue(controller.gestureRecognizerShouldBegin(try swipeGesture(on: "IO", in: controller)))
    XCTAssertNil(try button("eighteenKeyH", in: controller).gestureRecognizers?.first { $0 is EighteenKeySwipeGestureRecognizer },
                 "single-letter keys do not swipe")

    // 回到起点附近：已经接手，不补一次点按，也不输入。
    try swipe("BN", through: [40, 4], in: controller)
    XCTAssertFalse(composing(controller), "returning to the middle typed something")
    try swipe("BN", through: [40], cancelled: true, in: controller)
    XCTAssertFalse(composing(controller), "a cancelled swipe typed something")
    XCTAssertEqual(try button("eighteenKeyBN", in: controller).configuration?.title, "BN")

    try swipe("BN", through: [40], in: controller, whileMoving: { title in XCTAssertEqual(title, "N") })
    try swipe("IO", through: [40, -40], in: controller, whileMoving: { title in XCTAssertEqual(title, "I") })
    XCTAssertEqual(try button("eighteenKeyIO", in: controller).configuration?.title, "IO")
    XCTAssertTrue(shown(try button("readingSpelling_ni", in: controller)))
    XCTAssertNil(spellingButton("bi", in: controller), "the exact letters still offered bi")
    try tap(["H", "XC"], in: controller)
    XCTAssertTrue(candidates(in: controller).contains { $0.contains("你好") }, "\(candidates(in: controller))")
    XCTAssertEqual(controller.inlineMarkedText, "", "the exact markers are not written into the editor")
    attachScreenshot(of: controller, named: "Eighteen-key shuangpin exact swipe")

    // 清空后点按仍是整组，bi 和 ni 都在。
    for _ in 0..<4 { try deleteOnce(in: controller) }
    XCTAssertFalse(composing(controller))
    try tap(["BN", "IO"], in: controller)
    XCTAssertTrue(shown(try button("readingSpelling_ni", in: controller)))
    XCTAssertTrue(shown(try button("readingSpelling_bi", in: controller)))
  }

  /// 快速一甩：UIKit 只送了一次没过阈值的移动，抬手那一点才过阈值。与 Android 在抬手时也判一次横滑相同，BN 右甩取 N、IO 左甩取 I，读音只剩 ni；按键的抬手被扣下等手势判完（`delaysTouchesEnded`），识别了就不再补一次整组点按。抬手时仍在阈值以内则手势失败，交给按键算普通点按。
  func testAFlickThatCrossesTheThresholdOnlyAtLiftOffPicksTheLetter() throws {
    XCTAssertTrue(InputSchemePreference.select(.shuangpin))
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.eighteenKey))
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    XCTAssertTrue(try swipeGesture(on: "BN", in: controller).delaysTouchesEnded,
                  "the key's touch-up must wait for the swipe, or a lift-off flick also taps the group")

    // 没有真实事件分发时，失败的识别器当场被放回 `.possible`，所以这里只核对它没有识别。
    XCTAssertNotEqual(try swipe("BN", through: [15], liftAt: 12, in: controller), .ended)
    XCTAssertFalse(composing(controller), "a lift-off still inside the threshold is the key's own tap")

    XCTAssertEqual(try swipe("BN", through: [15], liftAt: 30, in: controller), .ended)
    XCTAssertEqual(try swipe("IO", through: [-15], liftAt: -30, in: controller), .ended)
    XCTAssertEqual(try button("eighteenKeyBN", in: controller).configuration?.title, "BN")
    XCTAssertTrue(shown(try button("readingSpelling_ni", in: controller)))
    XCTAssertNil(spellingButton("bi", in: controller), "the lift-off flick was taken as the whole BN group")
  }

  /// 长按两个字母的键弹出这两个字母，与 14 键相同；单字母键没有长按。
  func testHoldingAKeyOffersItsLetters() throws {
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    for key in EighteenKeyLayout.rows.joined() {
      let holds = (try button("eighteenKey\(key.face)", in: controller).gestureRecognizers ?? [])
        .compactMap { $0 as? UILongPressGestureRecognizer }
      XCTAssertEqual(holds.count, key.holdLetters.isEmpty ? 0 : 1, key.face)
    }
    let we = try button("eighteenKeyWE", in: controller)
    let hold = try XCTUnwrap(we.gestureRecognizers?.compactMap { $0 as? UILongPressGestureRecognizer }.first)
    hold.state = .began
    controller.perform(NSSelectorFromString("handleFourteenKeyHold:"), with: hold)
    XCTAssertTrue(shown(try button("nineKeyHoldOption-w", in: controller)))
    XCTAssertTrue(shown(try button("nineKeyHoldOption-e", in: controller)))
  }

  /// 设置页把双拼布局从 14 键换成 18 键：空闲时键面立即换成 18 键，方案和读音行高度不变；微软双拼的 L 键说明分号入口。
  func testShuangpinLayoutReloadSwitchesBetweenFourteenAndEighteenKeys() throws {
    XCTAssertTrue(InputSchemePreference.select(.microsoft))
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.fourteenKey))
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let fourteenHeight = height.constant
    XCTAssertTrue(shown(try button("fourteenKeyQW", in: controller)))
    let we = try button("eighteenKeyWE", in: controller)
    XCTAssertFalse(shown(we))

    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.eighteenKey))
    controller.viewWillAppear(false)
    let deadline = Date().addingTimeInterval(15)
    while !shown(we) && Date() < deadline { RunLoop.current.run(until: Date().addingTimeInterval(0.05)) }
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(we))
    XCTAssertFalse(shown(try button("fourteenKeyQW", in: controller)))
    XCTAssertEqual(height.constant, fourteenHeight)
    XCTAssertEqual(InputSchemePreference.scheme, .microsoft)
    let l = try button("eighteenKeyL", in: controller)
    XCTAssertEqual(l.configuration?.title, "L;")
    XCTAssertEqual(l.accessibilityLabel, "字母 L 和分号")
    XCTAssertFalse(shown(try button("microsoftFinalKey", in: controller)))
    try tap(["Q", "L"], in: controller)
    XCTAssertTrue(shown(try button("readingSpelling_qing", in: controller)), "微软 q; 的分号由 L 键承载")
  }

  /// 英文模式画 26 键，切回中文仍是 18 键；123 打开数字九宫格，返回键印「18键」，返回后仍是 18 键。
  func testEnglishAndTheSymbolLayerReturnToTheEighteenKeys() throws {
    let controller = makeController(width: 390)
    defer { controller.viewDidDisappear(false) }
    let we = try button("eighteenKeyWE", in: controller)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(we))
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(we))
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(we))
    XCTAssertEqual(try button("layoutToggleButton", in: controller).configuration?.title, "18键")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(we))
  }

  // MARK: - iPad

  /// iPad 与 14 键一样画完整的 18 键：不分体，不画数字行，底行是手机 26 键底行，七个键等宽。
  func testTheTabletDrawsTheEighteenKeysWhole() throws {
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = .pad
    controller.traitOverrides.horizontalSizeClass = .regular
    controller.loadViewIfNeeded()
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    controller.view.frame = CGRect(x: 0, y: 0, width: 1032, height: height)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    defer { controller.viewDidDisappear(false) }
    let q = try frame("eighteenKeyQ", in: controller)
    XCTAssertTrue(shown(try button("eighteenKeyQ", in: controller)))
    XCTAssertFalse(shown(try view("numberRow", in: controller)))
    XCTAssertFalse(shown(try button("tabletLayerKey", in: controller)), "the 18 keys use the phone bottom row")
    XCTAssertTrue(shown(try button("returnKey", in: controller)))
    for id in ["eighteenKeyA", "eighteenKeyBN", "eighteenKeyDelete"] {
      XCTAssertEqual(try frame(id, in: controller).width, q.width, accuracy: 0.5, id)
    }
    XCTAssertFalse(KeyboardSplitLayout.splitsLayout(scheme: .eighteenKey, chinese: true, localMode: false))
    attachScreenshot(of: controller, named: "Eighteen-key iPad keyboard")
  }

  // MARK: - 辅助

  /// 一根手指在键面中心按下、按 `offsets`（横向，pt）移动、再抬手或被取消，交给键盘的横滑处理；`liftAt` 给出时抬手前手指已到这个横向位置，但没有为它单独送一次移动，像快速一甩时 UIKit 只在抬手时报出最后的位置。`whileMoving` 拿到最后一次移动时的键面。返回抬手或取消后识别器的状态。
  ///
  /// 测试里没有真实的事件分发，UIKit 不会在序列结束后把识别器放回 `.possible`，所以每次用一个与键上那个同样配置的新识别器，挂到同一个键上跑完再摘掉。键上原有的识别器由 `swipeGesture(on:in:)` 和委托门控单独核对。
  @discardableResult
  private func swipe(_ face: String, through offsets: [CGFloat], liftAt: CGFloat? = nil, cancelled: Bool = false,
                     in controller: KeyboardViewController,
                     whileMoving: ((String?) -> Void)? = nil) throws -> UIGestureRecognizer.State {
    let button = try button("eighteenKey\(face)", in: controller)
    let installed = try swipeGesture(on: face, in: controller)
    let handle = NSSelectorFromString("handleEighteenKeySwipe:")
    let gesture = EighteenKeySwipeGestureRecognizer(letters: installed.letters, target: controller, action: handle)
    button.addGestureRecognizer(gesture)
    defer { button.removeGestureRecognizer(gesture) }
    let center = CGPoint(x: button.bounds.midX, y: button.bounds.midY)
    let touch = ScriptedTouch(at: center)
    gesture.touchesBegan([touch], with: UIEvent())
    var recognized = false
    for offset in offsets {
      touch.point = CGPoint(x: center.x + offset, y: center.y)
      gesture.touchesMoved([touch], with: UIEvent())
      if gesture.state == .began || gesture.state == .changed {
        recognized = true
        controller.perform(handle, with: gesture)
      }
    }
    whileMoving?(button.configuration?.title)
    if let liftAt { touch.point = CGPoint(x: center.x + liftAt, y: center.y) }
    if cancelled { gesture.touchesCancelled([touch], with: UIEvent()) } else { gesture.touchesEnded([touch], with: UIEvent()) }
    // 开始识别之后 UIKit 总会再送一次结束或取消的动作，抬手时才识别的也送一次结束；没识别过的序列没有动作，按键自己处理这次点按。
    let state = gesture.state
    if recognized || state == .ended { controller.perform(handle, with: gesture) }
    controller.view.layoutIfNeeded()
    return state
  }

  private func swipeGesture(on face: String, in controller: KeyboardViewController) throws -> EighteenKeySwipeGestureRecognizer {
    try XCTUnwrap(try button("eighteenKey\(face)", in: controller).gestureRecognizers?
      .compactMap { $0 as? EighteenKeySwipeGestureRecognizer }.first, "\(face) has no swipe")
  }

  private func makeController(width: CGFloat) -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: width, height: KeyboardViewController.keyboardHeight(separateReadingRow: true))
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    controller.view.layoutIfNeeded()
    return controller
  }

  private func tap(_ faces: [String], in controller: KeyboardViewController) throws {
    for face in faces { try button("eighteenKey\(face)", in: controller).sendActions(for: .primaryActionTriggered) }
    controller.view.layoutIfNeeded()
  }

  private func deleteOnce(in controller: KeyboardViewController) throws {
    let delete = try button("eighteenKeyDelete", in: controller)
    delete.sendActions(for: .touchDown)
    delete.sendActions(for: .touchUpInside)
    controller.view.layoutIfNeeded()
  }

  /// 正在组字：分词键只在组字时写「分词」。
  private func composing(_ controller: KeyboardViewController) -> Bool {
    (try? button("eighteenKeySeparator", in: controller).configuration?.title) == "分词"
  }

  private func preedit(in controller: KeyboardViewController) -> String {
    (try? button("preeditButton", in: controller).configuration?.title) ?? ""
  }

  private func spellingButton(_ spelling: String, in controller: KeyboardViewController) -> UIView? {
    descendants(controller.view).first { $0.accessibilityIdentifier == "readingSpelling_\(spelling)" && shown($0) }
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

/// 位置由测试给出的一根手指：手势识别器只读它在键上的位置。
private final class ScriptedTouch: UITouch {
  var point: CGPoint

  init(at point: CGPoint) {
    self.point = point
    super.init()
  }

  override func location(in view: UIView?) -> CGPoint { point }
}

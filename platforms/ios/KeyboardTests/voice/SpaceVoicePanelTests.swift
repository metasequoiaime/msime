import ObjectiveC
import UIKit
import XCTest

/// 空格长按、点按语音入口与聆听面板：识别完成直接上屏，会话未开时由主 App 开启；光标拖动裁决方式同 Android。
@MainActor
final class SpaceVoicePanelTests: XCTestCase {
  private var storedScheme = InputSchemePreference.scheme

  override func setUp() {
    super.setUp()
    storedScheme = InputSchemePreference.scheme
    InputSchemePreference.scheme = .quanpin
  }

  override func tearDown() {
    InputSchemePreference.scheme = storedScheme
    super.tearDown()
  }

  func testTheSpaceBarHoldsForAndroidsLongPress() throws {
    let controller = makeController()
    for space in spaceKeys(in: controller) {
      let hold = try XCTUnwrap(space.gestureRecognizers?.first { $0.name == "spaceVoiceHold" } as? UILongPressGestureRecognizer)
      XCTAssertEqual(hold.minimumPressDuration, 0.45, accuracy: 0.001)
      XCTAssertEqual(hold.allowableMovement, 10)
      XCTAssertTrue(hold.cancelsTouchesInView, "the hold takes the touch, so no space is typed")
      XCTAssertTrue(hold.delegate === controller)
      XCTAssertNotNil(space.gestureRecognizers?.first { $0.name == "spaceCursorPan" })
    }
  }

  /// 长按只在语音真能打开时武装：开关开着、有完全访问权限、手写板上没有笔迹，并且同 Android 的 `voiceInsertionReady` 没有正在拼写的内容、不在本地输入模式里。其余情况下这次按压保持普通空格。
  func testTheHoldIsArmedOnlyWhenVoiceCanOpen() throws {
    XCTAssertTrue(KeyboardViewController.armsSpaceVoice(enabled: true, fullAccess: true, handwritingInk: false,
                                                        composing: false, localMode: false))
    XCTAssertFalse(KeyboardViewController.armsSpaceVoice(enabled: false, fullAccess: true, handwritingInk: false,
                                                         composing: false, localMode: false), "the switch is off")
    XCTAssertFalse(KeyboardViewController.armsSpaceVoice(enabled: true, fullAccess: false, handwritingInk: false,
                                                         composing: false, localMode: false), "no Full Access, no voice")
    XCTAssertFalse(KeyboardViewController.armsSpaceVoice(enabled: true, fullAccess: true, handwritingInk: true,
                                                         composing: false, localMode: false), "space commits the ink")
    XCTAssertFalse(KeyboardViewController.armsSpaceVoice(enabled: true, fullAccess: true, handwritingInk: false,
                                                         composing: true, localMode: false), "a hold while spelling stays a space")
    XCTAssertFalse(KeyboardViewController.armsSpaceVoice(enabled: true, fullAccess: true, handwritingInk: false,
                                                         composing: false, localMode: true))
    // 测试宿主没有完全访问权限：语音开关仍开着时，慢一点的空格不能被长按吞掉。
    let controller = makeController()
    let space = try XCTUnwrap(spaceKeys(in: controller).first)
    let hold = try XCTUnwrap(space.gestureRecognizers?.first { $0.name == "spaceVoiceHold" })
    XCTAssertFalse(controller.hasFullAccess)
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(hold))
  }

  /// 「长按空格语音输入」关掉后，长按改为移动光标，键面上的麦克风、VoiceOver 提示和操作里都没有语音；再打开、键盘重新出现时麦克风回来（#10143）。
  func testTheSpaceVoiceSwitchReachesTheSpaceBar() throws {
    let stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.spaceVoiceKey)
    addTeardownBlock {
      if let stored { KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.spaceVoiceKey) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.spaceVoiceKey) }
    }
    KeyboardLayoutPreference.spaceVoice = false
    let controller = makeController()
    let space = try XCTUnwrap(spaceKeys(in: controller).first)
    XCTAssertEqual(space.accessibilityCustomActions?.map(\.name), ["光标左移", "光标右移"])
    XCTAssertTrue(space.accessibilityHint?.contains("长按移动光标") ?? false)
    XCTAssertNil(space.configuration?.image, "no mic while long-press space voice is off")
    XCTAssertFalse(space.configuration?.title?.isEmpty ?? true, "the scheme label stays")
    let hold = try XCTUnwrap(space.gestureRecognizers?.first { $0.name == "spaceVoiceHold" })
    XCTAssertTrue(controller.gestureRecognizerShouldBegin(hold))
    KeyboardLayoutPreference.spaceVoice = true
    controller.viewWillAppear(false)
    XCTAssertNotNil(space.configuration?.image, "the mic returns with the switch")
    XCTAssertEqual(space.accessibilityCustomActions?.map(\.name), ["光标左移", "光标右移", "语音输入"])
    XCTAssertTrue(space.accessibilityHint?.contains("按住说话，松开上屏") ?? false)
  }

  /// 通过生产长按入口验证文档代理、实际步进、松手清理与输入框切换保护。
  func testHoldingSpaceWithoutVoiceMovesTheDocumentCaret() throws {
    let stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.spaceVoiceKey)
    addTeardownBlock {
      if let stored { KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.spaceVoiceKey) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.spaceVoiceKey) }
    }
    KeyboardLayoutPreference.spaceVoice = false
    let document = CursorDocument()
    try attach(document)
    let controller = makeController()
    let space = try XCTUnwrap(spaceKeys(in: controller).first)
    let hold = CursorHold()
    space.addGestureRecognizer(hold)
    func send(_ state: UIGestureRecognizer.State, x: CGFloat) {
      hold.simulatedState = state
      hold.x = x
      controller.perform(NSSelectorFromString("handleSpaceVoiceHold:"), with: hold)
    }
    send(.began, x: 100)
    XCTAssertEqual(space.configuration?.title, "移动光标")
    send(.changed, x: 76)
    XCTAssertEqual(document.offsets, [-2])
    send(.changed, x: 88)
    XCTAssertEqual(document.offsets, [-2, 1])
    send(.ended, x: 88)
    XCTAssertNotEqual(space.configuration?.title, "移动光标")
    XCTAssertEqual(document.insertions, [], "长按不插入空格")

    send(.began, x: 100)
    document.documentIdentifier = UUID()
    send(.changed, x: 76)
    XCTAssertEqual(document.offsets, [-2, 1], "不能把旧手势应用到新输入框")
    XCTAssertNotEqual(space.configuration?.title, "移动光标")
    send(.cancelled, x: 76)
    send(.began, x: 100)
    send(.changed, x: 112)
    XCTAssertEqual(document.offsets, [-2, 1, 1], "取消后下一次手势可以正常开始")
    send(.ended, x: 112)
  }

  private final class CursorDocument: NSObject, UITextDocumentProxy {
    var documentIdentifier = UUID()
    var offsets: [Int] = []
    var insertions: [String] = []
    var documentContextBeforeInput: String? { "abcd" }
    var documentContextAfterInput: String? { "ef" }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    var hasText: Bool { true }
    func insertText(_ text: String) { insertions.append(text) }
    func deleteBackward() {}
    func adjustTextPosition(byCharacterOffset offset: Int) { offsets.append(offset) }
    func setMarkedText(_ text: String, selectedRange: NSRange) {}
    func unmarkText() {}
  }

  private final class CursorHold: UILongPressGestureRecognizer {
    var simulatedState: UIGestureRecognizer.State = .possible
    var x: CGFloat = 0
    override var state: UIGestureRecognizer.State { get { simulatedState } set { simulatedState = newValue } }
    override func location(in view: UIView?) -> CGPoint { CGPoint(x: x, y: 0) }
  }

  /// 沿用邮箱后缀测试的代理注入方式，结束时恢复 UIKit 实现。
  private func attach(_ document: CursorDocument) throws {
    let selector = #selector(getter: UIInputViewController.textDocumentProxy)
    let inherited = try XCTUnwrap(class_getInstanceMethod(UIInputViewController.self, selector))
    let original = method_getImplementation(inherited)
    let block: @convention(block) (AnyObject) -> AnyObject = { _ in document }
    let replacement = imp_implementationWithBlock(block)
    if !class_addMethod(KeyboardViewController.self, selector, replacement, method_getTypeEncoding(inherited)) {
      method_setImplementation(try XCTUnwrap(class_getInstanceMethod(KeyboardViewController.self, selector)), replacement)
    }
    addTeardownBlock {
      if let own = class_getInstanceMethod(KeyboardViewController.self, selector) { method_setImplementation(own, original) }
      imp_removeBlock(replacement)
    }
  }

  /// 没有完全访问权限时，读屏语音操作说明原因，不打开聆听面板。
  func testVoiceOverOpensVoiceAndExplainsMissingFullAccess() throws {
    let controller = makeController()
    let space = try XCTUnwrap(spaceKeys(in: controller).first)
    XCTAssertEqual(space.accessibilityCustomActions?.map(\.name), ["光标左移", "光标右移", "语音输入"])
    let voice = try XCTUnwrap(space.accessibilityCustomActions?.last)
    XCTAssertFalse(controller.hasFullAccess)
    XCTAssertTrue(voice.actionHandler?(voice) ?? false)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardVoiceListening" })
    let label = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "diagnosticLabel" } as? UILabel)
    XCTAssertEqual(label.text, "语音输入需要开启键盘的“允许完全访问”。")
  }

  /// 点击明确的结束与取消按钮；正文、空白和圆球不会丢弃录音。
  func testTheListeningPanelHasSeparateFinishAndCancelActions() throws {
    let panel = KeyboardVoicePanelView(dictationHeld: false, skin: KeyboardTheme.current)
    var finishes = 0, cancels = 0
    panel.onFinish = { finishes += 1 }
    panel.onCancel = { cancels += 1 }
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 300))
    window.addSubview(panel)
    panel.frame = CGRect(x: 0, y: 0, width: 384, height: 260)
    panel.layoutIfNeeded()

    // 260pt 的面板放得下最大的圆球和两行标题；光晕算进指示区，指示区在面板里面。
    XCTAssertEqual(panel.orb.orbSide, 64)
    XCTAssertEqual(panel.plan?.titleLines, 2)
    XCTAssertEqual(panel.plan?.showsHint, true)
    // 点按开始画人物：指示区高度仍是圆球的 1.5 倍，宽度按 320×250 的视口放宽。
    let indicator = panel.orb.frame
    XCTAssertEqual(indicator.height, 96, accuracy: 0.001)
    XCTAssertEqual(indicator.width, 96 * 320 / 250, accuracy: 0.001)
    XCTAssertEqual(indicator.midX, panel.bounds.midX, accuracy: 0.5)
    XCTAssertTrue(panel.bounds.contains(indicator))
    XCTAssertEqual(panel.statusLabel.text, "正在聆听…")
    XCTAssertEqual(panel.captionLabel.text, "说完后点「说完了」，识别后自动上屏")
    XCTAssertFalse(panel.captionLabel.isHidden)
    XCTAssertGreaterThanOrEqual(panel.captionLabel.frame.minY, panel.statusLabel.frame.maxY)
    XCTAssertFalse(panel.finishButton.isHidden)
    XCTAssertGreaterThanOrEqual(panel.finishButton.bounds.height, 44)
    let finish = panel.finishButton.convert(panel.finishButton.bounds, to: panel)
    let cancel = panel.cancelButton.convert(panel.cancelButton.bounds, to: panel)
    XCTAssertTrue(panel.bounds.contains(finish))
    XCTAssertTrue(panel.bounds.contains(cancel))
    XCTAssertGreaterThanOrEqual(panel.bounds.maxY - cancel.maxY, 10, "取消按钮不贴面板底边")
    XCTAssertGreaterThanOrEqual(finish.minY, panel.captionLabel.frame.maxY)
    XCTAssertFalse(panel.orb.isUserInteractionEnabled)

    // 安静时不逐帧刷新；电平拉满并追上以后，外圈光晕仍在面板里。
    XCTAssertFalse(panel.orb.isAnimatingLevel)
    let reduceMotion = UIAccessibility.isReduceMotionEnabled
    panel.setLevel(1)
    XCTAssertEqual(panel.orb.isAnimatingLevel, !reduceMotion)
    let deadline = Date().addingTimeInterval(3)
    while panel.orb.isAnimatingLevel && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    XCTAssertFalse(panel.orb.isAnimatingLevel, "追上电平后停下")
    XCTAssertEqual(panel.orb.shownLevel, reduceMotion ? 0 : 1)
    let halo = panel.orb.convert(panel.orb.outerHaloFrame, to: panel)
    XCTAssertTrue(panel.bounds.contains(halo), "\(halo) 超出了 \(panel.bounds)")
    if !reduceMotion { XCTAssertEqual(halo.width, indicator.height, accuracy: 0.5) }

    panel.finishButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(finishes, 1)
    XCTAssertEqual(cancels, 0)
    panel.showRecognizing(text: "合成文字", message: "")
    XCTAssertFalse(panel.finishButton.isEnabled)
    XCTAssertEqual(panel.orb.targetLevel, 0, "识别时光晕收回")
    panel.setLevel(0.8)
    XCTAssertEqual(panel.orb.targetLevel, 0, "识别时不再跟电平")
    panel.cancelButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(cancels, 1)
    XCTAssertTrue(panel.accessibilityPerformEscape())
    XCTAssertEqual(cancels, 2)
    panel.removeFromSuperview()
    XCTAssertFalse(panel.orb.isAnimatingLevel)

    let held = KeyboardVoicePanelView(dictationHeld: true, skin: KeyboardTheme.current)
    XCTAssertTrue(held.finishButton.isHidden, "按住空格时由松手结束")
    XCTAssertTrue(held.cancelButton.isHidden, "按住空格时上滑取消，面板上没有按钮")
    var heldCancels = 0
    held.onCancel = { heldCancels += 1 }
    XCTAssertTrue(held.accessibilityPerformEscape(), "读屏的退出手势仍然取消")
    XCTAssertEqual(heldCancels, 1)
  }

  /// #10313：点按开始的会话画水杉娘，按住空格说话保留圆球；按住的面板即使拿到素材也不换。
  func testOnlyTappedSessionsShowTheMascot() {
    XCTAssertTrue(KeyboardVoicePanelPolicy.showsMascot(held: false))
    XCTAssertFalse(KeyboardVoicePanelPolicy.showsMascot(held: true))
    let held = KeyboardVoicePanelView(dictationHeld: true, skin: KeyboardTheme.current)
    XCTAssertFalse(held.orb.showsMascot)
    held.orb.setMascot(Self.syntheticMascot())
    XCTAssertFalse(held.orb.isShowingMascot, "长按空格保留圆球和光晕")
    let tapped = KeyboardVoicePanelView(dictationHeld: false, skin: KeyboardTheme.current)
    XCTAssertTrue(tapped.orb.showsMascot)
    XCTAssertFalse(tapped.orb.isShowingMascot, "素材到之前先画圆球")
    tapped.orb.setMascot(Self.syntheticMascot())
    XCTAssertTrue(tapped.orb.isShowingMascot)
    XCTAssertFalse(tapped.orb.isAccessibilityElement, "人物是装饰，不进读屏")
  }

  /// 红线只跟显示电平：安静时一道也没有（不用圆球的 30% 静息光晕），随电平依次出现，数值与 Android 的 `VoiceMascotDrawing` 一致。
  func testMascotArcsFollowTheLevel() {
    typealias Policy = KeyboardVoicePanelPolicy
    XCTAssertEqual(Policy.mascotArcs(level: 0), [])
    XCTAssertEqual(Policy.mascotArcs(level: 0.005), [], "接近 0 的电平不画线")
    XCTAssertEqual(Policy.mascotArcs(level: .nan), [])
    XCTAssertEqual(Policy.mascotArcs(level: -1), [])
    XCTAssertEqual(Policy.mascotArcs(level: 0.1).count, 1)
    XCTAssertEqual(Policy.mascotArcs(level: 0.29).count, 1)
    XCTAssertEqual(Policy.mascotArcs(level: 0.3).count, 2)
    XCTAssertEqual(Policy.mascotArcs(level: 0.6).count, 3)
    let full = Policy.mascotArcs(level: 1)
    XCTAssertEqual(full.map(\.radius), [45, 60, 75])
    XCTAssertEqual(full.map(\.lineWidth), [5, 5, 5])
    XCTAssertEqual(full.first?.opacity ?? 0, 1, accuracy: 0.0001)
    XCTAssertEqual(Policy.mascotArcs(level: 3), full, "超过 1 的电平按 1 画")
    let quiet = Policy.mascotArcs(level: 0.1)
    XCTAssertEqual(quiet.first?.radius ?? 0, 33 + 1.2, accuracy: 0.0001)
    XCTAssertEqual(quiet.first?.opacity ?? 0, (90 + 16.5) / 255, accuracy: 0.0001)
    XCTAssertEqual(quiet.first?.lineWidth ?? 0, 3.2, accuracy: 0.0001)
  }

  /// 人物指示区的宽度按视口比例放宽但不超过面板，视口等比缩放后居中；素材按最大指示区下采样一次。
  func testMascotGeometryAndAsset() {
    typealias Policy = KeyboardVoicePanelPolicy
    XCTAssertEqual(Policy.mascotWidth(side: 96, available: 384), 122.88, accuracy: 0.0001)
    XCTAssertEqual(Policy.mascotWidth(side: 96, available: 100), 100)
    let fitted = Policy.mascotTransform(in: CGSize(width: 122.88, height: 96))
    XCTAssertEqual(fitted.a, 96 / 250, accuracy: 0.0001)
    XCTAssertEqual(fitted.tx, 0, accuracy: 0.0001)
    XCTAssertEqual(fitted.ty, 0, accuracy: 0.0001)
    let narrow = Policy.mascotTransform(in: CGSize(width: 100, height: 96))
    XCTAssertEqual(narrow.a, 100 / 320, accuracy: 0.0001)
    XCTAssertEqual(narrow.ty, (96 - 250 * 100 / 320) / 2, accuracy: 0.0001)
    XCTAssertEqual(KeyboardVoiceMascot.maximumSide, 96 * 228 / 250, accuracy: 0.0001)
    let directory = URL(fileURLWithPath: "/synthetic/stickers", isDirectory: true)
    let stickers = [KeyboardStickerCatalog.Sticker(url: directory.appendingPathComponent("02.webp"), name: "二"),
                    KeyboardStickerCatalog.Sticker(url: directory.appendingPathComponent("01.webp"), name: "一")]
    XCTAssertEqual(KeyboardVoiceMascot.sticker(in: stickers)?.name, "一", "和 Android 一样用 01.webp")
    XCTAssertNil(KeyboardVoiceMascot.sticker(in: Array(stickers.prefix(1))))
  }

  /// 点按面板拿到素材后画人物：安静时没有红线，满电平三道，进入识别立即收起、之后的电平也不再画。
  func testTheTappedPanelDrawsArcsOnlyWhileListening() throws {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 300))
    let panel = KeyboardVoicePanelView(dictationHeld: false, skin: KeyboardTheme.current)
    window.addSubview(panel)
    panel.frame = CGRect(x: 0, y: 0, width: 384, height: 260)
    panel.orb.setMascot(Self.syntheticMascot())
    panel.layoutIfNeeded()
    XCTAssertTrue(panel.orb.isShowingMascot)
    XCTAssertEqual(panel.orb.visibleMascotArcs, 0, "安静时耳边没有线")
    XCTAssertTrue(panel.orb.bounds.contains(panel.orb.mascotFrame), "\(panel.orb.mascotFrame) 超出了指示区")
    XCTAssertEqual(panel.orb.mascotFrame.height, 96 * 228 / 250, accuracy: 0.5)
    XCTAssertEqual(panel.statusLabel.text, "正在聆听…")
    XCTAssertFalse(panel.finishButton.isHidden)
    XCTAssertFalse(panel.cancelButton.isHidden)

    let reduceMotion = UIAccessibility.isReduceMotionEnabled
    panel.setLevel(1)
    let deadline = Date().addingTimeInterval(3)
    while panel.orb.isAnimatingLevel && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    XCTAssertEqual(panel.orb.visibleMascotArcs, reduceMotion ? 0 : 3, "减弱动态效果时按静音画")

    panel.showRecognizing(text: "", message: "正在润色…")
    XCTAssertEqual(panel.orb.visibleMascotArcs, 0, "识别和润色时立即收起红线")
    panel.setLevel(0.8)
    XCTAssertEqual(panel.orb.visibleMascotArcs, 0, "迟到的电平不让红线回来")
    // 面板在识别时本来就不再转发电平；直接推给指示区，确认是指示区自己在识别阶段按 0 画线。
    panel.orb.setLevel(0.8)
    let recognizingDeadline = Date().addingTimeInterval(3)
    while panel.orb.isAnimatingLevel && Date() < recognizingDeadline {
      RunLoop.main.run(until: Date().addingTimeInterval(0.02))
    }
    if !reduceMotion { XCTAssertEqual(panel.orb.shownLevel, 0.8, accuracy: 0.01) }
    XCTAssertEqual(panel.orb.visibleMascotArcs, 0, "识别阶段指示区按 0 画线")
    panel.removeFromSuperview()
  }

  private static func syntheticMascot() -> UIImage {
    UIGraphicsImageRenderer(size: CGSize(width: 8, height: 8)).image { context in
      UIColor.systemTeal.setFill()
      context.fill(CGRect(x: 0, y: 0, width: 8, height: 8))
    }
  }

  /// 按住说话没有动作行，整个面板都给指示区和文字；点按开始时动作行占掉底部，排版只用它上面的区域。
  func testHoldUsesTheWholePanelAndTapLeavesRoomForTheActions() throws {
    typealias Policy = KeyboardVoicePanelPolicy
    func layout(held: Bool, _ size: CGSize) throws -> (KeyboardVoicePanelView, UIWindow) {
      let window = UIWindow(frame: CGRect(origin: .zero, size: size))
      let panel = KeyboardVoicePanelView(dictationHeld: held, skin: KeyboardTheme.current)
      window.addSubview(panel)
      panel.frame = window.bounds
      panel.layoutIfNeeded()
      return (panel, window)
    }
    func expected(_ panel: KeyboardVoicePanelView, height: CGFloat) throws -> Policy.Plan {
      Policy.plan(height: height, titleLine: try XCTUnwrap(panel.statusLabel.font).lineHeight,
                  hintLine: try XCTUnwrap(panel.captionLabel.font).lineHeight)
    }

    let (hold, holdWindow) = try layout(held: true, CGSize(width: 390, height: 227))
    XCTAssertTrue(hold.finishButton.isHidden)
    XCTAssertTrue(hold.cancelButton.isHidden)
    XCTAssertEqual(hold.plan, try expected(hold, height: 227), "区域就是整个面板高度")
    XCTAssertEqual(hold.orb.orbSide, Policy.orbMax)
    XCTAssertEqual(hold.captionLabel.text, "松开结束，上滑取消")
    XCTAssertFalse(hold.captionLabel.isHidden)
    XCTAssertLessThanOrEqual(hold.captionLabel.frame.maxY, 227 - Policy.padBottom + 0.5)
    hold.setLevel(1)
    let deadline = Date().addingTimeInterval(3)
    while hold.orb.isAnimatingLevel && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    let halo = hold.orb.convert(hold.orb.outerHaloFrame, to: hold)
    XCTAssertTrue(hold.bounds.contains(halo), "\(halo) 超出了 \(hold.bounds)")
    holdWindow.isHidden = true

    // 横屏按住说话：去掉动作行以后提示行放得下。
    let (landscape, _) = try layout(held: true, CGSize(width: 844, height: 161))
    landscape.showListening(String(repeating: "合成的实时文字", count: 12))
    landscape.layoutIfNeeded()
    let landscapePlan = try XCTUnwrap(landscape.plan)
    XCTAssertEqual(landscapePlan, try expected(landscape, height: 161))
    XCTAssertTrue(landscapePlan.showsHint)
    XCTAssertFalse(landscape.captionLabel.isHidden)
    XCTAssertEqual(landscape.captionLabel.text, "松开结束，上滑取消")
    XCTAssertGreaterThanOrEqual(landscapePlan.orb, Policy.twoLineMinOrb)
    XCTAssertLessThanOrEqual(landscape.captionLabel.frame.maxY, 161 - Policy.padBottom + 0.5)

    let (tap, _) = try layout(held: false, CGSize(width: 390, height: 227))
    let cancel = tap.cancelButton.convert(tap.cancelButton.bounds, to: tap)
    XCTAssertFalse(tap.cancelButton.isHidden)
    XCTAssertFalse(tap.finishButton.isHidden)
    XCTAssertEqual(cancel.maxY, 227 - KeyboardVoicePanelView.actionsBottomMargin, accuracy: 0.5)
    XCTAssertEqual(tap.plan, try expected(tap, height: 227 - KeyboardVoicePanelView.actionsBottomMargin - cancel.height))
    XCTAssertEqual(tap.orb.orbSide, Policy.orbMax)
  }

  /// 竖屏、横屏、缩小的键盘和系统大字号下，按住和点按两种面板的指示区、标题、提示和动作行都在面板里、互不重叠。
  func testThePanelKeepsEverythingInsideAtEveryHeight() {
    let cases: [(CGSize, UIContentSizeCategory)] = [
      (CGSize(width: 390, height: 227), .large), (CGSize(width: 844, height: 161), .large),
      (CGSize(width: 390, height: 150), .large), (CGSize(width: 390, height: 227), .accessibilityExtraExtraExtraLarge),
      (CGSize(width: 844, height: 161), .accessibilityExtraExtraExtraLarge),
    ]
    for held in [true, false] {
      for (size, category) in cases {
        let name = "\(held ? "hold" : "tap") \(size) \(category.rawValue)"
        let window = UIWindow(frame: CGRect(origin: .zero, size: size))
        window.traitOverrides.preferredContentSizeCategory = category
        let panel = KeyboardVoicePanelView(dictationHeld: held, skin: KeyboardTheme.current)
        window.addSubview(panel)
        panel.frame = window.bounds
        panel.showListening(String(repeating: "合成的实时文字", count: 30))
        panel.layoutIfNeeded()
        if category.isAccessibilityCategory {
          XCTAssertGreaterThan(panel.statusLabel.font.pointSize, KeyboardVoicePanelView.titleSize, "跟随系统字号 \(name)")
        }
        let plan = panel.plan
        let indicator = panel.orb.frame
        XCTAssertGreaterThanOrEqual(indicator.minY, 0, name)
        XCTAssertLessThanOrEqual(indicator.maxY, panel.statusLabel.frame.minY, name)
        let textBottom = panel.captionLabel.isHidden ? panel.statusLabel.frame.maxY : panel.captionLabel.frame.maxY
        if held {
          XCTAssertLessThanOrEqual(textBottom, size.height - KeyboardVoicePanelPolicy.padBottom + 0.5, name)
        } else {
          let cancel = panel.cancelButton.convert(panel.cancelButton.bounds, to: panel)
          XCTAssertLessThanOrEqual(textBottom, cancel.minY + 0.5, name)
          XCTAssertLessThanOrEqual(cancel.maxY, size.height - 10 + 0.5, name)
        }
        XCTAssertEqual(panel.captionLabel.isHidden, plan?.showsHint == false, name)
        XCTAssertTrue(panel.statusLabel.text?.hasPrefix("…") ?? false, "放不下时省略开头 \(name)")
        XCTAssertTrue(panel.statusLabel.text?.hasSuffix("实时文字") ?? false, "保留最新的文字 \(name)")
      }
    }
  }

  /// 实时文字截到计划的两行，标签实际折出的行数也不超过两行，留下的是最新的那一段。
  func testALongPartialKeepsTheNewestWordsWithinTheTitleLines() throws {
    let panel = KeyboardVoicePanelView(dictationHeld: false, skin: KeyboardTheme.current)
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    panel.layoutIfNeeded()
    let partial = "第一句合成文字在这里开始，" + String(repeating: "中间的合成内容，", count: 10) + "最后这几个字要留下"
    panel.showListening(partial)
    panel.layoutIfNeeded()
    let title = try XCTUnwrap(panel.statusLabel.text)
    XCTAssertTrue(title.hasPrefix("…"))
    XCTAssertTrue(title.hasSuffix("最后这几个字要留下"))
    XCTAssertFalse(title.contains("第一句"))
    XCTAssertEqual(panel.statusLabel.numberOfLines, 2)
    let font = try XCTUnwrap(panel.statusLabel.font)
    let wrapped = panel.statusLabel.textRect(forBounds: CGRect(x: 0, y: 0, width: panel.statusLabel.bounds.width, height: 1000),
                                             limitedToNumberOfLines: 0)
    XCTAssertLessThanOrEqual(wrapped.height, 2 * font.lineHeight + 0.5, "标签自己折行也只用两行")
    XCTAssertEqual(panel.statusLabel.frame.height, ceil(2 * font.lineHeight), accuracy: 0.5)
    XCTAssertEqual(panel.captionLabel.text, "说完后点「说完了」，识别后自动上屏", "提示仍是操作说明")

    // 识别中实时文字移到提示行，截成一行。
    panel.showRecognizing(text: partial, message: "")
    panel.layoutIfNeeded()
    XCTAssertEqual(panel.statusLabel.text, "正在识别…")
    let hint = try XCTUnwrap(panel.captionLabel.text)
    XCTAssertTrue(hint.hasPrefix("…"))
    XCTAssertTrue(hint.hasSuffix("最后这几个字要留下"))
    XCTAssertEqual(KeyboardVoicePanelView.lineCount(hint, font: try XCTUnwrap(panel.captionLabel.font),
                                                    width: panel.captionLabel.bounds.width), 1)
  }

  func testTheLayoutPlanShrinksTheOrbBeforeDroppingLines() {
    typealias Policy = KeyboardVoicePanelPolicy
    // 用整数行高，便于手算：两行加提示的固定部分是 78，一行加提示 58，只有一行 38。
    func plan(_ height: CGFloat) -> Policy.Plan { Policy.plan(height: height, titleLine: 20, hintLine: 16) }
    let full = plan(260)
    XCTAssertEqual(full.orb, 64)
    XCTAssertEqual(full.halo, 16)
    XCTAssertEqual(full.titleLines, 2)
    XCTAssertTrue(full.showsHint)
    XCTAssertEqual(full.top, 43)
    XCTAssertEqual(full.indicatorCenterY, 95)
    XCTAssertEqual(full.titleTop, 149)
    XCTAssertEqual(full.hintTop(usedTitleLines: 2), 193)
    XCTAssertEqual(full.hintTop(usedTitleLines: 1), 173, "标题只用一行时提示紧跟着它")
    XCTAssertEqual(full.hintTop(usedTitleLines: 5), 193)

    XCTAssertEqual(plan(156).titleLines, 2, "圆球还有 52 就留两行")
    XCTAssertEqual(plan(156).orb, 52, accuracy: 0.001)
    XCTAssertEqual(plan(155).titleLines, 1)
    XCTAssertTrue(plan(155).showsHint)
    XCTAssertEqual(plan(118).orb, 40, accuracy: 0.001)
    XCTAssertTrue(plan(118).showsHint)
    XCTAssertFalse(plan(117).showsHint, "再矮就去掉提示")
    XCTAssertEqual(plan(117).orb, 79 / 1.5, accuracy: 0.001)
    // 只有一行也放不下 40 的圆球加光晕：圆球固定 40，光晕用剩下的空间。
    XCTAssertEqual(plan(90).orb, 40)
    XCTAssertEqual(plan(90).halo, 6)
    XCTAssertEqual(plan(80).halo, 1)
    XCTAssertEqual(plan(60).orb, 22)
    XCTAssertEqual(plan(60).halo, 0)
    XCTAssertEqual(plan(20).orb, 0)

    for tenth in 0...4000 {
      let height = CGFloat(tenth) / 10
      let result = plan(height)
      let fixed = Policy.fixed(lines: result.titleLines, hint: result.showsHint, titleLine: 20, hintLine: 16)
      XCTAssertLessThanOrEqual(result.indicatorSide, max(0, height - fixed) + 0.001, "height \(height)")
      XCTAssertLessThanOrEqual(result.orb, Policy.orbMax)
      XCTAssertGreaterThanOrEqual(result.top, 0)
      if height >= fixed {
        XCTAssertLessThanOrEqual(result.top + Policy.padTop + result.indicatorSide, height, "height \(height)")
      }
    }
  }

  func testTheLevelRisesFastFallsSlowlyAndSettles() {
    typealias Policy = KeyboardVoicePanelPolicy
    let rise = Policy.step(shown: 0, target: 1, elapsedMs: 16)
    let fall = 1 - Policy.step(shown: 1, target: 0, elapsedMs: 16)
    XCTAssertEqual(rise, 1 - Float(exp(-16.0 / 60)), accuracy: 0.0001)
    XCTAssertEqual(fall, 1 - Float(exp(-16.0 / 320)), accuracy: 0.0001)
    XCTAssertGreaterThan(rise, 3 * fall, "上升比回落快")

    XCTAssertEqual(Policy.step(shown: 0, target: .nan, elapsedMs: 16), 0, "NaN 当作 0")
    XCTAssertLessThan(Policy.step(shown: 0.5, target: .nan, elapsedMs: 16), 0.5)
    XCTAssertEqual(Policy.step(shown: 0, target: 5, elapsedMs: 50), Policy.step(shown: 0, target: 1, elapsedMs: 50))
    XCTAssertEqual(Policy.step(shown: 0.5, target: -3, elapsedMs: 50), Policy.step(shown: 0.5, target: 0, elapsedMs: 50))
    XCTAssertEqual(Policy.step(shown: 0, target: 1, elapsedMs: 5000), Policy.step(shown: 0, target: 1, elapsedMs: 100),
                   "一帧最多推进 100ms")
    XCTAssertEqual(Policy.step(shown: 0.5, target: 1, elapsedMs: -40), 0.5)
    XCTAssertEqual(Policy.step(shown: 0.5, target: 1, elapsedMs: .nan), 0.5)
    XCTAssertEqual(Policy.step(shown: 0.995, target: 1, elapsedMs: 1), 1, "差不到 0.01 直接落到目标")

    XCTAssertTrue(Policy.settled(shown: 0.995, target: 1))
    XCTAssertFalse(Policy.settled(shown: 0.9, target: 1))
    XCTAssertTrue(Policy.settled(shown: 1, target: 7))
    XCTAssertTrue(Policy.settled(shown: 0, target: .nan))

    var shown: Float = 0, frames = 0
    while shown != 1 && frames < 1000 { shown = Policy.step(shown: shown, target: 1, elapsedMs: 16); frames += 1 }
    XCTAssertEqual(shown, 1)
    XCTAssertLessThan(frames, 25)
    var released = 0
    while shown != 0 && released < 1000 { shown = Policy.step(shown: shown, target: 0, elapsedMs: 16); released += 1 }
    XCTAssertEqual(shown, 0)
    XCTAssertGreaterThan(released, 3 * frames)

    let idle = Policy.haloRadii(orb: 72, halo: 18, level: 0)
    XCTAssertEqual(idle.outer, 36 + 18 * 0.3, accuracy: 0.001)
    XCTAssertEqual(idle.inner, 36 + 9 * 0.3, accuracy: 0.001)
    let loud = Policy.haloRadii(orb: 72, halo: 18, level: 1)
    XCTAssertEqual(loud.outer, 54, accuracy: 0.001, "最大时正好铺满指示区")
    XCTAssertEqual(loud.inner, 45, accuracy: 0.001)
    XCTAssertEqual(Policy.haloRadii(orb: 72, halo: 18, level: 3).outer, 54, accuracy: 0.001)
  }

  func testFitTailKeepsTheNewestCharactersOnGraphemeBoundaries() {
    typealias Policy = KeyboardVoicePanelPolicy
    XCTAssertEqual(Policy.fitTail("  你好\n\n 世界\t ") { _ in true }, "你好 世界", "空白压成一个空格")
    XCTAssertEqual(Policy.fitTail(" \n\t\u{3000} ") { _ in true }, "")
    XCTAssertEqual(Policy.fitTail("今天天气很好我们去公园") { $0.count <= 6 }, "…我们去公园")
    XCTAssertEqual(Policy.fitTail("你好") { _ in false }, "…好", "一个字也放不下时只留最后一个字")
    XCTAssertEqual(Policy.fitTail("好") { _ in false }, "…好")

    let emoji = "开始👨‍👩‍👧‍👦结尾🇯🇵"
    XCTAssertEqual(Policy.fitTail(emoji) { $0.count <= 4 }, "…结尾🇯🇵")
    // 按 UTF-16 长度判断也不会切开组合表情。
    for limit in 1...20 {
      let fitted = Policy.fitTail(emoji) { $0.utf16.count <= limit }
      if fitted == emoji { continue }
      XCTAssertTrue(fitted.hasPrefix("…"))
      let tail = Array(fitted.dropFirst())
      XCTAssertFalse(tail.isEmpty)
      XCTAssertEqual(Array(Array(emoji).suffix(tail.count)), tail, "limit \(limit)")
    }
  }

  /// 从功能菜单的真实「语音输入」图块进入听写（iOS 工具栏不放语音按钮），结束后经真实会话通知自动上屏，不经过结果面板。
  func testMenuTileStartsDictationAndFinalTextEntersTheOriginalEditor() throws {
    let selector = #selector(getter: UIInputViewController.hasFullAccess)
    let method = try XCTUnwrap(class_getInstanceMethod(UIInputViewController.self, selector))
    let original = method_getImplementation(method)
    let block: @convention(block) (AnyObject) -> Bool = { _ in true }
    let replacement = imp_implementationWithBlock(block)
    if !class_addMethod(KeyboardViewController.self, selector, replacement, method_getTypeEncoding(method)) {
      method_setImplementation(try XCTUnwrap(class_getInstanceMethod(KeyboardViewController.self, selector)), replacement)
    }
    defer {
      if let own = class_getInstanceMethod(KeyboardViewController.self, selector) { method_setImplementation(own, original) }
      imp_removeBlock(replacement)
    }
    // 无签名的 CI 测试进程没有 App Group 容器；只在本用例内把目录解析到隔离的合成数据目录。
    let testDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: testDirectory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: testDirectory) }
    let containerSelector = #selector(FileManager.containerURL(forSecurityApplicationGroupIdentifier:))
    let containerMethod = try XCTUnwrap(class_getInstanceMethod(FileManager.self, containerSelector))
    let containerOriginal = method_getImplementation(containerMethod)
    typealias ContainerLookup = @convention(c) (AnyObject, Selector, NSString) -> NSURL?
    let containerLookup = unsafeBitCast(containerOriginal, to: ContainerLookup.self)
    let groupIdentifier = MSIMEAppEdition.appGroupIdentifier
    let containerBlock: @convention(block) (AnyObject, NSString) -> NSURL? = { manager, identifier in
      if identifier as String == groupIdentifier { return testDirectory as NSURL }
      return containerLookup(manager, containerSelector, identifier)
    }
    let containerReplacement = imp_implementationWithBlock(containerBlock)
    method_setImplementation(containerMethod, containerReplacement)
    defer {
      method_setImplementation(containerMethod, containerOriginal)
      imp_removeBlock(containerReplacement)
    }
    let directory = try XCTUnwrap(VoiceTextHandoffStore.defaultDirectory).appendingPathComponent("VoiceSession")
    let saved = ["state.json", "command.json"].map { (directory.appendingPathComponent($0), try? Data(contentsOf: directory.appendingPathComponent($0))) }
    defer {
      for (file, data) in saved {
        if let data { try? data.write(to: file, options: .atomic) }
        else { try? FileManager.default.removeItem(at: file) }
      }
    }
    let channel = VoiceSessionChannel()
    let now = Date()
    channel.publish(VoiceSessionState(phase: .ready, request: nil, heartbeat: now, expiresAt: now.addingTimeInterval(300)))
    let document = CursorDocument()
    try attach(document)
    let controller = makeController()
    let more = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "moreShortcut" } as? UIControl)
    more.sendActions(for: .primaryActionTriggered)
    let tile = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "moreCard-语音输入" } as? UIControl)
    tile.sendActions(for: .primaryActionTriggered)
    let command = try XCTUnwrap(channel.readCommand())
    XCTAssertEqual(command.action, .start, "点语音入口应直接开始听写")
    let panel = try XCTUnwrap(descendants(controller.view).compactMap { $0 as? KeyboardVoicePanelView }.first)
    XCTAssertTrue(panel.orb.showsMascot, "功能菜单的语音入口是点按开始，指示区画水杉娘")
    channel.publish(VoiceSessionState(phase: .listening, request: command.request, text: "合成",
      heartbeat: now, expiresAt: now.addingTimeInterval(300)))
    RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    panel.finishButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(channel.readCommand()?.action, .stop)
    XCTAssertFalse(panel.finishButton.isEnabled)
    XCTAssertEqual(document.insertions, [], "结束采集时仍等待最终转写")
    channel.publish(VoiceSessionState(phase: .finished, request: command.request, text: "合成语音文字",
      heartbeat: now, expiresAt: now.addingTimeInterval(300)))
    RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    XCTAssertEqual(document.insertions, ["合成语音文字"])
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardVoicePanelView })
  }

  private func makeController() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func spaceKeys(in controller: KeyboardViewController) -> [UIButton] {
    descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { $0.gestureRecognizers?.contains { $0.name == "spaceCursorPan" } == true }
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }
}

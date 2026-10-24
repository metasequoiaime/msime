import ObjectiveC
import UIKit
import XCTest

/// 空格长按与语音结果面板：长按按住说话（会话没开时转交主 App 开启），面板仍负责待插入结果；光标拖动裁决方式同 Android 的 `SpaceGesturePolicy`。
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

  /// 「长按空格语音输入」关掉后，长按改为移动光标，VoiceOver 提示和操作里都没有语音。
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
    let hold = try XCTUnwrap(space.gestureRecognizers?.first { $0.name == "spaceVoiceHold" })
    XCTAssertTrue(controller.gestureRecognizerShouldBegin(hold))
    KeyboardLayoutPreference.spaceVoice = true
    controller.viewWillAppear(false)
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

  /// VoiceOver 通过空格键的 语音输入 操作进入面板。测试宿主没有完全访问权限，结果永远无法从 App 传回：键盘会直接提示这一点，而不是打开面板。
  func testVoiceOverOpensVoiceAndExplainsMissingFullAccess() throws {
    let controller = makeController()
    let space = try XCTUnwrap(spaceKeys(in: controller).first)
    XCTAssertEqual(space.accessibilityCustomActions?.map(\.name), ["光标左移", "光标右移", "语音输入"])
    let voice = try XCTUnwrap(space.accessibilityCustomActions?.last)
    XCTAssertFalse(controller.hasFullAccess)
    XCTAssertTrue(voice.actionHandler?(voice) ?? false)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardVoicePanel" })
    let label = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "diagnosticLabel" } as? UILabel)
    XCTAssertEqual(label.text, "语音输入需要开启键盘的“允许完全访问”。")
  }

  func testThePanelSaysWhatTheOrbDoes() {
    XCTAssertEqual(KeyboardVoicePanelView.status(for: nil), "轻点开启语音")
    XCTAssertEqual(KeyboardVoicePanelView.orbLabel(for: nil), "去水杉 App 开启语音")
    let entry = VoiceTextHandoff(id: UUID(), text: "明天见", createdAt: Date(), expiresAt: Date().addingTimeInterval(600))
    XCTAssertEqual(KeyboardVoicePanelView.status(for: entry), "已识别：明天见")
    XCTAssertEqual(KeyboardVoicePanelView.orbLabel(for: entry), "插入语音结果")
  }

  /// 设计稿的面板：72pt 圆球在上，下面是 15pt 状态文字和 12.5pt 说明文字，间隔 14pt；点圆球执行操作，点其他任何地方取消。
  func testThePanelLayoutAndTaps() throws {
    let panel = KeyboardVoicePanelView(entry: nil, skin: KeyboardTheme.current)
    var orbTaps = 0, cancels = 0
    panel.onOrb = { orbTaps += 1 }
    panel.onCancel = { cancels += 1 }
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 300))
    window.addSubview(panel)
    panel.frame = CGRect(x: 0, y: 0, width: 384, height: 189)
    panel.layoutIfNeeded()

    let orb = panel.orb.convert(panel.orb.bounds, to: panel)
    XCTAssertEqual(orb.size, CGSize(width: 72, height: 72))
    XCTAssertEqual(orb.midX, panel.bounds.midX, accuracy: 0.5)
    let status = panel.statusLabel.convert(panel.statusLabel.bounds, to: panel)
    let caption = panel.captionLabel.convert(panel.captionLabel.bounds, to: panel)
    XCTAssertEqual(status.minY - orb.maxY, 14, accuracy: 0.5)
    XCTAssertEqual(caption.minY - status.maxY, 14, accuracy: 0.5)
    XCTAssertEqual(panel.statusLabel.text, "轻点开启语音")
    XCTAssertEqual(panel.statusLabel.font.pointSize, 15)
    XCTAssertEqual(panel.captionLabel.text, "点任意处取消")
    XCTAssertEqual(panel.captionLabel.font.pointSize, 12.5)
    XCTAssertEqual(panel.orb.accessibilityLabel, "去水杉 App 开启语音")
    XCTAssertEqual(panel.orb.isPulsing, !UIAccessibility.isReduceMotionEnabled)

    XCTAssertTrue(panel.hitTest(CGPoint(x: orb.midX, y: orb.midY), with: nil) === panel.orb)
    XCTAssertTrue(panel.hitTest(CGPoint(x: caption.midX, y: caption.midY), with: nil) === panel)
    XCTAssertTrue(panel.hitTest(CGPoint(x: 10, y: 10), with: nil) === panel)
    panel.orb.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(orbTaps, 1)
    XCTAssertEqual(cancels, 0)
    panel.sendActions(for: .touchUpInside)
    XCTAssertEqual(cancels, 1)
    XCTAssertTrue(panel.accessibilityPerformEscape())
    XCTAssertEqual(cancels, 2)
    panel.removeFromSuperview()
    XCTAssertFalse(panel.orb.isPulsing, "the ring stops off screen")
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

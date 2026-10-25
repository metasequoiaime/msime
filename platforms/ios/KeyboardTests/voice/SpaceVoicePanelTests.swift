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

    let orb = panel.orb.convert(panel.orb.bounds, to: panel)
    XCTAssertEqual(orb.size, CGSize(width: 72, height: 72))
    XCTAssertEqual(orb.midX, panel.bounds.midX, accuracy: 0.5)
    XCTAssertEqual(panel.statusLabel.text, "正在聆听…")
    XCTAssertEqual(panel.captionLabel.text, "说完后点「说完了」，识别后自动上屏")
    XCTAssertFalse(panel.finishButton.isHidden)
    XCTAssertGreaterThanOrEqual(panel.finishButton.bounds.height, 44)
    XCTAssertTrue(panel.bounds.contains(panel.finishButton.convert(panel.finishButton.bounds, to: panel)))
    XCTAssertTrue(panel.bounds.contains(panel.cancelButton.convert(panel.cancelButton.bounds, to: panel)))
    XCTAssertFalse(panel.orb.isUserInteractionEnabled)
    XCTAssertEqual(panel.orb.isPulsing, !UIAccessibility.isReduceMotionEnabled)
    panel.finishButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(finishes, 1)
    XCTAssertEqual(cancels, 0)
    panel.showRecognizing(text: "合成文字", message: "")
    XCTAssertFalse(panel.finishButton.isEnabled)
    panel.cancelButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(cancels, 1)
    XCTAssertTrue(panel.accessibilityPerformEscape())
    XCTAssertEqual(cancels, 2)
    panel.removeFromSuperview()
    XCTAssertFalse(panel.orb.isPulsing)

    let held = KeyboardVoicePanelView(dictationHeld: true, skin: KeyboardTheme.current)
    XCTAssertTrue(held.finishButton.isHidden, "按住空格时由松手结束")
    XCTAssertFalse(held.cancelButton.isHidden)
  }

  /// 从工具栏的真实按钮进入听写，结束后经真实会话通知自动上屏，不经过结果面板。
  func testToolbarStartsDictationAndFinalTextEntersTheOriginalEditor() throws {
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
    let toolbar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "layoutVoiceShortcut" } as? UIButton)
    XCTAssertEqual(toolbar.accessibilityLabel, "语音输入")
    toolbar.sendActions(for: .primaryActionTriggered)
    let command = try XCTUnwrap(channel.readCommand())
    XCTAssertEqual(command.action, .start, "点语音入口应直接开始听写")
    let panel = try XCTUnwrap(descendants(controller.view).compactMap { $0 as? KeyboardVoicePanelView }.first)
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

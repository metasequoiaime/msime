import ObjectiveC
import UIKit
import XCTest

/// 整块键盘横滑的入口与裁决；文档步进沿用空格手势的生产逻辑。
@MainActor
final class KeyboardCursorSwipeTests: XCTestCase {
  private let preferenceKey = "keyboard.gesture.cursorSwipe"
  private var saved: [String: Any] = [:]
  private var previousScheme = InputSchemePreference.scheme
  private var preferenceKeys: [String] {
    [preferenceKey, KeyboardLayoutPreference.glideTypingKey,
     ImeModeMemoryPreference.enabledKey, ImeModeMemoryPreference.lastChineseKey]
  }

  override func setUp() {
    super.setUp()
    previousScheme = InputSchemePreference.scheme
    enableAllInputSchemes()
    saved.removeAll()
    for key in preferenceKeys {
      if let value = KeyboardLayoutPreference.defaults.object(forKey: key) { saved[key] = value }
      KeyboardLayoutPreference.defaults.removeObject(forKey: key)
    }
    ImeModeMemoryPreference.defaults.set(true, forKey: ImeModeMemoryPreference.enabledKey)
    ImeModeMemoryPreference.defaults.set(true, forKey: ImeModeMemoryPreference.lastChineseKey)
    InputSchemePreference.scheme = .quanpin
  }

  override func tearDown() {
    for key in preferenceKeys {
      if let value = saved[key] { KeyboardLayoutPreference.defaults.set(value, forKey: key) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: key) }
    }
    InputSchemePreference.scheme = previousScheme
    super.tearDown()
  }

  func testCursorSwipeStaysOffWhenThePreferenceIsAbsent() {
    let controller = keyboard()
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
  }

  func testCursorSwipeStaysOffWhenThePreferenceIsExplicitlyFalse() {
    KeyboardLayoutPreference.defaults.set(false, forKey: preferenceKey)
    let controller = keyboard()
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
  }

  func testTheInstalledPanPreservesTapTimingAndCancelsTheStartingKey() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    let controller = keyboard()
    let root = try XCTUnwrap(nodes(controller.view).first { $0 is KeyAreaStackView })
    let pan = try XCTUnwrap(root.gestureRecognizers?.first { $0.name == "keyboardCursorPan" } as? UIPanGestureRecognizer)
    XCTAssertTrue(pan.cancelsTouchesInView)
    XCTAssertFalse(pan.delaysTouchesBegan)
    XCTAssertFalse(pan.delaysTouchesEnded)
    XCTAssertEqual(pan.maximumNumberOfTouches, 1)
    XCTAssertTrue(pan.delegate === controller)
  }

  func testOnlyHorizontalMovementCanStartWithTheSettingEnabled() {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    let controller = keyboard()
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertTrue(controller.gestureRecognizerShouldBegin(pan))
    pan.direction = CGPoint(x: -300, y: 10)
    XCTAssertTrue(controller.gestureRecognizerShouldBegin(pan))
    pan.direction = CGPoint(x: 10, y: 300)
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
    pan.direction = CGPoint(x: 100, y: 100)
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
    pan.direction = CGPoint(x: CGFloat.infinity, y: 0)
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
    pan.direction = CGPoint(x: CGFloat.nan, y: 0)
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
    pan.direction = .zero
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
  }

  func testGlideTypingKeepsPriorityOnTheQuanpinLetterKeyboard() {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    KeyboardLayoutPreference.glideTyping = true
    let controller = keyboard()
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
  }

  func testJapaneseNineKeyFlickKeepsItsHorizontalGestures() {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    InputSchemePreference.scheme = .japaneseNineKey
    let controller = keyboard()
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
  }

  func testJapaneseDigitFlickAndItsFunctionKeysKeepTheirGestures() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    InputSchemePreference.scheme = .japaneseNineKey
    let controller = keyboard()
    let toggle = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "layoutToggleButton" } as? UIButton)
    toggle.sendActions(for: .primaryActionTriggered)
    let kana = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "japaneseNineKey" })
    let one = try XCTUnwrap(nodes(kana).first { $0.accessibilityIdentifier == "japaneseKana0" })
    XCTAssertTrue(one.accessibilityLabel?.contains("☆") == true)
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
    for key in nodes(kana).compactMap({ $0 as? KeyboardKeyButton }) {
      XCTAssertFalse(receivesSwipe(on: key, controller: controller), key.accessibilityLabel ?? "假名键")
    }
  }

  func testGlidePreferenceDoesNotDisableCursorSwipingOnTheSymbolLayer() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    KeyboardLayoutPreference.glideTyping = true
    let controller = keyboard()
    let toggle = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "layoutToggleButton" } as? UIButton)
    toggle.sendActions(for: .primaryActionTriggered)
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertTrue(controller.gestureRecognizerShouldBegin(pan))
  }

  func testNineKeySymbolShortcutIsExcludedButTheDigitLayerCanSwipe() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    InputSchemePreference.scheme = .nineKey
    let controller = keyboard()
    let one = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "nineKey1" })
    XCTAssertEqual(one.accessibilityLabel, "1 符号")
    XCTAssertFalse(receivesSwipe(on: one, controller: controller))
    let toggle = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "layoutToggleButton" } as? UIButton)
    toggle.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(one.accessibilityLabel, "数字 1")
    XCTAssertTrue(receivesSwipe(on: one, controller: controller))
  }

  func testAnExistingCompositionCannotStartDocumentCursorMovement() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    let controller = keyboard()
    let key = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    key.sendActions(for: .primaryActionTriggered)
    let preedit = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)
    XCTAssertEqual(preedit.accessibilityValue?.filter(\.isLetter), "n")
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
  }

  func testOnlyCharacterKeysAndTheirSubviewsCanStartTheSwipe() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    let controller = keyboard()
    let letter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 N" } as? KeyboardKeyButton)
    XCTAssertTrue(receivesSwipe(on: letter, controller: controller))
    let label = UILabel()
    letter.addSubview(label)
    XCTAssertTrue(receivesSwipe(on: label, controller: controller))
    for key in nodes(controller.view).compactMap({ $0 as? KeyboardKeyButton }) where key.isFunctionKey {
      XCTAssertFalse(receivesSwipe(on: key, controller: controller), key.accessibilityLabel ?? "功能键")
    }
    let spaces = nodes(controller.view).filter { $0.accessibilityLabel == "空格" }
    XCTAssertFalse(spaces.isEmpty)
    for space in spaces { XCTAssertFalse(receivesSwipe(on: space, controller: controller)) }
    XCTAssertFalse(receivesSwipe(on: controller.view, controller: controller))
    XCTAssertFalse(receivesSwipe(on: UIButton(), controller: controller))
  }

  func testACharacterKeyOutsideTheKeyboardOrInsideAnExcludedPanelIsRejected() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    let controller = keyboard()
    let detached = KeyboardKeyButton(configuration: .plain())
    XCTAssertFalse(receivesSwipe(on: detached, controller: controller))
    let root = try XCTUnwrap(nodes(controller.view).first { $0 is KeyAreaStackView } as? KeyAreaStackView)
    let panel = try XCTUnwrap(root.gapRoutingExclusions.first)
    panel.addSubview(detached)
    XCTAssertFalse(receivesSwipe(on: detached, controller: controller))
  }

  func testTheDisabledSettingRejectsTouchesBeforeRecognition() throws {
    let controller = keyboard()
    let letter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 N" } as? KeyboardKeyButton)
    XCTAssertFalse(receivesSwipe(on: letter, controller: controller))
  }

  func testADragWithoutADocumentNeverSuppressesKeys() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    let controller = keyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    try send(.began, at: 0, pan: pan, to: controller)
    XCTAssertFalse(area.suppressesKeyHits)
    try send(.changed, at: 48, pan: pan, to: controller)
    XCTAssertFalse(area.suppressesKeyHits)
  }

  func testOverlappingCharacterAndFunctionTouchesCannotStartCursorSwiping() throws {
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    let controller = keyboard()
    let letter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 N" })
    let functionKeys = nodes(controller.view).compactMap { $0 as? KeyboardKeyButton }
      .filter { $0.isFunctionKey || $0.accessibilityLabel == "空格" }
    XCTAssertFalse(functionKeys.isEmpty)
    for key in functionKeys {
      for views in [[letter, key], [key, letter]] {
        let touches = views.map { view in
          let touch = StartingTouch()
          touch.startingView = view
          return touch
        }
        let event = TouchEvent()
        let pan = DirectionalPan()
        pan.name = "keyboardCursorPan"
        event.touches = [touches[0]]
        XCTAssertTrue(receivesEvent(event, pan: pan, controller: controller))
        event.touches.insert(touches[1])
        XCTAssertFalse(receivesEvent(event, pan: pan, controller: controller), key.accessibilityLabel ?? "功能键")
        XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
        touches[0].phaseValue = .moved
        touches[1].phaseValue = .ended
        XCTAssertTrue(receivesEvent(event, pan: pan, controller: controller))
        XCTAssertFalse(controller.gestureRecognizerShouldBegin(pan))
        touches[0].phaseValue = .ended
        let next = StartingTouch()
        next.startingView = letter
        event.touches = [touches[0], touches[1], next]
        XCTAssertTrue(receivesEvent(event, pan: pan, controller: controller))
        XCTAssertTrue(controller.gestureRecognizerShouldBegin(pan))
      }
    }
  }

  func testASecondTouchEndsAnActiveCursorDragEvenOnAnExcludedKey() throws {
    let (controller, document) = try documentKeyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    try send(.began, at: 0, pan: pan, to: controller)
    XCTAssertTrue(area.suppressesKeyHits)
    let event = TouchEvent()
    event.touches = [StartingTouch(), StartingTouch()]
    XCTAssertFalse(receivesEvent(event, pan: pan, controller: controller))
    XCTAssertFalse(area.suppressesKeyHits)
    try send(.changed, at: 24, pan: pan, to: controller)
    XCTAssertTrue(document.offsets.isEmpty)
  }

  func testDocumentDraggingAccumulatesAndReversesCharacterOffsets() throws {
    let (controller, document) = try documentKeyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    try send(.began, at: 0, pan: pan, to: controller)
    XCTAssertTrue(area.suppressesKeyHits)
    try send(.changed, at: 24, pan: pan, to: controller)
    try send(.changed, at: 18, pan: pan, to: controller)
    try send(.changed, at: 0, pan: pan, to: controller)
    XCTAssertEqual(document.offsets, [2, -2])
    XCTAssertTrue(document.insertions.isEmpty)
    try send(.ended, at: 0, pan: pan, to: controller)
    XCTAssertFalse(area.suppressesKeyHits)
  }

  func testEveryTerminalStateRestoresHitsAndIgnoresStaleChanges() throws {
    let (controller, document) = try documentKeyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    for state in [UIGestureRecognizer.State.ended, .cancelled, .failed] {
      try send(.began, at: 0, pan: pan, to: controller)
      XCTAssertTrue(area.suppressesKeyHits)
      try send(state, at: 0, pan: pan, to: controller)
      XCTAssertFalse(area.suppressesKeyHits)
      try send(.changed, at: 24, pan: pan, to: controller)
      XCTAssertTrue(document.offsets.isEmpty)
    }
  }

  func testAChangedDocumentImmediatelyRestoresHitsAndStopsTheOldDrag() throws {
    let (controller, document) = try documentKeyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    try send(.began, at: 0, pan: pan, to: controller)
    XCTAssertTrue(area.suppressesKeyHits)
    document.documentIdentifier = UUID()
    controller.textDidChange(nil)
    XCTAssertFalse(area.suppressesKeyHits)
    try send(.changed, at: 24, pan: pan, to: controller)
    XCTAssertTrue(document.offsets.isEmpty)
  }

  func testANullDocumentDuringTheDragRestoresHitsAndCannotResume() throws {
    let (controller, document) = try documentKeyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    try send(.began, at: 0, pan: pan, to: controller)
    XCTAssertTrue(area.suppressesKeyHits)
    let selector = #selector(getter: UITextDocumentProxy.documentIdentifier)
    let method = try XCTUnwrap(class_getInstanceMethod(Document.self, selector))
    let original = method_getImplementation(method)
    let block: @convention(block) (AnyObject) -> NSUUID? = { _ in nil }
    let replacement = imp_implementationWithBlock(block)
    method_setImplementation(method, replacement)
    do {
      defer {
        method_setImplementation(method, original)
        imp_removeBlock(replacement)
      }
      try send(.changed, at: 24, pan: pan, to: controller)
    }
    XCTAssertFalse(area.suppressesKeyHits)
    try send(.changed, at: 48, pan: pan, to: controller)
    XCTAssertTrue(document.offsets.isEmpty)
  }

  func testInvalidMovementRestoresHitsAndCannotResumeTheOldDrag() throws {
    let (controller, document) = try documentKeyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    for position in [CGFloat.nan, CGFloat.infinity, 5_000] {
      try send(.began, at: 0, pan: pan, to: controller)
      XCTAssertTrue(area.suppressesKeyHits)
      try send(.changed, at: position, pan: pan, to: controller)
      XCTAssertFalse(area.suppressesKeyHits)
      try send(.changed, at: 24, pan: pan, to: controller)
      XCTAssertTrue(document.offsets.isEmpty)
    }
  }

  func testDisappearingAndDisablingTheSettingRestoreHits() throws {
    let (controller, document) = try documentKeyboard()
    let area = try keyArea(controller)
    let pan = DirectionalPan()
    try send(.began, at: 0, pan: pan, to: controller)
    XCTAssertTrue(area.suppressesKeyHits)
    controller.viewWillDisappear(false)
    XCTAssertFalse(area.suppressesKeyHits)
    try send(.changed, at: 24, pan: pan, to: controller)
    XCTAssertTrue(document.offsets.isEmpty)
    controller.viewWillAppear(false)
    try send(.began, at: 0, pan: pan, to: controller)
    XCTAssertTrue(area.suppressesKeyHits)
    KeyboardLayoutPreference.defaults.set(false, forKey: preferenceKey)
    controller.viewWillAppear(false)
    XCTAssertFalse(area.suppressesKeyHits)
    try send(.changed, at: 24, pan: pan, to: controller)
    XCTAssertTrue(document.offsets.isEmpty)
  }

  func testGlideCancellationDoesNotReleaseAnActiveCursorDrag() throws {
    let (controller, _) = try documentKeyboard()
    let area = try keyArea(controller)
    let glide = try XCTUnwrap(area.gestureRecognizers?.compactMap { $0 as? GlideTypingGestureRecognizer }.first)
    let pan = DirectionalPan()
    try send(.began, at: 0, pan: pan, to: controller)
    glide.onCancelled()
    XCTAssertTrue(area.suppressesKeyHits)
    try send(.ended, at: 0, pan: pan, to: controller)
    XCTAssertFalse(area.suppressesKeyHits)
    glide.onBegan()
    XCTAssertTrue(area.suppressesKeyHits)
    try send(.cancelled, at: 0, pan: pan, to: controller)
    XCTAssertTrue(area.suppressesKeyHits)
    glide.onCancelled()
    XCTAssertFalse(area.suppressesKeyHits)
  }

  /// 用公开代理协议记录宿主收到的位移，未合入 #9375 时明确跳过依赖的用例。
  private final class Document: NSObject, UITextDocumentProxy {
    var documentIdentifier = UUID()
    var offsets: [Int] = []
    var insertions: [String] = []
    var documentContextBeforeInput: String? { "甲乙丙" }
    var documentContextAfterInput: String? { "丁戊己" }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    var hasText: Bool { true }
    func insertText(_ text: String) { insertions.append(text) }
    func deleteBackward() {}
    func adjustTextPosition(byCharacterOffset offset: Int) { offsets.append(offset) }
    func setMarkedText(_ text: String, selectedRange: NSRange) {}
    func unmarkText() {}
  }

  private func documentKeyboard() throws -> (KeyboardViewController, Document) {
    let document = Document()
    XCTAssertEqual(KeyboardHostContext.documentIdentifier(for: document), document.documentIdentifier,
      "公开代理文档身份读取必须可用，不能跳过文档移动回归")
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
    KeyboardLayoutPreference.defaults.set(true, forKey: preferenceKey)
    return (keyboard(), document)
  }

  private func keyArea(_ controller: KeyboardViewController) throws -> KeyAreaStackView {
    try XCTUnwrap(nodes(controller.view).first { $0 is KeyAreaStackView } as? KeyAreaStackView)
  }

  private func send(_ state: UIGestureRecognizer.State, at position: CGFloat,
                    pan: DirectionalPan, to controller: KeyboardViewController) throws {
    let selector = NSSelectorFromString("handleKeyboardCursorPan:")
    XCTAssertTrue(controller.responds(to: selector))
    guard controller.responds(to: selector) else { return }
    pan.phase = state
    pan.position = position
    controller.perform(selector, with: pan)
  }

  private func receivesSwipe(on view: UIView, controller: KeyboardViewController) -> Bool {
    let pan = DirectionalPan()
    pan.name = "keyboardCursorPan"
    let touch = StartingTouch()
    touch.startingView = view
    let delegate: UIGestureRecognizerDelegate = controller
    return delegate.gestureRecognizer?(pan, shouldReceive: touch) ?? true
  }

  private func receivesEvent(_ event: UIEvent, pan: UIPanGestureRecognizer, controller: KeyboardViewController) -> Bool {
    let delegate: UIGestureRecognizerDelegate = controller
    return delegate.gestureRecognizer?(pan, shouldReceive: event) ?? true
  }

  private final class TouchEvent: UIEvent {
    var touches: Set<UITouch> = []
    override var allTouches: Set<UITouch>? { touches }
  }

  private final class StartingTouch: UITouch {
    // 夹具持有明确的起点，临时创建的控件也必须保留至本次委托判定完成。
    var startingView: UIView?
    var phaseValue: UITouch.Phase = .began
    override var view: UIView? { startingView }
    override var phase: UITouch.Phase { phaseValue }
  }

  private final class DirectionalPan: UIPanGestureRecognizer {
    var direction = CGPoint(x: 300, y: 0)
    var phase: UIGestureRecognizer.State = .possible
    var position: CGFloat = 0
    override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
    override func velocity(in view: UIView?) -> CGPoint { direction }
    override func translation(in view: UIView?) -> CGPoint { CGPoint(x: position, y: 0) }
  }

  private func keyboard() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }
}

import ObjectiveC
import UIKit
import XCTest

/// 按住 ⌫ 横滑删除在键盘控制器上的行为：用例把 `textDocumentProxy` 换成内存里的文档，用一个状态和位移由用例给出的拖动手势驱动 `handleBackspaceSwipe`，看文档里删掉和恢复了什么。
@MainActor
final class BackspaceSwipeGestureTests: XCTestCase {
  /// 光标前的文字和选中的文字由用例给出；删和插都落在光标前。
  private final class Document: NSObject, UITextDocumentProxy {
    var text: String
    var selection: String?
    init(text: String, selection: String? = nil) {
      self.text = text
      self.selection = selection
    }
    var documentIdentifier = UUID()
    var documentInputMode: UITextInputMode? { nil }
    var hasText: Bool { !text.isEmpty }
    var documentContextBeforeInput: String? { text }
    var documentContextAfterInput: String? { "" }
    var selectedText: String? { selection }
    func insertText(_ inserted: String) { text += inserted }
    func deleteBackward() {
      if let selection, !selection.isEmpty {
        self.selection = nil
      } else if !text.isEmpty {
        text.removeLast()
      }
    }
    func setMarkedText(_ text: String, selectedRange: NSRange) {}
    func unmarkText() {}
    func adjustTextPosition(byCharacterOffset offset: Int) {}
  }

  /// ⌫ 上的拖动手势：状态和横向位移由用例给出。
  private final class SwipePan: UIPanGestureRecognizer {
    var phase: UIGestureRecognizer.State = .possible
    var x: CGFloat = 0
    override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
    override func translation(in view: UIView?) -> CGPoint { CGPoint(x: x, y: 0) }
    override func velocity(in view: UIView?) -> CGPoint { CGPoint(x: -100, y: 0) }
  }

  private func makeController() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    return controller
  }

  private func drive(_ controller: KeyboardViewController, _ pan: SwipePan, _ phase: UIGestureRecognizer.State, steps: Int) {
    pan.phase = phase
    pan.x = -CGFloat(steps) * CGFloat(KeyboardLayoutPreference.backspaceSwipeStep)
    controller.handleBackspaceSwipe(pan)
  }

  func testSwipingLeftDeletesTextAndSwipingBackRestoresIt() throws {
    let document = Document(text: "abcdef")
    try attach(document)
    let controller = makeController()
    defer { controller.viewDidDisappear(false) }
    let pan = SwipePan()
    drive(controller, pan, .began, steps: 0)
    drive(controller, pan, .changed, steps: 3)
    XCTAssertEqual(document.text, "abc")
    drive(controller, pan, .changed, steps: 1)
    XCTAssertEqual(document.text, "abcde")
    drive(controller, pan, .changed, steps: 0)
    XCTAssertEqual(document.text, "abcdef", "swiping back to the start restores everything")
    drive(controller, pan, .changed, steps: 2)
    drive(controller, pan, .ended, steps: 2)
    XCTAssertEqual(document.text, "abcd", "lifting keeps what was deleted, and deletes nothing more")
    XCTAssertNil(controller.backspaceSwipe)
  }

  /// 有选中的文字时不接横滑：第一次 deleteBackward 删掉的是整段选中，往回滑却会插回从没删过的字。
  func testASelectionKeepsTheSwipeOff() throws {
    let document = Document(text: "abc", selection: "def")
    try attach(document)
    let controller = makeController()
    defer { controller.viewDidDisappear(false) }
    XCTAssertNil(controller.backspaceSwipeUnit())
    let pan = SwipePan()
    drive(controller, pan, .began, steps: 1)
    drive(controller, pan, .changed, steps: 1)
    drive(controller, pan, .changed, steps: 0)
    XCTAssertEqual(document.text, "abc")
    XCTAssertEqual(document.selectedText, "def", "the selection is left to ⌫'s tap and hold")
    XCTAssertNil(controller.backspaceSwipe)
  }

  /// 组字时往左滑一格删一个音节，往回滑把删掉的音节按原样重打，组字回到删之前；全拼和九键（数字组码经 `handleCharacter` 的九键过滤重打）都一样，文档不动。
  func testSwipingACompositionDeletesBySyllableAndRestoresIt() throws {
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let document = Document(text: "")
    try attach(document)
    for (scheme, codes) in [(ChineseInputScheme.quanpin, "nihao"), (.nineKey, "64426")] {
      InputSchemePreference.scheme = scheme
      let controller = makeController()
      defer { controller.viewDidDisappear(false) }
      for code in codes { controller.handleCharacter(String(code)) }
      let original = controller.lastEditingText
      XCTAssertFalse(original.isEmpty, "\(scheme)")
      let pan = SwipePan()
      drive(controller, pan, .began, steps: 0)
      XCTAssertNotNil(controller.backspaceSwipe, "\(scheme): a composition takes the swipe")
      drive(controller, pan, .changed, steps: 1)
      let shortened = controller.lastEditingText
      XCTAssertFalse(shortened.isEmpty, "\(scheme): one step removes one syllable, not the whole composition")
      XCTAssertLessThan(shortened.count, original.count, "\(scheme)")
      drive(controller, pan, .changed, steps: 0)
      XCTAssertEqual(controller.lastEditingText, original, "\(scheme): swiping back retypes what was removed")
      drive(controller, pan, .ended, steps: 0)
      XCTAssertEqual(controller.lastEditingText, original, "\(scheme): lifting deletes nothing more")
      XCTAssertEqual(document.text, "", "\(scheme): composition edits never reach the document")
    }
  }

  /// 上屏后联想还在时横滑删文字，和点按 ⌫ 一样先清掉联想，免得删掉的字的后续词还留在候选栏上、数字键还按联想选词。
  func testSwipingCommittedTextClearsTheAfterCommitPrediction() throws {
    let document = Document(text: "你好")
    try attach(document)
    let controller = makeController()
    defer { controller.viewDidDisappear(false) }
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    controller.render(try bridge.snapshot(from: try NextWordPredictionTests.predictionFixture(flag: "true")))
    XCTAssertTrue(controller.enginePrediction)
    let pan = SwipePan()
    drive(controller, pan, .began, steps: 0)
    XCTAssertFalse(controller.enginePrediction, "starting the swipe clears the prediction")
    drive(controller, pan, .changed, steps: 2)
    drive(controller, pan, .ended, steps: 2)
    XCTAssertEqual(document.text, "")
    XCTAssertFalse(controller.enginePrediction)
  }

  /// 把键盘的 `textDocumentProxy` 换成 `document`，用例结束时恢复 UIKit 的实现（与 FourteenKeyKeyboardTests 相同的注入方式）。
  private func attach(_ document: Document) throws {
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
}

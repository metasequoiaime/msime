import UIKit
import XCTest

/// `textWillChange` 认得出自己改动的迟到回声，不再把下一键刚开始的组字结束掉；真正的外部改动仍然结束组字，残留的期待也不会一直吞掉它。
@MainActor
final class OwnEditEchoWindowTests: XCTestCase {
  // MARK: - 窗口本身

  func testNothingIsAnEchoBeforeTheKeyboardEditsAnything() {
    XCTAssertFalse(OwnEditEchoWindow().isEcho(at: 100))
  }

  func testCallbacksShortlyAfterAnOwnEditAreEchoesHoweverManyArrive() {
    var window = OwnEditEchoWindow()
    window.recordOwnEdit(at: 100)
    // 同一轮的几次改动可能合成一对回调，相邻两轮的回调也可能先后到；在窗口内的都认作回声，收到一次并不关窗。
    XCTAssertTrue(window.isEcho(at: 100.005))
    XCTAssertTrue(window.isEcho(at: 100.035))
    XCTAssertTrue(window.isEcho(at: 100 + OwnEditEchoWindow.duration))
  }

  func testALeftoverExpectationExpires() {
    var window = OwnEditEchoWindow()
    // 有的改动根本没有回调（实测 `insertText` 就没有），过了窗口到达的回调只能是外部改动。
    window.recordOwnEdit(at: 100)
    XCTAssertFalse(window.isEcho(at: 100 + OwnEditEchoWindow.duration + 0.01))
    XCTAssertFalse(window.isEcho(at: 200))
  }

  func testEveryOwnEditRestartsTheWindowAndResetClearsIt() {
    var window = OwnEditEchoWindow()
    window.recordOwnEdit(at: 100)
    window.recordOwnEdit(at: 100.4)
    XCTAssertTrue(window.isEcho(at: 100.8))
    window.reset()
    XCTAssertFalse(window.isEcho(at: 100.8))
  }

  // MARK: - 控制器

  private var savedInlineStyle: Any?
  private var savedScheme: ChineseInputScheme?

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    savedScheme = InputSchemePreference.scheme
    // 行内预编辑关闭（默认）：组字不写进宿主，敲字母本身不改文档。
    savedInlineStyle = InlinePreeditPreference.defaults.object(forKey: InlinePreeditPreference.styleKey)
    InlinePreeditPreference.style = .off
  }

  override func tearDown() {
    InlinePreeditPreference.defaults.set(savedInlineStyle, forKey: InlinePreeditPreference.styleKey)
    if let savedScheme { InputSchemePreference.scheme = savedScheme }
    super.tearDown()
  }

  private func controller() -> KeyboardViewController {
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func press(_ label: String, in controller: KeyboardViewController) throws {
    let key = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityLabel == label } as? UIButton, "no key labelled \(label)")
    key.sendActions(for: .primaryActionTriggered)
  }

  private func preedit(_ controller: KeyboardViewController) throws -> String? {
    let button = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)
    return button.configuration?.title
  }

  func testAHostChangeFinishesTheComposition() throws {
    let controller = self.controller()
    let idle = try preedit(controller)
    try press("字母 N", in: controller)
    try press("字母 I", in: controller)
    XCTAssertNotEqual(try preedit(controller), idle)
    // 键盘还没改过文档，这对回调只能来自宿主：组字照旧结束。
    controller.textWillChange(nil)
    XCTAssertEqual(try preedit(controller), idle)
  }

  func testTheLateEchoOfACommitLeavesTheNextCompositionAlone() throws {
    let controller = self.controller()
    try press("字母 N", in: controller)
    try press("字母 I", in: controller)
    try press("空格", in: controller)
    try press("字母 H", in: controller)
    try press("字母 A", in: controller)
    let composing = try preedit(controller)
    // 上屏那次 `insertText` 的回调这时才到。
    controller.textWillChange(nil)
    controller.textDidChange(nil)
    XCTAssertEqual(try preedit(controller), composing)
  }

  func testAHostChangeAfterTheWindowStillFinishesTheComposition() throws {
    let controller = self.controller()
    let idle = try preedit(controller)
    try press("字母 N", in: controller)
    try press("字母 I", in: controller)
    try press("空格", in: controller)
    try press("字母 H", in: controller)
    // 上屏没有等来回调；之后用户点到别处，不能被当成那次上屏的回声吞掉。
    RunLoop.current.run(until: Date().addingTimeInterval(OwnEditEchoWindow.duration + 0.1))
    controller.textWillChange(nil)
    XCTAssertEqual(try preedit(controller), idle)
  }
}

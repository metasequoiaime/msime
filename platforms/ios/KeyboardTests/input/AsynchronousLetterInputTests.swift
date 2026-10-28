import UIKit
import XCTest

/// 26 键全拼和 26 键双拼的字母交给会话线程查询（#10031）：主线程只投递，结果按顺序画出；空格、删除、Shift、收起、宿主编辑和切换中英先排空此前的字母，旧回调不能恢复已结束的组字。
@MainActor
final class AsynchronousLetterInputTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = previous }
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func keyboard(_ scheme: ChineseInputScheme) -> KeyboardViewController {
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.asynchronousInputEnabled = true
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func control(_ identifier: String, in controller: KeyboardViewController) throws -> UIControl {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIControl, identifier)
  }

  private func letterKey(_ letter: Character, in controller: KeyboardViewController) throws -> KeyboardKeyButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 \(letter.uppercased())" } as? KeyboardKeyButton,
                  "\(letter)")
  }

  /// 像松手输入那样按字母键；`touched` 时带一个偏离键心的按下点，双拼据此走触点入口。
  private func press(_ letter: Character, in controller: KeyboardViewController, touched: Bool = false) throws {
    let key = try letterKey(letter, in: controller)
    if touched, let area = nodes(controller.view).first(where: { $0 is KeyAreaStackView }) as? KeyAreaStackView {
      let frame = KeyAreaStackView.layoutFrame(of: key, in: area)
      key.touchDownLocation = CGPoint(x: frame.midX + frame.width * 0.3, y: frame.midY)
    }
    key.sendActions(for: .primaryActionTriggered)
  }

  /// 组字区里还没上屏的拉丁字母。
  private func preedit(_ controller: KeyboardViewController) -> String {
    (nodes(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)?
      .accessibilityValue?.filter { $0.isASCII && $0.isLetter } ?? ""
  }

  private func composing(_ controller: KeyboardViewController) throws -> Bool {
    try control("returnKey", in: controller).accessibilityLabel == "确认"
  }

  /// 让已排队的主线程回调都跑一轮。
  private func drainMainQueue() async {
    let drained = expectation(description: "主线程回调跑完")
    DispatchQueue.main.async { drained.fulfill() }
    await fulfillment(of: [drained], timeout: 10)
  }

  private func waitUntilSettled(_ controller: KeyboardViewController) async throws {
    let deadline = CACurrentMediaTime() + 10
    while controller.hasPendingAsynchronousInput && CACurrentMediaTime() < deadline {
      try await Task.sleep(nanoseconds: 1_000_000)
    }
    XCTAssertFalse(controller.hasPendingAsynchronousInput, "异步字母没有画完")
  }

  /// 字母投递后立即返回，不等引擎；会话线程被挡住时也不阻塞主线程。放开后按顺序画出整段拼写。双拼带按下点的字母同样异步，触点计数照常。
  func testLettersReturnBeforeTheEngineAnswersAndRenderInOrder() async throws {
    for (scheme, letters, touched) in [(ChineseInputScheme.quanpin, "nihao", false), (.shuangpin, "womf", true)] {
      let controller = keyboard(scheme)
      defer { controller.viewWillDisappear(false) }
      if touched {
        // 第一个带按下点的字母先同步注册键位几何；先打一键再删掉，挡住会话线程时就不会卡在注册上。
        try press("w", in: controller, touched: true)
        let delete = try control("letterDeleteKey", in: controller)
        delete.sendActions(for: .touchDown)
        delete.sendActions(for: .touchUpInside)
        XCTAssertFalse(try composing(controller))
      }
      let touchesBefore = controller.touchCharacterCount
      let gate = DispatchSemaphore(value: 0)
      defer { gate.signal() }
      MSIMEInputThread.shared.async { gate.wait() }
      for letter in letters { try press(letter, in: controller, touched: touched) }
      XCTAssertTrue(controller.hasPendingAsynchronousInput, "\(scheme)")
      XCTAssertEqual(preedit(controller), "", "\(scheme): 引擎还没回答，主线程不能先画出组字")
      gate.signal()
      try await waitUntilSettled(controller)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(preedit(controller), letters, "\(scheme)")
      XCTAssertTrue(try composing(controller), "\(scheme)")
      XCTAssertEqual(controller.touchCharacterCount - touchesBefore, touched ? letters.count : 0, "\(scheme)")
      try control("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(try composing(controller), "\(scheme): 空格上屏首选")
      await drainMainQueue()
      XCTAssertFalse(try composing(controller), "\(scheme): 上屏后旧回调不能恢复中间帧")
    }
  }

  /// 首个字母还在会话线程上时就按空格、删除、收起、宿主编辑或切换中英：边界先处理这个字母，事后的回调不能把已结束的组字画回来。
  func testFirstLetterIsConsumedBeforeEveryBoundary() async throws {
    for scheme in [ChineseInputScheme.quanpin, .shuangpin] {
      for boundary in ["space", "delete", "dismiss", "hostChange", "mode"] {
        let controller = keyboard(scheme)
        let gate = DispatchSemaphore(value: 0)
        defer { gate.signal() }
        MSIMEInputThread.shared.async { gate.wait() }
        try press("n", in: controller)
        gate.signal()
        switch boundary {
        case "space":
          try control("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)
        case "delete":
          let key = try control("letterDeleteKey", in: controller)
          key.sendActions(for: .touchDown)
          key.sendActions(for: .touchUpInside)
        case "hostChange":
          controller.textWillChange(nil)
        case "mode":
          try control("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
        default:
          controller.viewWillDisappear(false)
        }
        XCTAssertFalse(try composing(controller), "\(scheme) \(boundary)")
        XCTAssertFalse(controller.hasPendingAsynchronousInput, "\(scheme) \(boundary)")
        await drainMainQueue()
        XCTAssertFalse(try composing(controller), "\(scheme) \(boundary): 旧字母的回调不能恢复已结束的组字")
        if boundary != "dismiss" { controller.viewWillDisappear(false) }
      }
    }
  }

  /// 字母还没画出来就按 Shift：要按字母已在组字来判断。双拼组字中的 Shift 让下一个字母成为辅助码，不能因为画面还是空的就切到英文、把字母原样上屏；全拼的 Shift 是分词键，组字照常继续。
  func testShiftRightAfterALetterSeesTheComposition() async throws {
    for scheme in [ChineseInputScheme.shuangpin, .quanpin] {
      let controller = keyboard(scheme)
      defer { controller.viewWillDisappear(false) }
      let gate = DispatchSemaphore(value: 0)
      defer { gate.signal() }
      MSIMEInputThread.shared.async { gate.wait() }
      try press("w", in: controller)
      gate.signal()
      try control("shiftButton", in: controller).sendActions(for: .primaryActionTriggered)
      await drainMainQueue()
      XCTAssertTrue(try composing(controller), "\(scheme): Shift 之后仍在组字")
      XCTAssertEqual(preedit(controller), "w", "\(scheme)")
    }
  }

  /// 只有普通全拼九键、26 键全拼和 26 键双拼异步；五笔、日语等其他 26 键方案仍走同步入口，按完当场画出，与改动前一样。
  func testOtherSchemesStaySynchronous() throws {
    let quanpin = keyboard(.quanpin)
    defer { quanpin.viewWillDisappear(false) }
    try press("n", in: quanpin)
    XCTAssertTrue(quanpin.hasPendingAsynchronousInput, "全拼 26 键的字母异步")
    for scheme in [ChineseInputScheme.wubi, .japanese] {
      let controller = keyboard(scheme)
      defer { controller.viewWillDisappear(false) }
      try press(scheme == .wubi ? "g" : "a", in: controller)
      XCTAssertFalse(controller.hasPendingAsynchronousInput, "\(scheme)")
      if scheme == .wubi { XCTAssertEqual(preedit(controller), "g", "五笔同步入口当场画出组字") }
    }
  }
}

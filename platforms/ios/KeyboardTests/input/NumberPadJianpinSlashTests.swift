import UIKit
import XCTest

/// 经真实键盘数字页按键进入超级简拼，避免只验证桥接层而漏掉宿主的原样上屏分支。
@MainActor
final class NumberPadJianpinSlashTests: XCTestCase {
  override func setUpWithError() throws {
    try super.setUpWithError()
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences() ?? [:]
    let scheme = InputSchemePreference.scheme
    enableAllInputSchemes()
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = previous }
      InputSchemePreference.scheme = scheme
    }
    // 从出厂字段开始，只关闭测试不需要的学习和联网功能；不显式开启待测入口。
    let fresh = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: fresh) }
    let defaults = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: fresh))
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences {
      $0 = defaults
      $0["learning"] = false
      $0["cloud_candidates"] = false
      var ai = $0["ai_assistant"] as? [String: Any] ?? [:]
      ai["enabled"] = false
      $0["ai_assistant"] = ai
    })
  }

  func testDefaultShuangpinSlashConvertsAndBackspaceRestores() throws {
    let controller = keyboard(.shuangpin)
    defer { controller.viewDidDisappear(false) }
    try type("NUU", in: controller)
    let original = try preedit(controller)
    XCTAssertEqual(original, "nuu")
    try press("layoutToggleButton", in: controller)
    try press("运算符 /", in: controller)
    XCTAssertEqual(try preedit(controller), "Jnuu")
    let delete = try button("nineKeyDelete", in: controller)
    delete.sendActions(for: .touchDown)
    delete.sendActions(for: .touchUpInside)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try preedit(controller), original)
  }

  func testDisabledShortcutLeavesTheNumberPadLiteralRoute() throws {
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences {
      var modes = $0["local_modes"] as? [String: Any] ?? [:]
      modes["super_jianpin_slash"] = false
      $0["local_modes"] = modes
    })
    try assertSlashEndsComposition(.shuangpin, letters: "NUU")
  }

  func testOneSyllableShuangpinKeepsItsLiteralSlash() throws {
    try assertSlashEndsComposition(.shuangpin, letters: "NU")
  }

  func testQuanpinNumberPadDoesNotEnterSuperJianpin() throws {
    try assertSlashEndsComposition(.quanpin, letters: "NIHAO")
  }

  private func assertSlashEndsComposition(_ scheme: ChineseInputScheme, letters: String) throws {
    let controller = keyboard(scheme)
    defer { controller.viewDidDisappear(false) }
    try type(letters, in: controller)
    XCTAssertFalse(try preedit(controller).isEmpty)
    try press("layoutToggleButton", in: controller)
    try press("运算符 /", in: controller)
    XCTAssertEqual(try preedit(controller), "")
  }

  private func keyboard(_ scheme: ChineseInputScheme) -> KeyboardViewController {
    _ = InputSchemePreference.update { selection in
      selection.enabled = ChineseInputScheme.allCases
      selection.scheme = scheme
    }
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 402, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func type(_ letters: String, in controller: KeyboardViewController) throws {
    if try button("bottomLanguageKey", in: controller).accessibilityValue == "英文输入" {
      try press("bottomLanguageKey", in: controller)
    }
    for letter in letters { try press("字母 \(letter)", in: controller) }
  }

  private func preedit(_ controller: KeyboardViewController) throws -> String {
    try button("preeditButton", in: controller).accessibilityValue ?? ""
  }

  private func press(_ name: String, in controller: KeyboardViewController) throws {
    try button(name, in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
  }

  private func button(_ name: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).compactMap { $0 as? UIButton }.first {
      $0.accessibilityIdentifier == name || $0.accessibilityLabel == name
    }, name)
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }
}

import XCTest
import UIKit
import ObjectiveC

/// 随机数字（#10254）：键序策略与 Android `RandomDigitOrderPolicySmoke` 逐条相同；键盘上设置只放出「随」，点「随」只切换高亮、不跳页，数字九宫格的 0–9 打乱、每输入一个数字重新打乱，英文不给入口，关闭或隐藏工具栏恢复标准键序。
@MainActor
final class RandomDigitsTests: XCTestCase {
  private static let standard = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0]

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    // 用例改的是模拟器共用的 App Group 和文档，结束后方案、本机工具栏开关和共享的工具栏按钮都放回原样。
    let scheme = InputSchemePreference.scheme
    let local = (TouchToolbarLocalPreference.phrases, TouchToolbarLocalPreference.scheme,
                 TouchToolbarLocalPreference.hidden, TouchToolbarLocalPreference.randomDigits)
    let toolbar = MetasequoiaInputSessionBridge.loadSharedPreferences()?[TouchToolbarPreference.key]
    let hidesLanguage = KeyboardLayoutPreference.hideLanguageKey
    addTeardownBlock {
      InputSchemePreference.scheme = scheme
      KeyboardLayoutPreference.hideLanguageKey = hidesLanguage
      TouchToolbarLocalPreference.phrases = local.0
      TouchToolbarLocalPreference.scheme = local.1
      TouchToolbarLocalPreference.hidden = local.2
      TouchToolbarLocalPreference.randomDigits = local.3
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[TouchToolbarPreference.key] = toolbar }
    }
    TouchToolbarLocalPreference.hidden = false
    KeyboardLayoutPreference.hideLanguageKey = false
    InputSchemePreference.scheme = .quanpin
  }

  // MARK: - 键序策略

  func testThePolicyMatchesAndroid() {
    var calls = 0
    var state = 10254
    var policy = RandomDigitOrderPolicy { bound in
      calls += 1
      state = (state * 48271) % 2147483647
      return state % bound
    }
    XCTAssertFalse(policy.enabled)
    XCTAssertEqual(policy.faces(eligible: true), Self.standard, "默认不重排")
    policy.setEnabled(true)
    XCTAssertEqual(calls, 0, "字母页启用时不计算键序")
    XCTAssertEqual(policy.faces(eligible: false), Self.standard, "英文和字母页保留标准排列")
    policy.afterDigit(eligible: false)
    XCTAssertEqual(calls, 0, "页面外数字输入不取随机数")
    var previous = policy.faces(eligible: true)
    for _ in 0..<1000 {
      XCTAssertEqual(previous.sorted(), Array(0...9), "十个数字完整且无重复")
      let before = calls
      XCTAssertEqual(policy.faces(eligible: true), previous, "普通重画不换序")
      XCTAssertEqual(calls, before)
      _ = policy.faces(eligible: false)
      policy.afterDigit(eligible: false)
      XCTAssertEqual(calls, before, "离开数字页不计算")
      XCTAssertTrue(policy.enabled, "离开数字页不关闭")
      XCTAssertEqual(policy.faces(eligible: true), previous, "返回数字页沿用上次键序")
      policy.afterDigit(eligible: true)
      let next = policy.faces(eligible: true)
      XCTAssertNotEqual(next, previous, "每次数字输入后换序")
      previous = next
    }
    let before = calls
    policy.setEnabled(false)
    XCTAssertEqual(policy.faces(eligible: true), Self.standard, "关闭立即恢复")
    policy.afterDigit(eligible: true)
    XCTAssertEqual(calls, before, "关闭后不计算")
  }

  func testAnIdentityShuffleStillChangesTheOrder() {
    var policy = RandomDigitOrderPolicy { $0 - 1 }
    policy.setEnabled(true)
    let first = policy.faces(eligible: true)
    XCTAssertNotEqual(first, Self.standard, "首次仍换序")
    policy.afterDigit(eligible: true)
    XCTAssertNotEqual(policy.faces(eligible: true), first, "恒等排列不能使输入后的换序失效")
  }

  // MARK: - 键盘

  /// 设置只放出「随」；点「随」只亮起、不跳页；数字九宫格打乱、按键面上屏后重新打乱，离开再回来沿用；再点「随」恢复标准键序。
  func testTheToolbarEntryShufflesTheNumberPadAndTypesWhatItShows() throws {
    TouchToolbarLocalPreference.randomDigits = false
    var controller = keyboard()
    XCTAssertFalse(shown(try button("randomDigitsShortcut", in: controller)), "the entry is off by default")
    controller.viewDidDisappear(false)

    TouchToolbarLocalPreference.randomDigits = true
    controller = keyboard()
    let document = Document()
    try attach(document)
    let entry = try button("randomDigitsShortcut", in: controller)
    XCTAssertTrue(shown(entry), "the setting only shows the entry")
    XCTAssertEqual(entry.configuration?.title, "随")
    XCTAssertEqual(entry.accessibilityValue, "已关闭")
    try toggleLayer(controller)
    XCTAssertEqual(try faces(controller), Self.standard, "showing the entry does not turn shuffling on")
    try toggleLayer(controller)

    entry.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(entry.accessibilityValue, "已开启")
    XCTAssertTrue(entry.accessibilityTraits.contains(.selected))
    XCTAssertFalse(shown(try button("nineKey1", in: controller)), "tapping 随 does not open the number pad")

    try toggleLayer(controller)
    var order = try faces(controller)
    XCTAssertEqual(order.sorted(), Array(0...9))
    XCTAssertNotEqual(order, Self.standard, "the number pad is shuffled")
    XCTAssertEqual(try button("nineKeyMiddleKey", in: controller).configuration?.title, ".", "other keys stay put")
    var typed = ""
    for _ in 0..<12 {
      let slot = try XCTUnwrap(order.firstIndex(of: 4))
      let key = try slotButton(slot, in: controller)
      XCTAssertEqual(key.accessibilityLabel, "数字 4", "the screen reader reads the face")
      key.sendActions(for: .primaryActionTriggered)
      typed += "4"
      XCTAssertEqual(document.text, typed, "the key types the digit it shows")
      let next = try faces(controller)
      XCTAssertEqual(next.sorted(), Array(0...9))
      XCTAssertNotEqual(next, order, "every digit reshuffles the pad")
      order = next
    }

    // 离开数字页再回来沿用上次排列。
    try toggleLayer(controller)
    try toggleLayer(controller)
    XCTAssertEqual(try faces(controller), order, "coming back keeps the last order")

    entry.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(entry.accessibilityValue, "已关闭")
    XCTAssertEqual(try faces(controller), Self.standard, "turning it off restores the standard order")
    try button("nineKey7", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, typed + "7")
    controller.viewDidDisappear(false)
  }

  /// 临时模式（U 模式）里数字页是标准键序，数字交给会话组字、不换序；退出模式后回到进入前的随机排列，与 Android `RandomDigitsDeviceChecks` 的 U 模式检查相同。iOS 从工具菜单打开的模式退出时 `render` 会回到字母页（Android 留在数字页），所以退出后重新打开数字页再核对排列。
  func testALocalModeShowsTheStandardOrderAndLeavingItRestoresTheShuffle() throws {
    TouchToolbarLocalPreference.randomDigits = true
    let controller = keyboard()
    let document = Document()
    try attach(document)
    let entry = try button("randomDigitsShortcut", in: controller)
    entry.sendActions(for: .primaryActionTriggered)
    try toggleLayer(controller)
    let shuffled = try faces(controller)
    XCTAssertNotEqual(shuffled, Self.standard)
    try toggleLayer(controller)

    controller.openLocalInputMode("U")
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(entry), "a local mode draws no 随")
    try toggleLayer(controller)
    XCTAssertEqual(try faces(controller), Self.standard, "the number pad keeps the standard order in a local mode")
    try button("nineKey4", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try faces(controller), Self.standard, "a digit typed in a local mode does not reshuffle")

    // 第一次退格删掉 4；只剩触发字母时模式可能已经退出，所以「随」还没回来时才再退一次。
    let delete = try button("nineKeyDelete", in: controller)
    for _ in 0..<2 where !shown(entry) {
      delete.sendActions(for: .touchDown)
      delete.sendActions(for: .touchUpInside)
      controller.view.layoutIfNeeded()
    }
    XCTAssertTrue(shown(entry), "leaving the local mode brings 随 back")
    XCTAssertEqual(entry.accessibilityValue, "已开启", "the local mode does not turn shuffling off")
    XCTAssertFalse(shown(try button("nineKey1", in: controller)), "leaving a mode opened from the tools returns to the letters")
    try toggleLayer(controller)
    XCTAssertEqual(try faces(controller), shuffled, "leaving the mode restores the order from before it")
    XCTAssertEqual(document.text, "", "nothing reached the document")
    controller.viewDidDisappear(false)
  }

  /// 英文键盘不给入口、也不打乱；工具栏设成隐藏时清掉随机状态，取消隐藏后「随」仍是关着的。
  func testEnglishHasNoEntryAndHidingTheToolbarTurnsShufflingOff() throws {
    TouchToolbarLocalPreference.randomDigits = true
    let controller = keyboard()
    let entry = try button("randomDigitsShortcut", in: controller)
    entry.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(entry.accessibilityValue, "已开启")

    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(shown(entry), "English draws no 随")
    try toggleLayer(controller)
    XCTAssertEqual(try faces(controller), Self.standard, "the English number pad is not shuffled")
    try toggleLayer(controller)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(entry))
    XCTAssertEqual(entry.accessibilityValue, "已开启", "switching languages does not turn it off")

    TouchToolbarLocalPreference.hidden = true
    controller.viewWillAppear(false)
    TouchToolbarLocalPreference.hidden = false
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(shown(entry))
    XCTAssertEqual(entry.accessibilityValue, "已关闭", "hiding the toolbar turned shuffling off")
    try toggleLayer(controller)
    XCTAssertEqual(try faces(controller), Self.standard)
    controller.viewDidDisappear(false)
  }

  /// 其他可选按钮全关、只开着「随」时，中文键盘的工具栏这一行照常显示；切到英文时「随」不在，这一行也不消失，与 Android 的 `randomDigitsKeepsToolbarRow` 相同。没开「随」时这一行照旧不显示。
  func testTheRandomDigitsEntryAloneKeepsTheToolbarRow() throws {
    var none = TouchToolbarPreference()
    for option in TouchToolbarPreference.options { none[keyPath: option.keyPath] = false }
    XCTAssertTrue(TouchToolbarPreference.save(none))
    TouchToolbarLocalPreference.phrases = false
    TouchToolbarLocalPreference.scheme = false

    TouchToolbarLocalPreference.randomDigits = false
    var controller = keyboard()
    XCTAssertTrue(try bar(controller).isHidden, "nothing but the logo and the collapse button")
    controller.viewDidDisappear(false)

    TouchToolbarLocalPreference.randomDigits = true
    controller = keyboard()
    XCTAssertFalse(try bar(controller).isHidden)
    XCTAssertEqual(try visibleTools(controller), ["moreShortcut", "randomDigitsShortcut", "dismissShortcut"])
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(try bar(controller).isHidden, "English keeps the row")
    XCTAssertEqual(try visibleTools(controller), ["moreShortcut", "dismissShortcut"])
    controller.viewDidDisappear(false)
  }

  // MARK: - 辅助

  private func keyboard() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 402, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func toggleLayer(_ controller: KeyboardViewController) throws {
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
  }

  /// 键位 0–8 是网格 nineKey1…nineKey9，键位 9 是底行的 0。
  private func slotButton(_ slot: Int, in controller: KeyboardViewController) throws -> UIButton {
    try button(slot == 9 ? "nineKeyZero" : "nineKey\(slot + 1)", in: controller)
  }

  /// 数字九宫格十个键位此刻画的数字，并核对读屏名称与键面一致。
  private func faces(_ controller: KeyboardViewController) throws -> [Int] {
    try (0..<10).map { slot in
      let key = try slotButton(slot, in: controller)
      XCTAssertTrue(shown(key), "slot \(slot) is on screen")
      let digit = try XCTUnwrap(key.configuration?.title.flatMap { Int($0) }, "slot \(slot)")
      XCTAssertEqual(key.accessibilityLabel, "数字 \(digit)", "slot \(slot)")
      return digit
    }
  }

  private func bar(_ controller: KeyboardViewController) throws -> UIStackView {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" } as? UIStackView)
  }

  private func visibleTools(_ controller: KeyboardViewController) throws -> [String] {
    try bar(controller).arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier)
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func shown(_ view: UIView) -> Bool {
    sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden }
  }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }

  /// 内存里的一份文档：测试宿主里的 `textDocumentProxy` 没有文档，上屏看不到（见 `KeyLayoutStyleTests.SidebarDocument`）。
  private final class Document: NSObject, UITextDocumentProxy {
    var text = ""
    var documentContextBeforeInput: String? { text }
    var documentContextAfterInput: String? { nil }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    var documentIdentifier = UUID()
    var hasText: Bool { !text.isEmpty }
    func insertText(_ inserted: String) { text += inserted }
    func deleteBackward() { if !text.isEmpty { text.removeLast() } }
    func adjustTextPosition(byCharacterOffset offset: Int) {}
    func setMarkedText(_ markedText: String, selectedRange: NSRange) {}
    func unmarkText() {}
  }

  private static let proxySelector = #selector(getter: UIInputViewController.textDocumentProxy)

  /// 让 `KeyboardViewController` 的 `textDocumentProxy` 返回 `document`，用例结束时换回父类的实现。
  private func attach(_ document: Document) throws {
    let inherited = try XCTUnwrap(class_getInstanceMethod(UIInputViewController.self, Self.proxySelector))
    let original = method_getImplementation(inherited)
    let block: @convention(block) (AnyObject) -> AnyObject = { _ in document }
    let replacement = imp_implementationWithBlock(block)
    if !class_addMethod(KeyboardViewController.self, Self.proxySelector, replacement, method_getTypeEncoding(inherited)) {
      method_setImplementation(try XCTUnwrap(class_getInstanceMethod(KeyboardViewController.self, Self.proxySelector)), replacement)
    }
    addTeardownBlock {
      if let own = class_getInstanceMethod(KeyboardViewController.self, Self.proxySelector) {
        method_setImplementation(own, original)
      }
      imp_removeBlock(replacement)
    }
  }
}

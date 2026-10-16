import ObjectiveC
import UIKit
import XCTest

/// 光标前打到 `xxx@` 时键盘候选条给出邮箱后缀，点一下补全。
///
/// 测试宿主里的 `textDocumentProxy` 没有文档（见 `SmartPunctuationGestureTests`），上屏和上文都看不到。所以用例期间把 `KeyboardViewController` 的 `textDocumentProxy` 换成一份内存里的文档 `Document`，结束时换回 `UIInputViewController` 自己的实现。控制器经 Objective-C 消息读这个属性，换掉它就是键盘看到的全部上文。
@MainActor
final class EmailSuffixKeyboardTests: XCTestCase {
  /// 光标总在末尾的一份文档：插入接在后面，退格删掉最后一个字符。
  @MainActor
  private final class Document: NSObject, UITextDocumentProxy {
    var text: String

    init(_ text: String) {
      self.text = text
    }

    var documentContextBeforeInput: String? { text }
    var documentContextAfterInput: String? { nil }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    let documentIdentifier = UUID()
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

  private func makeKeyboard(_ document: Document) throws -> KeyboardViewController {
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    try attach(document)
    return controller
  }

  func testAddressBeforeTheCaretOffersSuffixesAndTappingOneCompletesIt() throws {
    let document = Document("zhangsan@")
    let controller = try makeKeyboard(document)
    controller.textDidChange(nil)

    let shown = chips(in: controller)
    XCTAssertEqual(shown.map { $0.configuration?.title }, EmailSuffixPolicy.suffixes, "二十个后缀全排在候选条上，不分页")
    let first = try button("candidate-1", in: controller)
    XCTAssertEqual(first.configuration?.title, "@qq.com")
    XCTAssertEqual(first.accessibilityLabel, "邮箱后缀 1：@qq.com")
    XCTAssertEqual(try button("candidate-2", in: controller).accessibilityLabel, "邮箱后缀 2：@163.com")

    first.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "zhangsan@qq.com")
    XCTAssertTrue(chips(in: controller).isEmpty, "补全后上文不再匹配，候选条回到空闲")
    spinMainRunLoop()
    XCTAssertTrue(chips(in: controller).isEmpty)
  }

  func testTypedDomainNarrowsTheSuffixesAndIsReplacedInLowercase() throws {
    let document = Document("我的邮箱是 zhangsan@GM")
    let controller = try makeKeyboard(document)
    controller.selectionDidChange(nil)

    XCTAssertEqual(chips(in: controller).map { $0.configuration?.title }, ["@gmail.com", "@gmx.com"])
    try button("candidate-1", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "我的邮箱是 zhangsan@gmail.com")
  }

  /// 上文在点下之前已经变了：不再补全，只收起。
  func testTappingAfterTheContextChangedOnlyDismisses() throws {
    let document = Document("zhangsan@")
    let controller = try makeKeyboard(document)
    controller.textDidChange(nil)
    document.text = "zhangsan@ "
    try button("candidate-1", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, "zhangsan@ ")
    XCTAssertTrue(chips(in: controller).isEmpty)
  }

  func testNoSuffixesWithoutAnAddress() throws {
    for text in ["微信@", "zhangsan@163.com", "zhangsan＠"] {
      let controller = try makeKeyboard(Document(text))
      controller.textDidChange(nil)
      XCTAssertTrue(chips(in: controller).isEmpty, text)
    }
  }

  /// 键盘自己上屏的 `@` 之后候选条给出后缀：下滑或长按 a 键输入的角标 `@` 在中英文模式下都有，英文 123 层也有 `@`（中文模式的 123 是数字页，没有它）。中文模式的角标先结束组字再插入，插入后不再更新候选条，靠下一轮的重新匹配；英文模式插入后更新英文建议时就给出。两条路线都不等宿主回调 `textDidChange`，键盘自己的插入 iOS 不回调。
  func testAtTypedByTheKeyboardOffersSuffixesInBothModes() throws {
    let document = Document("")
    let controller = try makeKeyboard(document)
    var typedOnNumberLayer = false
    // 起始模式取决于模拟器里留下的模式记忆；第二轮切到另一种模式，两轮合起来覆盖中英文，最后切回去，不改动留给后面用例的模式。
    for round in 0..<2 {
      if round == 1 { try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered) }

      document.text = "zhangsan"
      controller.textDidChange(nil)
      XCTAssertEqual(emailSuffixChips(in: controller), [], "round \(round)")
      let a = try XCTUnwrap(descendants(controller.view).first {
        ["字母 A", "大写 A"].contains($0.accessibilityLabel ?? "")
      } as? KeyboardKeyButton)
      try XCTUnwrap(a.onCornerHint)("@")
      XCTAssertEqual(document.text, "zhangsan@", "round \(round)")
      spinMainRunLoop()
      XCTAssertEqual(chips(in: controller).first?.configuration?.title, "@qq.com", "corner hint, round \(round)")

      document.text = "lisi"
      controller.textDidChange(nil)
      // 后缀不再匹配后，英文模式换回 `lisi` 的英文建议，中文模式回到空闲。
      XCTAssertEqual(emailSuffixChips(in: controller), [], "round \(round)")
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let at = try button("symbolLayerKey1_7", in: controller)
      if at.configuration?.title == "@" && shown(at, in: controller) {
        typedOnNumberLayer = true
        at.sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(document.text, "lisi@")
        spinMainRunLoop()
        XCTAssertEqual(chips(in: controller).first?.configuration?.title, "@qq.com", "123 layer")
      }
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    }
    XCTAssertTrue(typedOnNumberLayer, "one of the two modes is English, whose 123 layer has @")
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
  }

  /// 让自己改动文档后排在下一轮的重新匹配跑完。
  private func spinMainRunLoop() {
    RunLoop.main.run(until: Date().addingTimeInterval(0.05))
  }

  /// 候选条上看得见的词块，按编号排好。
  private func chips(in controller: KeyboardViewController) -> [UIButton] {
    func number(_ chip: UIButton) -> Int { Int(chip.accessibilityIdentifier?.dropFirst("candidate-".count) ?? "") ?? 0 }
    return descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && number($0) > 0 && shown($0, in: controller) }
      .sorted { number($0) < number($1) }
  }

  /// 候选条上读作邮箱后缀的词块。
  private func emailSuffixChips(in controller: KeyboardViewController) -> [String] {
    chips(in: controller).filter { $0.accessibilityLabel?.hasPrefix("邮箱后缀") == true }.compactMap { $0.configuration?.title }
  }

  /// 这个视图和它到键盘根视图之间的每一层都没有隐藏。
  private func shown(_ view: UIView, in controller: KeyboardViewController) -> Bool {
    var node: UIView? = view
    while let current = node, current !== controller.view {
      if current.isHidden { return false }
      node = current.superview
    }
    return true
  }

  private func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }
}

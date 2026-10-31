import ObjectiveC
import UIKit
import XCTest

/// 光标前打到 `xxx@` 时键盘候选条给出邮箱后缀，点一下补全。
///
/// 测试宿主里的 `textDocumentProxy` 没有文档（见 `SmartPunctuationGestureTests`），上屏和上文都看不到。所以用例期间把 `KeyboardViewController` 的 `textDocumentProxy` 换成一份内存里的文档 `Document`，结束时换回 `UIInputViewController` 自己的实现。控制器经 Objective-C 消息读这个属性，换掉它就是键盘看到的全部上文。
@MainActor
final class EmailSuffixKeyboardTests: XCTestCase {
  /// 默认光标在末尾；有选区时，上下文分开读取，插入和退格实际替换选中内容。
  @MainActor
  private final class Document: NSObject, UITextDocumentProxy {
    var text: String
    var selectedRange: NSRange?

    init(_ text: String) {
      self.text = text
    }

    var documentContextBeforeInput: String? {
      selectedRange.map { (text as NSString).substring(to: $0.location) } ?? text
    }
    var documentContextAfterInput: String? {
      selectedRange.map { (text as NSString).substring(from: NSMaxRange($0)) }
    }
    var selectedText: String? {
      selectedRange.map { (text as NSString).substring(with: $0) }
    }
    var documentInputMode: UITextInputMode? { nil }
    var documentIdentifier = UUID()
    var hasText: Bool { !text.isEmpty }

    func insertText(_ inserted: String) {
      if let selectedRange {
        text = (text as NSString).replacingCharacters(in: selectedRange, with: inserted)
        self.selectedRange = nil
      } else {
        text += inserted
      }
    }
    func deleteBackward() {
      if selectedRange != nil {
        insertText("")
      } else if !text.isEmpty {
        text.removeLast()
      }
    }
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

  /// 键盘自己上屏的 `@` 之后候选条给出后缀：下滑或长按 a 键输入的角标 `@` 在中英文模式下都有。123 在中英文下都打开数字九宫格，上面没有 `@`。中文模式的角标先结束组字再插入，插入后不再更新候选条，靠下一轮的重新匹配；英文模式插入后更新英文建议时就给出。两条路线都不等宿主回调 `textDidChange`，键盘自己的插入 iOS 不回调。
  func testAtTypedByTheKeyboardOffersSuffixesInBothModes() throws {
    let document = Document("")
    let controller = try makeKeyboard(document)
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
      XCTAssertTrue(shown(try button("nineKey1", in: controller), in: controller), "round \(round)")
      try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    }
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
  }

  /// 编辑器改了上文、变化回报尚未到达时，点击旧答案也不能替换另一词。
  func testEnglishCorrectionDoesNotReplaceADifferentCurrentWord() throws {
    let document = Document("langug")
    let controller = try makeEnglishKeyboard(document)
    try waitForEnglishCandidate("language", in: controller)
    let correction = try button("candidate-2", in: controller)

    document.text = "hello"
    correction.sendActions(for: .primaryActionTriggered)

    XCTAssertEqual(document.text, "hello", "旧词的纠错不能替换编辑器里的新词")
  }

  /// 两个输入框恰好有相同拼写，也不能沿用上一输入框的答案。
  func testEnglishCorrectionDoesNotCrossDocumentIdentity() throws {
    let document = Document("langug")
    let controller = try makeEnglishKeyboard(document)
    try waitForEnglishCandidate("language", in: controller)
    let correction = try button("candidate-2", in: controller)

    document.documentIdentifier = UUID()
    correction.sendActions(for: .primaryActionTriggered)

    XCTAssertEqual(document.text, "languge", "旧输入框的纠错不能写进新输入框")
  }

  /// 光标前的词没有变化，但用户已经选中后面的文本，旧答案不能退格删选区。
  func testEnglishCorrectionDoesNotReplaceSelectedText() throws {
    let document = Document("langug")
    let controller = try makeEnglishKeyboard(document)
    try waitForEnglishCandidate("language", in: controller)
    let correction = try button("candidate-2", in: controller)
    document.text = "languge other"
    document.selectedRange = NSRange(location: 7, length: 6)

    correction.sendActions(for: .primaryActionTriggered)

    XCTAssertEqual(document.text, "languge other")
    XCTAssertEqual(document.selectedText, " other", "点旧纠错不能改动用户的选区")
  }

  /// 空格可以按编辑器规则替换选区，但不能先纠正选区前的词并多删字符。
  func testEnglishBoundaryDoesNotCorrectTheWordBeforeSelectedText() throws {
    let document = Document("langug")
    let controller = try makeEnglishKeyboard(document)
    try waitForEnglishCandidate("language", in: controller)
    document.text = "languge other"
    document.selectedRange = NSRange(location: 7, length: 6)

    try button("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)

    XCTAssertEqual(document.text, "languge ", "空格只替换选中的内容，保留选区前的原文")
    XCTAssertNil(document.selectedText)
  }

  /// 新查询尚未回到主线程时先展示这一键之后的原文，上一词的答案不再可点。
  func testEnglishQueryImmediatelyReplacesThePreviousWordsSuggestions() throws {
    let document = Document("langug")
    let controller = try makeEnglishKeyboard(document)
    try waitForEnglishCandidate("language", in: controller)
    let x = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityLabel == "字母 X" && shown($0, in: controller)
    } as? UIButton)

    x.sendActions(for: .primaryActionTriggered)

    XCTAssertEqual(document.text, "langugex")
    XCTAssertEqual(chips(in: controller).compactMap { $0.configuration?.title }, [document.text],
                   "查询结果回来前只给当前原文，不保留旧词的纠错")
  }

  /// 真实候选条展示的是前缀补全时，点原文只收起，不生成拒绝纠错记录。
  func testChoosingOriginalPrefixDoesNotRejectACompletion() throws {
    let document = Document("en")
    let controller = try makeEnglishKeyboard(document, finalLetter: "G", expectedWord: "eng")
    try waitForEnglishCandidate("English", in: controller)

    try button("candidate-1", in: controller).sendActions(for: .primaryActionTriggered)

    XCTAssertEqual(document.text, "eng")
    let rejectedPrefix = try XCTUnwrap(Mirror(reflecting: controller).children.first {
      $0.label == "englishCorrectionRejectedPrefix"
    }?.value as? String)
    XCTAssertEqual(rejectedPrefix, "", "前缀补全的原文不能被记成拒绝纠错")
  }

  /// 空格不得把展示栏中的第二个前缀补全当成唯一纠错自动上屏。
  func testEnglishBoundaryDoesNotCommitDisplayedPrefixCompletion() throws {
    let document = Document("en")
    let controller = try makeEnglishKeyboard(document, finalLetter: "G", expectedWord: "eng")
    try waitForEnglishCandidate("English", in: controller)

    try button("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)

    XCTAssertEqual(document.text, "eng ")
  }

  private func makeEnglishKeyboard(_ document: Document, finalLetter: String = "E",
                                   expectedWord: String = "languge") throws -> KeyboardViewController {
    let previous = EnglishSuggestionsPreference.isEnabled
    EnglishSuggestionsPreference.isEnabled = true
    addTeardownBlock { EnglishSuggestionsPreference.isEnabled = previous }
    let controller = try makeKeyboard(document)
    let language = try button("bottomLanguageKey", in: controller)
    let startedInEnglish = language.accessibilityValue == "英文输入"
    if startedInEnglish { language.sendActions(for: .primaryActionTriggered) }
    language.sendActions(for: .primaryActionTriggered)
    addTeardownBlock {
      if !startedInEnglish { language.sendActions(for: .primaryActionTriggered) }
    }
    controller.textDidChange(nil)
    // 无邮箱后缀时，文档回调和模式切换都不启动英文查询；通过真实字母键补全待纠错原文。
    let letter = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityLabel == "字母 " + finalLetter && shown($0, in: controller)
    } as? UIButton)
    letter.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, expectedWord, "必须实际输入末尾字母，触发本次英文查询")
    return controller
  }

  private func waitForEnglishCandidate(_ candidate: String, in controller: KeyboardViewController) throws {
    let deadline = Date().addingTimeInterval(10)
    while !chips(in: controller).contains(where: { $0.configuration?.title == candidate }) && Date() < deadline {
      RunLoop.main.run(until: Date().addingTimeInterval(0.01))
    }
    _ = try XCTUnwrap(chips(in: controller).first { $0.configuration?.title == candidate },
                      "英文查询必须先实际返回候选，才能验证点击与过期结果的处理")
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

import ObjectiveC
import XCTest
import UIKit

/// 行内预编辑 keeps the host's marked text equal to the composition, and context reads look past it.
final class InlineCompositionTests: XCTestCase {
  func testEditsOnlyWhenTheMarkedTextChanges() {
    XCTAssertNil(InlineCompositionPolicy.edit(showing: "", next: ""))
    XCTAssertNil(InlineCompositionPolicy.edit(showing: "ni", next: "ni"))
    XCTAssertEqual(InlineCompositionPolicy.edit(showing: "", next: "n"), .mark("n"))
    XCTAssertEqual(InlineCompositionPolicy.edit(showing: "ni", next: "ni'h"), .mark("ni'h"))
    XCTAssertEqual(InlineCompositionPolicy.edit(showing: "ni", next: ""), .clear)
  }

  func testContextBeforeCompositionDropsOnlyTheMarkedSuffix() {
    XCTAssertEqual(InlineCompositionPolicy.contextBefore("你好，ni'hao", marked: "ni'hao"), "你好，")
    XCTAssertEqual(InlineCompositionPolicy.contextBefore("你好，", marked: ""), "你好，")
    // A host that does not report the marked text before the caret is left as it is.
    XCTAssertEqual(InlineCompositionPolicy.contextBefore("你好，", marked: "ni"), "你好，")
    XCTAssertNil(InlineCompositionPolicy.contextBefore(nil, marked: "ni"))
  }

  func testEachStyleWritesItsOwnFormOfTheComposition() {
    typealias Style = InlinePreeditPreference.Style
    XCTAssertEqual(Style.off.text(phrasePrefix: "你", preedit: "hao", editingText: "hao", japaneseReading: nil), "")
    XCTAssertEqual(Style.pinyin.text(phrasePrefix: "", preedit: "ni'hao", editingText: "nihao", japaneseReading: nil), "ni'hao")
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "ni'hao", editingText: "nihao", japaneseReading: nil), "nihao")
    // Shuangpin keys stay keys in raw and expand in pinyin, as Windows draws them.
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "shi", editingText: "ui", japaneseReading: nil), "ui")
    XCTAssertEqual(Style.raw.text(phrasePrefix: "你好", preedit: "shi'jie", editingText: "shijie", japaneseReading: nil), "你好shijie")
    // No ASCII keys (a local mode) falls back to what the Engine shows.
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "〔笔画〕", editingText: "", japaneseReading: nil), "〔笔画〕")
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "かな", editingText: "kana", japaneseReading: "かな"), "かな")
  }

  /// Korean marks the Hangul syllable whatever the style says, never its Dubeolsik key letters.
  func testKoreanAlwaysMarksTheSyllable() {
    for style in InlinePreeditPreference.Style.allCases {
      XCTAssertEqual(InlineCompositionPolicy.markedText(inPlace: true, style: style, phrasePrefix: "", preedit: "녕",
                                                        editingText: "sud", japaneseReading: nil), "녕", "\(style)")
    }
    XCTAssertEqual(InlineCompositionPolicy.markedText(inPlace: false, style: .off, phrasePrefix: "", preedit: "ni'hao",
                                                      editingText: "nihao", japaneseReading: nil), "")
    XCTAssertEqual(InlineCompositionPolicy.markedText(inPlace: false, style: .raw, phrasePrefix: "", preedit: "ni'hao",
                                                      editingText: "nihao", japaneseReading: nil), "nihao")
  }
}

/// 拼写固定写进输入框（拼音分词），候选栏不再画读音：没有可见读音时不保留编辑入口，候选按键时不左右移动；九键写进去的是拼音，不是数字键。
@MainActor
final class InlineCompositionDefaultTests: XCTestCase {
  private var savedScheme: ChineseInputScheme?

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    savedScheme = InputSchemePreference.scheme
  }

  override func tearDown() {
    if let savedScheme { InputSchemePreference.scheme = savedScheme }
    super.tearDown()
  }

  private func controller(_ scheme: ChineseInputScheme) -> KeyboardViewController {
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }

  private func view(_ identifier: String, in controller: KeyboardViewController) throws -> UIView {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier }, identifier)
  }

  private func tap(_ predicate: (UIView) -> Bool, in controller: KeyboardViewController) throws {
    try XCTUnwrap(descendants(controller.view).first(where: predicate) as? UIButton).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
  }

  func testQuanpinMarksThePinyinAndKeepsTheFirstCandidateInPlace() throws {
    let controller = controller(.quanpin)
    var firstCandidateX: [CGFloat] = []
    for letter in ["N", "I", "H", "A", "O"] {
      try tap({ $0.accessibilityLabel == "字母 \(letter)" }, in: controller)
      let preedit = try XCTUnwrap(try view("preeditButton", in: controller) as? UIButton)
      XCTAssertEqual(preedit.configuration?.title ?? "", "", "候选栏在 \(letter) 后不重复画拼写")
      XCTAssertNil(preedit.configuration?.image, "候选栏不显示拼写编辑铅笔")
      XCTAssertTrue(preedit.isHidden, "没有可见读音时不保留空白编辑入口")
      let chip = try view("candidate-1", in: controller)
      firstCandidateX.append(chip.convert(chip.bounds, to: controller.view).minX)
    }
    XCTAssertEqual(controller.inlineMarkedText.replacingOccurrences(of: "'", with: ""), "nihao")
    XCTAssertEqual(Set(firstCandidateX).count, 1, "the first candidate moved: \(firstCandidateX)")
  }

  func testNineKeyMarksPinyinRatherThanDigits() throws {
    let controller = controller(.nineKey)
    for digit in [6, 4] { try tap({ $0.accessibilityIdentifier == "nineKey\(digit)" }, in: controller) }
    let marked = controller.inlineMarkedText
    XCTAssertFalse(marked.isEmpty)
    XCTAssertFalse(marked.contains { $0.isNumber }, marked)
    let preedit = try XCTUnwrap(try view("preeditButton", in: controller) as? UIButton)
    XCTAssertEqual(preedit.configuration?.title ?? "", "", "候选栏不重复画九键拼写")
    XCTAssertNil(preedit.configuration?.image, "候选栏不显示拼写编辑铅笔")
    XCTAssertTrue(preedit.isHidden, "没有可见读音时不保留空白编辑入口")
  }

  /// 多按一键切不成音节（6442677，末尾两个 7 接不上任何音节）时，写进输入框的仍是一位对一键的字母，不退回数字。
  func testNineKeyMarksLettersWhenAKeyIsMistyped() throws {
    let controller = controller(.nineKey)
    for digit in [6, 4, 4, 2, 6, 7, 7] {
      try tap({ $0.accessibilityIdentifier == "nineKey\(digit)" }, in: controller)
      let marked = controller.inlineMarkedText
      XCTAssertFalse(marked.isEmpty)
      XCTAssertFalse(marked.contains { $0.isNumber }, marked)
    }
    let marked = controller.inlineMarkedText
    XCTAssertEqual(marked.filter { $0 != "'" }.count, 7, marked)
    XCTAssertNotNil(CompositionCaretPolicy.alignment(marked, "6442677"), marked)
  }

  /// 候选按 #10330 立即呈现：每按一个键，候选条上的词块、读音和展开箭头都直接画成新状态，图层上不挂任何动画。
  ///
  /// 被手指按过的词块换字时叠着渐变（标题 0.25s 的 `CATransition` fade）只在系统键盘里的真实触摸之后出现，进程内设 `isHighlighted` 或合成触摸都触发不了，那一项靠模拟器录屏验收（见 2026-10-30-candidate-immediate-presentation）；这里守住按键重画候选条这条路径本身不带动画。
  func testTypingRedrawsTheStripWithoutAnimations() throws {
    let controller = controller(.quanpin)
    func layers(_ layer: CALayer) -> [CALayer] { [layer] + (layer.sublayers ?? []).flatMap { layers($0) } }
    for letter in ["N", "I", "H", "A", "O"] {
      try tap({ $0.accessibilityLabel == "字母 \(letter)" }, in: controller)
      let animated = layers(try view("candidateStrip", in: controller).layer).flatMap { layer in
        (layer.animationKeys() ?? []).map { "\(layer.delegate.map { String(describing: Swift.type(of: $0)) } ?? "\(layer)") \($0)" }
      }
      XCTAssertEqual(animated, [], "按下 \(letter) 后候选条上挂着动画")
    }
  }
}

/// 上屏要替换输入框里的拼音标记，而不是把汉字插在标记后面（#10873）。
///
/// 文档按自绘输入框（Flutter 的 `FlutterTextInputView` 一类）的行为建模：`insertText` 只替换当前选区并清掉标记，不碰标记文本；`setMarkedText` 替换标记文本（没有标记时替换选区）；`unmarkText` 只把标记变成普通文字。系统 `UITextView` 的 `insertText` 会顺带替换标记，所以用它测不出这个问题。
@MainActor
final class InlineCompositionCommitTests: XCTestCase {
  private final class SelectionOnlyInsertDocument: NSObject, UITextDocumentProxy {
    var text = "" as NSString
    var selection = NSRange(location: 0, length: 0)
    var marked: NSRange?
    var calls: [String] = []
    var documentIdentifier = UUID()
    var documentInputMode: UITextInputMode? { nil }
    var hasText: Bool { text.length > 0 }
    var documentContextBeforeInput: String? { text.substring(to: selection.location) }
    var documentContextAfterInput: String? { text.substring(from: NSMaxRange(selection)) }
    var selectedText: String? { selection.length == 0 ? nil : text.substring(with: selection) }

    private func replace(_ range: NSRange, with string: String) {
      text = text.replacingCharacters(in: range, with: string) as NSString
    }

    func insertText(_ string: String) {
      calls.append("insert \(string)")
      replace(selection, with: string)
      selection = NSRange(location: selection.location + (string as NSString).length, length: 0)
      marked = nil
    }

    func setMarkedText(_ string: String, selectedRange: NSRange) {
      calls.append("mark \(string)")
      let range = marked ?? selection
      replace(range, with: string)
      let length = (string as NSString).length
      marked = length == 0 ? nil : NSRange(location: range.location, length: length)
      selection = NSRange(location: range.location + selectedRange.location, length: selectedRange.length)
    }

    func unmarkText() {
      calls.append("unmark")
      marked = nil
    }

    func deleteBackward() {
      marked = nil
      if selection.length > 0 {
        replace(selection, with: "")
      } else if selection.location > 0 {
        replace(NSRange(location: selection.location - 1, length: 1), with: "")
        selection.location -= 1
      }
      selection.length = 0
    }

    func adjustTextPosition(byCharacterOffset offset: Int) {
      selection = NSRange(location: min(max(selection.location + offset, 0), text.length), length: 0)
    }
  }

  private func keyboard(_ document: SelectionOnlyInsertDocument) throws -> KeyboardViewController {
    enableAllInputSchemes()
    let scheme = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = scheme }
    InputSchemePreference.scheme = .quanpin
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
    let keyboard = KeyboardViewController()
    keyboard.loadViewIfNeeded()
    keyboard.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    keyboard.viewWillAppear(false)
    keyboard.view.layoutIfNeeded()
    addTeardownBlock { keyboard.viewDidDisappear(false) }
    return keyboard
  }

  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }

  private func tap(_ predicate: (UIView) -> Bool, in keyboard: KeyboardViewController) throws {
    try XCTUnwrap(descendants(keyboard.view).first(where: predicate) as? UIButton).sendActions(for: .primaryActionTriggered)
    keyboard.view.layoutIfNeeded()
  }

  func testCommitReplacesThePinyinMarkedTextInAHostWhoseInsertTextOnlyReplacesTheSelection() throws {
    let document = SelectionOnlyInsertDocument()
    document.text = "甲丙"
    document.selection = NSRange(location: 1, length: 0)
    let keyboard = try keyboard(document)
    keyboard.render(.init(isHandled: true, preedit: "ni'hao", editingText: "nihao"))
    XCTAssertEqual(document.text as String, "甲ni'hao丙")
    XCTAssertNotNil(document.marked)
    document.calls = []
    keyboard.render(.init(isHandled: true, commitText: "你好"))
    XCTAssertEqual(document.calls, ["mark 你好", "unmark"])
    XCTAssertEqual(document.text as String, "甲你好丙")
    XCTAssertEqual(document.selection, NSRange(location: 3, length: 0))
    XCTAssertNil(document.marked)
  }

  /// 部分选词（同一个快照里上屏一段、还剩组字）仍直接 insertText：「改标记、取消标记、再标记剩余组字」在真实跨进程代理里会让剩余组字盖掉已选汉字（#10284，见 2026-10-31-ios-cantonese-partial-commit）。
  func testPartialCommitKeepsInsertingTheSelectedTextBeforeMarkingTheRest() throws {
    let document = SelectionOnlyInsertDocument()
    let keyboard = try keyboard(document)
    keyboard.render(.init(isHandled: true, preedit: "ni'hao'a", editingText: "nihaoa"))
    document.calls = []
    keyboard.render(.init(isHandled: true, commitText: "你好", preedit: "a", editingText: "a"))
    XCTAssertEqual(document.calls, ["insert 你好", "mark a"])
  }

  func testTypingAndPickingTheFirstCandidateLeavesOnlyTheChineseText() throws {
    let document = SelectionOnlyInsertDocument()
    let keyboard = try keyboard(document)
    for letter in ["N", "I", "H", "A", "O"] {
      try tap({ $0.accessibilityLabel == "字母 \(letter)" }, in: keyboard)
    }
    XCTAssertEqual((document.text as String).replacingOccurrences(of: "'", with: ""), "nihao")
    try tap({ $0.accessibilityIdentifier == "candidate-1" }, in: keyboard)
    let committed = document.text as String
    XCTAssertFalse(committed.isEmpty)
    XCTAssertFalse(committed.contains { $0.isASCII }, "拼音留在了输入框里：\(committed)")
    XCTAssertEqual(document.selection, NSRange(location: document.text.length, length: 0))
    XCTAssertNil(document.marked)
  }
}

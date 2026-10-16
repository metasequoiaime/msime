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

/// 拼写固定写进输入框（拼音分词），候选栏不再画读音：候选从行首开始，按键时不左右移动；九键写进去的是拼音，不是数字键。
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
      XCTAssertTrue(try view("preeditButton", in: controller).isHidden, "the strip draws no reading after \(letter)")
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
    XCTAssertTrue(try view("preeditButton", in: controller).isHidden)
  }
}

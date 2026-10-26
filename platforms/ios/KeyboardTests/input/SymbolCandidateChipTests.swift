import UIKit
import XCTest

/// 符号混输的行（打「立方米」出的「㎥」「m³」，来源是 emoji）永远没有释义，候选条上的词块只按符号本身取宽，不因为释义开着就留出一栏（候选栏的三分之一）。鸿蒙曾经一律留一栏，一屏只剩三个候选、符号后面空出一大段。
@MainActor
final class SymbolCandidateChipTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(descendants) }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }

  func testSymbolRowsStayAsNarrowAsTheirSymbolWithGlossesOn() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    setCandidateGloss(true)
    // 九键的 54 3264 64 是 li fang mi，7464 3264 64 是 ping fang mi。
    for (digits, word, symbols) in [("54326464", "立方米", ["㎥", "m³"]), ("7464326464", "平方米", ["㎡", "m²"])] {
      InputSchemePreference.scheme = .nineKey
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
      controller.viewWillAppear(false)
      for digit in digits { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
      controller.view.layoutIfNeeded()
      let chips = descendants(controller.view).compactMap { $0 as? KeyboardKeyButton }
        .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
      func chip(_ text: String) throws -> KeyboardKeyButton {
        try XCTUnwrap(chips.first { $0.accessibilityLabel?.hasSuffix("：\(text)") == true }, "候选条上没有 \(text)")
      }
      XCTAssertNotNil(try chip(word).constraints.first { $0.identifier == "candidateChipWidth" && $0.isActive },
                      "释义开着时词按释义行取宽")
      let strip = try XCTUnwrap(chips.first?.superview?.superview)
      for symbol in symbols {
        let symbolChip = try chip(symbol)
        XCTAssertNil(symbolChip.constraints.first { $0.identifier == "candidateChipWidth" && $0.isActive },
                     "\(symbol) 没有释义行，不按释义列取宽")
        XCTAssertFalse(symbolChip.configuration?.attributedTitle.map { String($0.characters) }?.contains("\n") ?? false,
                       "\(symbol) 不画释义行")
        XCTAssertLessThan(symbolChip.bounds.width, strip.bounds.width / 6,
                          "\(symbol) 的词块只按符号取宽，不能占去一栏：\(symbolChip.bounds.width) / \(strip.bounds.width)")
      }
      controller.viewDidDisappear(false)
    }
  }
}

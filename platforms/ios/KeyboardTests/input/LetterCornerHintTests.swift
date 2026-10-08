import XCTest
import UIKit

/// 字母键的角标符号（ios-kb-11）：与 Android 共用的符号表和滑动阈值，以及 26 键怎样绘制和输入它们。
@MainActor
final class LetterCornerHintTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func button(_ identifier: String, in controller: UIViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }

  private func key(labelled label: String, in controller: UIViewController) throws -> KeyboardKeyButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == label } as? KeyboardKeyButton,
                  "No key labelled \(label).")
  }

  private func cornerLabel(of key: UIButton) -> UILabel? {
    key.subviews.first { $0.accessibilityIdentifier == "letterCornerHint" } as? UILabel
  }

  private func makeController(_ scheme: ChineseInputScheme) -> KeyboardViewController {
    let previous = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  func testTableMatchesTheDesignRows() {
    let rows = [("qwertyuiop", "1234567890"), ("asdfghjkl", "@#¥%&*()\""), ("zxcvbnm", "~…、?!-/")]
    for (letters, hints) in rows {
      for (letter, hint) in zip(letters, hints) {
        XCTAssertEqual(LetterHintTable.hint(for: String(letter)), String(hint), String(letter))
        XCTAssertEqual(LetterHintTable.hint(for: String(letter).uppercased()), String(hint), "upper-case \(letter)")
      }
    }
    for other in [";", "1", "", "qq", "é", "ㅂ"] {
      XCTAssertNil(LetterHintTable.hint(for: other), other)
    }
  }

  func testSwipeNeedsMoreThanFourteenPointsDown() {
    XCTAssertFalse(SwipeHintPolicy.swiped(downY: 10, currentY: 24), "exactly the threshold is not a swipe")
    XCTAssertTrue(SwipeHintPolicy.swiped(downY: 10, currentY: 24.5))
    XCTAssertFalse(SwipeHintPolicy.swiped(downY: 40, currentY: 10), "moving up is not a swipe down")
  }

  func testQwertyKeysDrawAndCarryTheirCornerSymbol() throws {
    let controller = makeController(.quanpin)
    let q = try key(labelled: "字母 Q", in: controller)
    let l = try key(labelled: "字母 L", in: controller)
    XCTAssertEqual(q.cornerHint, "1")
    XCTAssertEqual(cornerLabel(of: q)?.text, "1")
    XCTAssertEqual(cornerLabel(of: q)?.isHidden, false)
    XCTAssertEqual(l.cornerHint, "\"")
    XCTAssertEqual(cornerLabel(of: q)?.isAccessibilityElement, false, "the key still reads as its letter")

    // 微软双拼的 `;` 键没有自己的角标符号。
    let semicolon = try XCTUnwrap(try button("microsoftFinalKey", in: controller) as? KeyboardKeyButton)
    XCTAssertNil(semicolon.cornerHint)
    XCTAssertEqual(cornerLabel(of: semicolon)?.isHidden, true)
  }

  func testKoreanJamoFacesHaveNoCornerSymbol() throws {
    let controller = makeController(.korean)
    let q = try key(labelled: "字母 ㅂ", in: controller)
    XCTAssertNil(q.cornerHint)
    XCTAssertEqual(cornerLabel(of: q)?.isHidden, true)
  }

  func testCornerSymbolFinishesTheCompositionInsteadOfJoiningIt() throws {
    let controller = makeController(.quanpin)
    for label in ["字母 N", "字母 I"] {
      try key(labelled: label, in: controller).sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    let preedit = try button("preeditButton", in: controller)
    XCTAssertTrue(preedit.configuration?.title?.contains("ni") == true, "ni is composing")

    // 在 q 上下滑、松手时交给控制器的内容。
    let q = try key(labelled: "字母 Q", in: controller)
    let typeHint = try XCTUnwrap(q.onCornerHint)
    typeHint("1")
    controller.view.layoutIfNeeded()
    XCTAssertEqual(preedit.configuration?.title, "水杉输入法",
                   "the composition was committed before the symbol, not extended by it")
  }

  /// 「滑动输入符号」关掉后字母键不再响应下滑，长按仍输入角标（与 Android 一致）；键盘再次出现时读到新值。
  func testSwipeSymbolsSwitchReachesTheLetterKeys() throws {
    let stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.swipeSymbolsKey)
    addTeardownBlock {
      if let stored { KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.swipeSymbolsKey) }
    }
    KeyboardLayoutPreference.swipeSymbols = false
    let controller = makeController(.quanpin)
    let q = try key(labelled: "字母 Q", in: controller)
    XCTAssertFalse(q.swipesCornerHint)
    XCTAssertEqual(q.cornerHint, "1", "the hold still types the corner symbol")
    KeyboardLayoutPreference.swipeSymbols = true
    controller.viewWillAppear(false)
    XCTAssertTrue(q.swipesCornerHint)
  }
}

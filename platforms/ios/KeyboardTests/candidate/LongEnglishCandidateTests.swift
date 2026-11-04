import UIKit
import XCTest

/// 长英文候选不能挤走候选条上的所有备选，完整词形仍由选词和展开列表保留。
@MainActor
final class LongEnglishCandidateTests: XCTestCase {
  private var savedGestures: [Any?] = []

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    let defaults = KeyboardLayoutPreference.defaults
    savedGestures = [KeyboardLayoutPreference.glideTypingKey, KeyboardLayoutPreference.bilingualGlideKey]
      .map { defaults.object(forKey: $0) }
    KeyboardLayoutPreference.glideTyping = true
    KeyboardLayoutPreference.bilingualGlide = true
  }

  override func tearDown() {
    let defaults = KeyboardLayoutPreference.defaults
    for (key, value) in zip([KeyboardLayoutPreference.glideTypingKey, KeyboardLayoutPreference.bilingualGlideKey], savedGestures) {
      if let value { defaults.set(value, forKey: key) }
      else { defaults.removeObject(forKey: key) }
    }
    super.tearDown()
  }

  private func ancestor<T: UIView>(_ type: T.Type, of view: UIView?) -> T? {
    var current = view?.superview
    while let node = current {
      if let match = node as? T { return match }
      current = node.superview
    }
    return nil
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func keyboard(width: CGFloat) -> KeyboardViewController {
    let previous = InputSchemePreference.scheme
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    addTeardownBlock {
      controller.viewDidDisappear(false)
      InputSchemePreference.scheme = previous
    }
    return controller
  }

  private func show(_ words: [String], in controller: KeyboardViewController) {
    let spelling = words[0]
    controller.render(MetasequoiaInputSnapshot(
      isHandled: true, preedit: spelling, reading: spelling,
      candidates: words, candidateCodes: words, candidateSources: words.map { _ in 0 },
      candidatePageCount: 1, editingText: spelling, caretPosition: spelling.count))
    controller.view.layoutIfNeeded()
  }

  private func chip(_ number: Int, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "candidate-\(number)" } as? UIButton)
  }

  func testTwoLongEnglishAlternativesFitWithoutChangingTheirWords() throws {
    for width in [CGFloat(320), 440] {
      let controller = keyboard(width: width)
      XCTAssertEqual(controller.view.bounds.width, width, accuracy: 0.5)
      let words = ["internationalization", "internationalism", "international"]
      show(words, in: controller)
      let first = try chip(1, in: controller)
      let second = try chip(2, in: controller)
      let scroll = try XCTUnwrap(ancestor(UIScrollView.self, of: first))
      XCTAssertGreaterThan(scroll.bounds.width, 0)
      let firstFrame = first.convert(first.bounds, to: scroll)
      let secondFrame = second.convert(second.bounds, to: scroll)
      XCTAssertGreaterThanOrEqual(firstFrame.minX, scroll.bounds.minX - 0.5)
      XCTAssertLessThanOrEqual(secondFrame.maxX, scroll.bounds.maxX + 0.5,
                               "长英文首选之后仍要看得到完整的第二个词块")
      XCTAssertTrue(first.accessibilityLabel?.contains(words[0]) == true)
      XCTAssertTrue(second.accessibilityLabel?.contains(words[1]) == true)
      if let label = first.titleLabel {
        XCTAssertLessThanOrEqual(label.bounds.width, first.bounds.width + 0.5)
        XCTAssertGreaterThanOrEqual(label.frame.minX, -0.5)
      }
    }
  }

  func testReusingAnEnglishChipForChineseRestoresItsNaturalWidth() throws {
    let controller = keyboard(width: 440)
    show(["internationalization", "internationalism"], in: controller)
    let first = try chip(1, in: controller)
    show(["山", "水"], in: controller)
    XCTAssertTrue(first === (try chip(1, in: controller)))
    XCTAssertLessThan(first.bounds.width, 70)
    XCTAssertTrue(first.accessibilityLabel?.contains("山") == true)
    show(["internationalization"], in: controller)
    XCTAssertGreaterThan(first.bounds.width, 120, "只有一个候选时允许显示完整英文")
  }

  func testEnglishPhraseAlternativesAlsoLeaveRoomForASecondChoice() throws {
    for words in [
      ["hello wonderful world", "hello wonderful word"],
      ["你好 wonderful world", "你好 wonderful word"]
    ] {
      let controller = keyboard(width: 320)
      show(words, in: controller)
      let first = try chip(1, in: controller)
      let second = try chip(2, in: controller)
      let scroll = try XCTUnwrap(ancestor(UIScrollView.self, of: first))
      XCTAssertLessThanOrEqual(second.convert(second.bounds, to: scroll).maxX, scroll.bounds.maxX + 0.5)
      XCTAssertTrue(first.accessibilityLabel?.contains(words[0]) == true)
    }
  }

  func testDisablingBilingualGlideKeepsTheExistingCandidateWidths() throws {
    KeyboardLayoutPreference.bilingualGlide = false
    let controller = keyboard(width: 320)
    show(["internationalization", "internationalism"], in: controller)
    let first = try chip(1, in: controller)
    let scroll = try XCTUnwrap(ancestor(UIScrollView.self, of: first))
    XCTAssertGreaterThan(first.bounds.width, scroll.bounds.width / 2)
  }
}

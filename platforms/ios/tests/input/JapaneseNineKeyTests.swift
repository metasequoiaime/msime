import XCTest
import UIKit

@MainActor
final class JapaneseNineKeyTests: XCTestCase {
  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to
  // whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  func testKanaKeysFeedJapaneseEngineCandidates() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    // The candidate the Engine returns is what is being checked. A gloss appends a second line to
    // the chip's title, so leaving it on turns a suffix check into a check of the gloss layout.
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.enabledSchemes = enabled
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = false
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .japaneseNineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.compositionRowHeight)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(nodes(controller.view).compactMap { $0 as? JapaneseNineKeyView }.first)
    XCTAssertFalse(panel.isHidden)
    panel.select(4, direction: 1) // に
    panel.select(5, direction: 4) // ほ
    panel.select(9, direction: 2) // ん
    let first = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "candidate-1" } as? UIButton)
    XCTAssertTrue(first.configuration?.title?.hasSuffix("日本") == true, first.configuration?.title ?? "No candidate")
    let screenshot = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image {
      controller.view.layer.render(in: $0.cgContext)
    })
    screenshot.name = "Japanese nine-key candidates"; screenshot.lifetime = .keepAlways; add(screenshot)
    let picker = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeButton" } as? UIButton)
    picker.sendActions(for: .primaryActionTriggered)
    for name in ["japanese", "japaneseNineKey"] {
      XCTAssertNotNil(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeCard-\(name)" })
    }
    let roman = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeCard-japanese" } as? UIButton)
    roman.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(InputSchemePreference.scheme, .japanese)
    XCTAssertTrue(panel.isHidden)
    controller.view.layoutIfNeeded()
    let letter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 A" } as? UIButton)
    XCTAssertGreaterThan(letter.bounds.height, 40)
    picker.sendActions(for: .primaryActionTriggered)
    let nine = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeCard-japaneseNineKey" } as? UIButton)
    nine.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(InputSchemePreference.scheme, .japaneseNineKey)
    XCTAssertFalse(panel.isHidden)
  }

  func testEveryKanaKeyConvertsAndLayoutsKeepFullHeight() throws {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToJapanese()
    for key in JapaneseNineKeyView.keys {
      for (kana, input) in zip(key.kana, key.strokes) where !input.isEmpty {
        _ = bridge.cancel()
        var snapshot: MetasequoiaInputSnapshot?
        for letter in input { snapshot = bridge.handleCharacter(String(letter)) }
        XCTAssertTrue(snapshot?.candidates.contains(kana) == true, "\(input) → \(kana): \(snapshot?.candidates ?? [])")
      }
    }
    for width in [320.0, 414.0] {
      let panel = JapaneseNineKeyView { title, _, action in
        var config = UIButton.Configuration.plain(); config.title = title
        return UIButton(configuration: config, primaryAction: UIAction { _ in action() })
      }
      panel.frame = CGRect(x: 0, y: 0, width: width, height: 176)
      panel.applyLayout(); panel.layoutIfNeeded()
      let buttons = nodes(panel).compactMap { $0 as? UIButton }
      // Eleven kana keys, the kana modifier and delete. The punctuation row added the eleventh
      // kana key; without side or mode keys the panel builds nothing else.
      XCTAssertEqual(buttons.count, 13)
      for button in buttons {
        XCTAssertGreaterThan(button.bounds.height, 45)
        XCTAssertGreaterThan(button.bounds.width, 44)
        XCTAssertLessThanOrEqual(button.convert(button.bounds, to: panel).maxX, width + 0.5)
      }
    }
  }
  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { nodes($0) } }
}

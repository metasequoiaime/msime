import UIKit
import XCTest

/// 只覆盖选中文字色时，真实候选控件保留原生底色、字重和非首选样式。
@MainActor
final class CandidateSelectedTextTests: XCTestCase {
  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(descendants) }

  private func prepare() {
    enableAllInputSchemes()
    preserveSharedTheme()
    setCandidateGloss(false)
    let defaults = CandidatePalette.defaults
    let follows = defaults.object(forKey: CandidatePalette.followsDesktopKey)
    let scheme = InputSchemePreference.scheme
    addTeardownBlock {
      defaults.set(follows, forKey: CandidatePalette.followsDesktopKey)
      InputSchemePreference.scheme = scheme
    }
    defaults.set(true, forKey: CandidatePalette.followsDesktopKey)
    InputSchemePreference.scheme = .nineKey
  }

  private func setColor(_ color: String?) {
    XCTAssertTrue(GlobalThemePreference.update {
      $0["global_theme"] = "custom"
      var custom: [String: Any] = ["base": "system"]
      if let color { custom["candidate_colors"] = ["selected_text": color] }
      $0["custom_theme"] = custom
    })
  }

  private func strip(_ color: String?) throws -> [UIButton.Configuration] {
    setColor(color)
    let controller = KeyboardViewController()
    controller.overrideUserInterfaceStyle = .light
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    defer { controller.viewDidDisappear(false) }
    RunLoop.main.run(until: Date().addingTimeInterval(0.6))
    for digit in "64" {
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "nineKey\(digit)" } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    return try (1...2).map { number in
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidate-\(number)" } as? UIButton)
      return try XCTUnwrap(key.configuration)
    }
  }

  private func font(_ configuration: UIButton.Configuration) throws -> UIFont {
    if let transformer = configuration.titleTextAttributesTransformer {
      return try XCTUnwrap(transformer(AttributeContainer()).font)
    }
    let title = NSAttributedString(try XCTUnwrap(configuration.attributedTitle))
    return try XCTUnwrap(title.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
  }

  private func sameSurface(_ actual: UIButton.Configuration, _ original: UIButton.Configuration) {
    XCTAssertEqual(actual.background.backgroundColor, original.background.backgroundColor, "选中文字色不能改变底色")
    XCTAssertEqual(actual.background.cornerRadius, original.background.cornerRadius)
    XCTAssertEqual(actual.background.strokeWidth, original.background.strokeWidth)
  }

  func testStripSelectedTextKeepsNativeSurfaceAndFont() throws {
    prepare()
    let original = try strip(nil)
    let picked = try strip("#FF3B30")
    XCTAssertEqual(picked[0].baseForegroundColor, ThemeColor.parse("#FF3B30"))
    for index in 0...1 {
      sameSurface(picked[index], original[index])
      XCTAssertEqual(try font(picked[index]).fontDescriptor, try font(original[index]).fontDescriptor, "文字色不能改变字重")
    }
    XCTAssertEqual(picked[1].baseForegroundColor, original[1].baseForegroundColor)
    let cleared = try strip(nil)
    sameSurface(cleared[0], original[0])
    XCTAssertEqual(cleared[0].baseForegroundColor, original[0].baseForegroundColor)
  }

  func testExpandedPanelSelectedTextKeepsNativeSurfaces() throws {
    prepare()
    setColor(nil)
    func chips(_ palette: CandidatePalette?) throws -> [UIButton.Configuration] {
      let panel = KeyboardCandidatePanelView(candidates: ["合成甲", "合成乙"], preedit: "", showsHeader: false,
        palette: palette, display: { $0 }, onSelect: { _ in }, onClose: {})
      let controller = UIViewController()
      controller.view = panel
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 300))
      window.rootViewController = controller
      window.isHidden = false
      defer { window.isHidden = true }
      panel.frame = window.bounds
      panel.layoutIfNeeded()
      return try (1...2).map { number in
        let key = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-\(number)" } as? UIButton)
        return try XCTUnwrap(key.configuration)
      }
    }
    let original = try chips(nil)
    let palette = CandidatePalette.resolve(["global_theme": "custom",
      "custom_theme": ["base": "system", "candidate_colors": ["selected_text": "#FF3B30"]]], systemDark: false)
    let picked = try chips(palette)
    XCTAssertEqual(picked[0].baseForegroundColor, ThemeColor.parse("#FF3B30"))
    for index in 0...1 { sameSurface(picked[index], original[index]) }
    XCTAssertEqual(picked[1].baseForegroundColor, original[1].baseForegroundColor)
  }
}

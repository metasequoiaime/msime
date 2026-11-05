import UIKit
import XCTest

/// 底部双拼提示在零尺寸、换宽和显示状态变化后仍沿用单行 UILabel 的布局与缩字。
@MainActor
final class KeyboardBottomHintTests: XCTestCase {
  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func keyboard() -> KeyboardViewController {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    addTeardownBlock {
      InputSchemePreference.enabledSchemes = enabled
      InputSchemePreference.scheme = previous
    }
    enableAllInputSchemes()
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func bottomHint(in key: UIButton) throws -> UILabel {
    try XCTUnwrap(key.subviews.compactMap { $0 as? UILabel }.first {
      $0 !== key.titleLabel && $0.font.pointSize == 9
    })
  }

  private func hintKey(size: CGSize, text: String?) -> (KeyboardKeyButton, KeyboardBottomHintLabel) {
    let key = KeyboardKeyButton(frame: CGRect(origin: .zero, size: size))
    let hint = KeyboardBottomHintLabel()
    key.addSubview(hint)
    key.bottomHintLabel = hint
    hint.text = text
    key.layoutIfNeeded()
    return (key, hint)
  }

  func testHintTracksKeyWidthAndKeepsTheOriginalBottomAndSideInsets() {
    let (key, hint) = hintKey(size: CGSize(width: 36, height: 44), text: "iu")
    for size in [CGSize(width: 36, height: 44), CGSize(width: 70, height: 52), CGSize(width: 24, height: 30)] {
      key.bounds.size = size
      key.layoutIfNeeded()
      XCTAssertEqual(hint.frame.minX, 2, accuracy: 0.001)
      XCTAssertEqual(hint.frame.maxX, size.width - 2, accuracy: 0.001)
      XCTAssertEqual(hint.frame.maxY, size.height - 2, accuracy: 0.001)
      XCTAssertGreaterThan(hint.bounds.height, 0)
      XCTAssertLessThanOrEqual(hint.bounds.height, size.height - 2)
    }
  }

  func testZeroAndTinyKeysRecoverWhenTheyGrow() {
    let (key, hint) = hintKey(size: .zero, text: "iang uang")
    for size in [CGSize.zero, CGSize(width: 3, height: 1), CGSize(width: 36, height: 44), .zero,
                 CGSize(width: 60, height: 50)] {
      key.bounds.size = size
      key.layoutIfNeeded()
      XCTAssertGreaterThanOrEqual(hint.bounds.width, 0)
      XCTAssertGreaterThanOrEqual(hint.bounds.height, 0)
      if size.width <= 4 || size.height <= 2 { XCTAssertEqual(hint.frame, .zero) }
      else {
        XCTAssertEqual(hint.bounds.width, size.width - 4, accuracy: 0.001)
        XCTAssertEqual(hint.frame.maxY, size.height - 2, accuracy: 0.001)
      }
    }
  }

  func testHiddenHintCanReceiveTextAndBecomeVisibleWithoutChangingTheKeySize() {
    let (key, hint) = hintKey(size: CGSize(width: 36, height: 44), text: nil)
    XCTAssertTrue(hint.isHidden)
    hint.text = "iang uang"
    hint.isHidden = false
    key.layoutIfNeeded()
    XCTAssertFalse(hint.isHidden)
    XCTAssertGreaterThan(hint.bounds.height, 0)
    XCTAssertEqual(hint.frame.maxY, 42, accuracy: 0.001)
    hint.isHidden = true
    hint.text = nil
    key.layoutIfNeeded()
    hint.text = "iu"
    hint.isHidden = false
    key.layoutIfNeeded()
    XCTAssertGreaterThan(hint.bounds.height, 0)
    XCTAssertEqual(key.bounds.size, CGSize(width: 36, height: 44))
  }

  func testLongHintKeepsUILabelRenderingAndMinimumScale() {
    let (_, hint) = hintKey(size: CGSize(width: 36, height: 44), text: "iang uang")
    hint.isHidden = false
    let reference = UILabel(frame: hint.bounds)
    reference.font = .systemFont(ofSize: 9, weight: .regular)
    reference.textColor = hint.textColor
    reference.textAlignment = .center
    reference.numberOfLines = 1
    reference.adjustsFontSizeToFitWidth = true
    reference.minimumScaleFactor = 0.6
    reference.text = hint.text
    XCTAssertGreaterThan((reference.text! as NSString).size(withAttributes: [.font: reference.font!]).width,
                         reference.bounds.width)
    XCTAssertTrue(hint.adjustsFontSizeToFitWidth)
    XCTAssertEqual(hint.minimumScaleFactor, 0.6, accuracy: 0.001)
    XCTAssertEqual(hint.numberOfLines, 1)
    func render(_ label: UILabel) -> Data? {
      UIGraphicsImageRenderer(bounds: label.bounds).image { _ in label.drawText(in: label.bounds) }.pngData()
    }
    let rendered = render(hint)
    XCTAssertEqual(rendered, render(reference))
    hint.text = nil
    XCTAssertNotEqual(rendered, render(hint), "渲染对比必须包含真实字形，不能让两张空图误过")
  }

  func testHintAppearanceWithShortLongAndHiddenText() throws {
    let controller = keyboard()
    for (name, text) in [("Q", "iu"), ("W", "iang uang"), ("E", ""), ("R", "uan er")] {
      let key = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 \(name)" } as? UIButton)
      let hint = try bottomHint(in: key)
      hint.text = text
      hint.isHidden = text.isEmpty
      if var configuration = key.configuration {
        configuration.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 0, bottom: text.isEmpty ? 0 : 11, trailing: 0)
        key.configuration = configuration
      }
    }
    controller.view.layoutIfNeeded()
    for name in ["Q", "W", "R"] {
      let key = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 \(name)" } as? UIButton)
      let hint = try bottomHint(in: key)
      XCTAssertFalse(hint.isHidden)
      XCTAssertGreaterThan(hint.bounds.height, 0)
      XCTAssertEqual(hint.frame.minX, 2, accuracy: 0.001)
      XCTAssertEqual(hint.frame.maxX, key.bounds.width - 2, accuracy: 0.001)
      XCTAssertEqual(hint.frame.maxY, key.bounds.height - 2, accuracy: 0.001)
    }
    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image {
      controller.view.layer.render(in: $0.cgContext)
    })
    attachment.name = "底部提示-短文本-长文本-隐藏"
    attachment.lifetime = .keepAlways
    add(attachment)
  }
}

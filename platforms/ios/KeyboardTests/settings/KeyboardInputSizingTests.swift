import UIKit
import XCTest

/// 系统按外层输入视图协商尺寸，不能由内容视图的首轮布局反推键盘高度。
@MainActor
final class KeyboardInputSizingTests: XCTestCase {
  func testTheFirstHostMeasurementUsesTheKeyboardHeightBeforePresentation() throws {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let input = try XCTUnwrap(controller.inputView)
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant

    for proposedHeight in [CGFloat(0), 216, 932] {
      let measured = input.systemLayoutSizeFitting(CGSize(width: 390, height: proposedHeight))
      XCTAssertEqual(measured, CGSize(width: 390, height: height))
    }
  }

  func testHeightChangesAreMeasuredWithoutMovingTheContentIntoTheContainerLayout() {
    let content = KeyboardInputView()
    let input = KeyboardInputContainerView(contentView: content)
    for height in [CGFloat(296), 344, 224, 296] {
      input.updatePreferredHeight(height)
      let proposal = CGSize(width: 430, height: 932)
      XCTAssertEqual(input.systemLayoutSizeFitting(proposal), CGSize(width: 430, height: height))
      let fitted = input.systemLayoutSizeFitting(
        proposal, withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
      XCTAssertEqual(fitted.height, height, accuracy: 0.5)
      input.frame = CGRect(origin: .zero, size: CGSize(width: 430, height: height))
      input.layoutIfNeeded()
      XCTAssertEqual(content.frame, input.bounds)
    }
  }

  func testRequiredHostHeightWinsAndInvalidUpdatesPreserveTheKeyboardHeight() {
    let input = KeyboardInputContainerView(contentView: KeyboardInputView())
    input.updatePreferredHeight(296)
    let proposal = CGSize(width: 390, height: 224)
    XCTAssertEqual(
      input.systemLayoutSizeFitting(
        proposal, withHorizontalFittingPriority: .required, verticalFittingPriority: .required),
      proposal)

    for invalidHeight in [CGFloat.zero, -1, .nan, .infinity] {
      input.updatePreferredHeight(invalidHeight)
      XCTAssertEqual(input.systemLayoutSizeFitting(proposal), CGSize(width: 390, height: 296))
    }
  }
}

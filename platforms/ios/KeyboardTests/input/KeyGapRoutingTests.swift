import UIKit
import XCTest

/// 落在键距、行距里的触摸归到最近的键，而不是被 stack view 接住丢掉；候选栏不受影响。
@MainActor
final class KeyGapRoutingTests: XCTestCase {
  private var savedLayout: [String: Any] = [:]
  private let layoutKeys = [KeyboardLayoutPreference.keySpacingKey, KeyboardLayoutPreference.rowSpacingKey]

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    // 别的用例改过的键距会留在 App Group 里；这里按默认间距量空隙。
    savedLayout = [:]
    for key in layoutKeys {
      savedLayout[key] = KeyboardLayoutPreference.defaults.object(forKey: key)
      KeyboardLayoutPreference.defaults.removeObject(forKey: key)
    }
  }

  override func tearDown() {
    for key in layoutKeys {
      if let value = savedLayout[key] { KeyboardLayoutPreference.defaults.set(value, forKey: key) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: key) }
    }
    super.tearDown()
  }

  // MARK: - 纯几何

  func testNearestPicksTheCloserKeyOnEachSideOfAGap() {
    let left = CGRect(x: 0, y: 0, width: 30, height: 40)
    let right = CGRect(x: 36, y: 0, width: 30, height: 40)
    let reach = KeyGapRouting.reach(spacing: 6, axis: .horizontal, margins: nil)
    XCTAssertEqual(reach, 4)
    XCTAssertEqual(KeyGapRouting.nearest(to: CGPoint(x: 31, y: 20), in: [left, right], reach: reach), 0)
    XCTAssertEqual(KeyGapRouting.nearest(to: CGPoint(x: 35, y: 20), in: [left, right], reach: reach), 1)
    // 正中间一样近，取靠前的那个，结果是确定的。
    XCTAssertEqual(KeyGapRouting.nearest(to: CGPoint(x: 33, y: 20), in: [left, right], reach: reach), 0)
  }

  func testNearestLeavesPointsBeyondReachAlone() {
    let key = CGRect(x: 0, y: 0, width: 30, height: 40)
    XCTAssertNil(KeyGapRouting.nearest(to: CGPoint(x: 35, y: 20), in: [key], reach: 4))
    XCTAssertEqual(KeyGapRouting.nearest(to: CGPoint(x: 34, y: 20), in: [key], reach: 4), 0)
    // 斜角按真实距离算：(3, 3) 离角点约 4.24，超出 4。
    XCTAssertNil(KeyGapRouting.nearest(to: CGPoint(x: 33, y: 43), in: [key], reach: 4))
    XCTAssertNil(KeyGapRouting.nearest(to: .zero, in: [.null, .zero], reach: 10), "空矩形不参与")
  }

  func testReachCoversHalfTheSpacingAndTheWholeMargin() {
    XCTAssertEqual(KeyGapRouting.reach(spacing: 7, axis: .vertical, margins: nil), 4.5)
    // 居中字母行两侧 19pt 的空白整段归最外侧的键。
    let margins = UIEdgeInsets(top: 0, left: 19, bottom: 0, right: 19)
    XCTAssertEqual(KeyGapRouting.reach(spacing: 6, axis: .horizontal, margins: margins), 20)
    XCTAssertEqual(KeyGapRouting.reach(spacing: 6, axis: .vertical, margins: margins), 4)
  }

  // MARK: - 真实键盘

  private func controller(_ scheme: ChineseInputScheme) -> KeyboardViewController {
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func key(_ label: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(
      descendants(controller.view).first { ($0.accessibilityLabel == label || $0.accessibilityIdentifier == label) && !$0.isHidden }
        as? UIButton,
      "no key \(label)")
  }

  private func frame(_ view: UIView, in controller: KeyboardViewController) -> CGRect {
    view.convert(view.bounds, to: controller.view)
  }

  private func hit(_ point: CGPoint, in controller: KeyboardViewController) -> UIView? {
    controller.view.hitTest(point, with: nil)
  }

  func testLetterGapsGoToTheNearestKey() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let controller = self.controller(.quanpin)
    let q = try key("字母 Q", in: controller), w = try key("字母 W", in: controller)
    let a = try key("字母 A", in: controller), s = try key("字母 S", in: controller)
    let qFrame = frame(q, in: controller), wFrame = frame(w, in: controller), sFrame = frame(s, in: controller)
    XCTAssertGreaterThan(wFrame.minX - qFrame.maxX, 2, "Q 与 W 之间要有空隙才测得到")
    XCTAssertGreaterThan(sFrame.minY - wFrame.maxY, 2, "两行之间要有空隙才测得到")

    // 改动前这些点命中的是行或根 stack view 本身，触摸被丢掉。
    let between = (qFrame.maxX + wFrame.minX) / 2
    XCTAssertTrue(hit(CGPoint(x: between - 1, y: qFrame.midY), in: controller) === q)
    XCTAssertTrue(hit(CGPoint(x: between + 1, y: qFrame.midY), in: controller) === w)
    let rowGap = (wFrame.maxY + sFrame.minY) / 2
    XCTAssertTrue(hit(CGPoint(x: wFrame.midX, y: rowGap - 1), in: controller) === w)
    XCTAssertTrue(hit(CGPoint(x: sFrame.midX, y: rowGap + 1), in: controller) === s)
    // 键内照旧由 UIKit 自己命中。
    XCTAssertTrue(hit(CGPoint(x: qFrame.midX, y: qFrame.midY), in: controller) === q)

    // 第二行两侧的边距归 A：居中字母行默认留出的半个键宽不再是死区。
    let aFrame = frame(a, in: controller)
    if aFrame.minX > 8 {
      XCTAssertTrue(hit(CGPoint(x: aFrame.minX - 3, y: aFrame.midY), in: controller) === a)
    }
  }

  func testAPressedKeyKeepsItsLayoutArea() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let controller = self.controller(.quanpin)
    let w = try key("字母 W", in: controller)
    let wFrame = frame(w, in: controller)
    // 按下动画把键缩到 0.94，UIKit 的默认命中随之缩小；边缘这一圈仍要算这个键的。
    w.transform = CGAffineTransform(scaleX: 0.94, y: 0.94)
    defer { w.transform = .identity }
    XCTAssertTrue(hit(CGPoint(x: wFrame.minX + 0.5, y: wFrame.midY), in: controller) === w)
    XCTAssertTrue(hit(CGPoint(x: wFrame.maxX - 0.5, y: wFrame.midY), in: controller) === w)
  }

  func testNineKeyGapsGoToTheNearestKey() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let controller = self.controller(.nineKey)
    let two = try key("nineKey2", in: controller), three = try key("nineKey3", in: controller)
    let five = try key("nineKey5", in: controller), delete = try key("nineKeyDelete", in: controller)
    let twoFrame = frame(two, in: controller), threeFrame = frame(three, in: controller)
    let fiveFrame = frame(five, in: controller), deleteFrame = frame(delete, in: controller)

    let between = (twoFrame.maxX + threeFrame.minX) / 2
    XCTAssertTrue(hit(CGPoint(x: between - 1, y: twoFrame.midY), in: controller) === two)
    XCTAssertTrue(hit(CGPoint(x: between + 1, y: twoFrame.midY), in: controller) === three)
    let rowGap = (twoFrame.maxY + fiveFrame.minY) / 2
    XCTAssertTrue(hit(CGPoint(x: fiveFrame.midX, y: rowGap + 1), in: controller) === five)
    // 网格与右侧删除列之间的空隙同样归最近的键。
    let columnGap = (threeFrame.maxX + deleteFrame.minX) / 2
    XCTAssertTrue(hit(CGPoint(x: columnGap + 1, y: deleteFrame.midY), in: controller) === delete)
  }

  func testCandidateStripIsNotACatchment() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let controller = self.controller(.quanpin)
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    let stripFrame = frame(strip, in: controller)
    let q = try key("字母 Q", in: controller)
    let qFrame = frame(q, in: controller)
    // 候选栏里面的点仍然命中候选栏自己的视图。
    let inside = try XCTUnwrap(hit(CGPoint(x: stripFrame.midX, y: stripFrame.midY), in: controller))
    XCTAssertTrue(inside.isDescendant(of: strip))
    // 紧贴候选栏下沿的点离第一行比离候选栏远，不交给第一行的键。
    let justBelowStrip = hit(CGPoint(x: qFrame.midX, y: stripFrame.maxY + 0.5), in: controller)
    XCTAssertFalse(justBelowStrip === q)
  }

  func testNineKeyHoldWaitsHalfASecond() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let controller = self.controller(.nineKey)
    for digit in 2...9 {
      let button = try key("nineKey\(digit)", in: controller)
      let hold = try XCTUnwrap(button.gestureRecognizers?.compactMap { $0 as? UILongPressGestureRecognizer }.first)
      XCTAssertEqual(hold.minimumPressDuration, KeyboardViewController.nineKeyHoldDuration)
    }
    XCTAssertEqual(KeyboardViewController.nineKeyHoldDuration, 0.5)
  }
}

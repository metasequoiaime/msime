import UIKit
import XCTest

@MainActor
final class CandidateStripRowTests: XCTestCase {
  func testChangingWidthsAndVisibilityKeepsContentGeometry() {
    let row = CandidateStripRow()
    row.stack.axis = .horizontal
    row.stack.spacing = 11
    row.stack.translatesAutoresizingMaskIntoConstraints = false
    let viewport = UIScrollView(frame: CGRect(x: 0, y: 0, width: 390, height: 42))
    viewport.addSubview(row.stack)
    NSLayoutConstraint.activate([
      row.stack.leadingAnchor.constraint(equalTo: viewport.contentLayoutGuide.leadingAnchor),
      row.stack.trailingAnchor.constraint(equalTo: viewport.contentLayoutGuide.trailingAnchor),
      row.stack.topAnchor.constraint(equalTo: viewport.contentLayoutGuide.topAnchor),
      row.stack.bottomAnchor.constraint(equalTo: viewport.contentLayoutGuide.bottomAnchor),
      row.stack.heightAnchor.constraint(equalTo: viewport.frameLayoutGuide.heightAnchor),
    ])
    var widths: [NSLayoutConstraint] = []
    for index in 0..<9 {
      let button = KeyboardKeyButton()
      var configuration = UIButton.Configuration.plain()
      configuration.title = "合成\n条目"
      button.configuration = configuration
      button.titleLineCount = 2
      button.accessibilityIdentifier = "candidate-\(index + 1)"
      let width = button.widthAnchor.constraint(equalToConstant: 100)
      width.priority = .init(999)
      width.isActive = true
      widths.append(width)
      row.add(button)
    }
    for update in 0..<180 {
      let visible = update % 10
      for index in row.buttons.indices {
        widths[index].constant = CGFloat(90 + (update + index) % 80)
        row.buttons[index].configuration?.title = update % 2 == 0 ? "合成\n条目" : "示例\n注释"
        row.setHidden(index >= visible, at: index)
      }
      viewport.setNeedsLayout()
      row.stack.setNeedsLayout()
      viewport.layoutIfNeeded()
      var previousEdge: CGFloat?
      for index in 0..<visible {
        let frame = row.buttons[index].convert(row.buttons[index].bounds, to: viewport)
        XCTAssertEqual(frame.width, widths[index].constant, accuracy: 1)
        XCTAssertEqual(frame.height, 42, accuracy: 1)
        if index == 0 {
          let target = viewport.hitTest(CGPoint(x: frame.midX, y: frame.midY), with: nil)
          XCTAssertTrue(target === row.buttons[index] || target?.isDescendant(of: row.buttons[index]) == true,
                        "包装层不能拦截候选按钮的触摸")
        }
        if let previousEdge { XCTAssertEqual(frame.minX - previousEdge, 11, accuracy: 1) }
        previousEdge = frame.maxX
      }
      XCTAssertEqual(viewport.contentSize.width, previousEdge ?? 0, accuracy: 1)
      XCTAssertTrue(row.buttons.dropFirst(visible).allSatisfy(\.isHidden))
    }
  }
}

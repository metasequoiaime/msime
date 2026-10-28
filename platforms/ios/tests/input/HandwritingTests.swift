import XCTest
import UIKit

@MainActor
final class HandwritingTests: XCTestCase {
  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to
  // whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  // Pen trajectories for 中国, not text rendered using a font.
  private var chineseInk: [[CGPoint]] {
    let points: [[(Double, Double)]] = [
      [(35, 40), (35, 105)], [(35, 40), (125, 40), (125, 105)], [(35, 105), (125, 105)], [(80, 15), (80, 140)],
      [(175, 20), (175, 140)], [(175, 20), (280, 20), (280, 140)], [(175, 140), (280, 140)],
      [(192, 45), (261, 45)], [(198, 78), (257, 78)], [(226, 45), (226, 112)], [(190, 112), (264, 112)], [(247, 91), (256, 101)],
    ]
    return points.map { $0.map { CGPoint(x: $0.0, y: $0.1) } }
  }
  func testRealChineseInkRecognition() async throws {
    let canvas = HandwritingCanvas(frame: CGRect(x: 0, y: 0, width: 320, height: 155))
    canvas.setTestStrokes(chineseInk)
    let recognizer = HandwritingRecognizer()
    try await recognizer.download { _ in }
    let words = try await recognizer.recognize(Array(chineseInk.prefix(4)), width: 160, height: 155)
    XCTAssertEqual(words.first, "中", "Actual candidates: \(words)")
  }
  func testCommonCharactersFromPenTrajectories() async throws {
    let recognizer = HandwritingRecognizer()
    try await recognizer.download { _ in }
    let examples: [(String, [[(Double, Double)]])] = [
      ("人", [[(80,20),(75,55),(60,95),(30,140)],[(73,65),(90,100),(130,140)]]),
      ("大", [[(25,65),(135,65)],[(80,20),(75,70),(60,110),(25,145)],[(78,70),(95,110),(140,145)]]),
      ("木", [[(25,60),(135,60)],[(80,15),(80,145)],[(76,65),(55,95),(20,125)],[(85,70),(105,100),(140,125)]]),
      ("水", [[(80,15),(80,135),(73,145),(58,135)],[(20,65),(55,65),(45,90),(18,120)],[(130,40),(95,75)],[(85,60),(103,100),(140,130)]]),
      ("天", [[(35,30),(125,30)],[(20,65),(140,65)],[(80,30),(75,80),(55,120),(20,145)],[(78,80),(100,120),(140,145)]]),
      ("日", [[(40,20),(40,140)],[(40,20),(120,20),(120,140)],[(40,80),(120,80)],[(40,140),(120,140)]]),
    ]
    for (expected, strokes) in examples {
      let ink = strokes.map { $0.map { CGPoint(x: $0.0, y: $0.1) } }
      let words = try await recognizer.recognize(ink, width: 160, height: 160)
      print("Handwriting \(expected): \(words)")
      XCTAssertEqual(words.first, expected, "Actual candidates for \(expected): \(words)")
    }
  }
  func testCandidateSelectionInsertsOnlyAfterConfirmation() async throws {
    try await HandwritingRecognizer().download { _ in }
    let panel = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 414, height: 160))
    panel.layoutIfNeeded()
    var inserted = ""
    panel.onInsert = { inserted += $0 }
    panel.canvas.setTestStrokes(chineseInk.prefix(4).map { $0.map { CGPoint(x: $0.x * 0.6 + 5, y: $0.y * 0.6 + 5) } })
    for _ in 0..<100 {
      if !panel.results.isEmpty { break }
      try await Task.sleep(nanoseconds: 50_000_000)
    }
    XCTAssertTrue(panel.results.contains("中"))
    XCTAssertEqual(inserted, "")
    let candidate = try XCTUnwrap(nodes(panel).first { ($0 as? UIButton)?.title(for: .normal) == "中" } as? UIButton)
    candidate.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(inserted, "中")
    XCTAssertFalse(panel.hasInk)
    XCTAssertTrue(panel.results.isEmpty)
    candidate.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(inserted, "中", "A stale candidate must not insert again")
  }

  func testClearInvalidatesPendingRecognitionAndUndoRemovesOneStroke() async throws {
    let panel = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 414, height: 160))
    panel.layoutIfNeeded()
    panel.canvas.setTestStrokes(chineseInk)
    panel.canvas.undo()
    XCTAssertEqual(panel.canvas.strokes.count, chineseInk.count - 1)
    panel.clear()
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertFalse(panel.hasInk)
    XCTAssertTrue(panel.results.isEmpty)
    XCTAssertTrue(panel.canvas.strokes.isEmpty)
  }
  /// 书写时画布只重画新线段周围的一小块（`extendStroke` 返回的区域），一笔写完后的画面必须和整块重画一模一样，包括卡片底边的投影和穿过已有笔画的地方。
  func testIncrementalInkRedrawMatchesFullRedraw() throws {
    let size = CGSize(width: 320, height: 155)
    let format = UIGraphicsImageRendererFormat(); format.scale = 2; format.opaque = false
    let renderer = UIGraphicsImageRenderer(size: size, format: format)
    let canvas = HandwritingCanvas(frame: CGRect(origin: .zero, size: size))
    canvas.inkWidth = 6
    canvas.setTestStrokes(chineseInk + [[CGPoint(x: 20, y: 20)]])
    // 斜穿已有笔画，再贴着底边写出界（被夹回边上），每 2pt 一个采样点，和手指移动时的触摸事件相当。
    let path = (0...140).map { CGPoint(x: 20 + Double($0) * 2, y: 20 + Double($0) * 0.9) }
      + (0...20).map { CGPoint(x: 300 - Double($0) * 2, y: 150 + Double($0)) }
    var dirtyRects: [CGRect] = []
    let incremental = renderer.image { context in
      // UIKit 每次 draw(_:) 都给一个新的上下文；这里共用一个，所以每次都存取一下状态，免得笔迹的圆头线帽漏到下一次画的虚线辅助线上。
      context.cgContext.saveGState(); canvas.draw(canvas.bounds); context.cgContext.restoreGState()
      for point in path {
        guard let dirty = canvas.extendStroke([point]) else { continue }
        dirtyRects.append(dirty)
        // 和 UIKit 处理 setNeedsDisplay(_:) 一样：区域按像素向外取整，先清空再在裁剪下重画，区域外保留上一次的结果。
        let area = dirty.integral
        context.cgContext.saveGState()
        context.cgContext.clip(to: area)
        context.cgContext.clear(area)
        canvas.draw(area)
        context.cgContext.restoreGState()
      }
    }
    XCTAssertNil(canvas.extendStroke([canvas.strokes.last!.last!]), "不到 1pt 的移动不接点，也不重画")
    XCTAssertGreaterThan(dirtyRects.count, 100)
    for dirty in dirtyRects { XCTAssertLessThan(dirty.width * dirty.height, size.width * size.height / 50, "\(dirty)") }
    let reference = HandwritingCanvas(frame: CGRect(origin: .zero, size: size))
    reference.inkWidth = canvas.inkWidth
    reference.setTestStrokes(canvas.strokes)
    let full = renderer.image { _ in reference.draw(reference.bounds) }
    let actual = try pixels(incremental), expected = try pixels(full)
    XCTAssertEqual(actual.count, expected.count)
    // 同一笔描得更长以后，少数抗锯齿边缘像素会差一个舍入单位；再大就是新线段没画全或旧像素被弄脏了。
    let worst = zip(actual, expected).map { abs(Int($0) - Int($1)) }.max() ?? 0
    XCTAssertLessThanOrEqual(worst, 2)
    if worst > 2 {
      for (name, image) in [("incremental", incremental), ("full", full)] {
        let shot = XCTAttachment(image: image); shot.name = name; shot.lifetime = .keepAlways; add(shot)
      }
    }
  }
  private func pixels(_ image: UIImage) throws -> [UInt8] {
    let data = try XCTUnwrap(image.cgImage?.dataProvider?.data) as Data
    return [UInt8](data)
  }
  func testHandwritingSchemeKeepsToolbarAndSwitchesBackToLetters() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    let controller = KeyboardViewController(); controller.loadViewIfNeeded()
    for width in [320.0, 414.0] {
      let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
      XCTAssertEqual(height.constant, 360 + KeyboardViewController.compositionRowHeight)
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: height.constant); controller.view.layoutIfNeeded()
      let panel = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" } as? HandwritingInputView)
      XCTAssertFalse(panel.isHidden)
      XCTAssertGreaterThanOrEqual(panel.canvas.bounds.height, 200)
      XCTAssertGreaterThan(panel.canvas.bounds.width, 200)
      let shot = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { controller.view.layer.render(in: $0.cgContext) }); shot.name = "Handwriting keyboard \(Int(width))"; shot.lifetime = .keepAlways; add(shot)
    }
    let language = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "bottomLanguageKey" } as? UIButton)
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, 260 + KeyboardViewController.compositionRowHeight)
    XCTAssertTrue(try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" }).isHidden)
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, 360 + KeyboardViewController.compositionRowHeight)
    XCTAssertFalse(try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" }).isHidden)
  }
  func testHandwritingHeightTracksOrientationAndSymbolMode() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    let parent = UIViewController()
    let controller = KeyboardViewController()
    parent.addChild(controller)
    controller.loadViewIfNeeded()
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let toggle = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "layoutToggleButton" } as? UIButton)
    let panel = try XCTUnwrap(nodes(controller.view).first { $0 is HandwritingInputView } as? HandwritingInputView)
    let enter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "returnKey" })
    for (verticalSize, width, writingHeight, typingHeight) in [
      (UIUserInterfaceSizeClass.regular, 414.0, 360.0 + KeyboardViewController.compositionRowHeight, 260.0 + KeyboardViewController.compositionRowHeight),
      (.compact, 812.0, 260.0 + KeyboardViewController.compositionRowHeight, 216.0 + KeyboardViewController.compositionRowHeight),
      (.regular, 320.0, 360.0 + KeyboardViewController.compositionRowHeight, 260.0 + KeyboardViewController.compositionRowHeight),
    ] {
      parent.setOverrideTraitCollection(UITraitCollection(verticalSizeClass: verticalSize), forChild: controller)
      controller.viewDidLayoutSubviews()
      XCTAssertEqual(height.constant, writingHeight)
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: writingHeight)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(enter.bounds.height, 44, accuracy: 0.5)
      XCTAssertGreaterThanOrEqual(panel.canvas.bounds.height, verticalSize == .compact ? 110 : 200)
      toggle.sendActions(for: .primaryActionTriggered)
      XCTAssertTrue(panel.isHidden)
      XCTAssertEqual(height.constant, typingHeight)
      controller.view.frame.size.height = typingHeight
      controller.view.layoutIfNeeded()
      toggle.sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(panel.isHidden)
      XCTAssertEqual(height.constant, writingHeight)
    }
  }
  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }
}

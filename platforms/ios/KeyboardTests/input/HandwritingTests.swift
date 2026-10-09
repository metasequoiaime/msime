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

  func testDownloadCompletionFromAnInactiveGenerationIsIgnored() {
    var gate = HandwritingDownloadGate()
    gate.setActive(true)
    let token = gate.beginDownload()
    gate.setActive(false)
    XCTAssertFalse(gate.accepts(token))
    gate.setActive(true)
    XCTAssertFalse(gate.accepts(token))
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
  #if canImport(MLKitDigitalInkRecognition)
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
    var published: [String] = []
    panel.onInsert = { inserted += $0 }
    panel.onResults = { published = $0 }
    panel.canvas.setTestStrokes(chineseInk.prefix(4).map { $0.map { CGPoint(x: $0.x * 0.6 + 5, y: $0.y * 0.6 + 5) } })
    for _ in 0..<100 {
      if !panel.results.isEmpty { break }
      try await Task.sleep(nanoseconds: 50_000_000)
    }
    XCTAssertTrue(panel.results.contains("中"))
    XCTAssertEqual(published, panel.results, "Recognised candidates never reached the shared strip.")
    XCTAssertEqual(inserted, "")
    let index = try XCTUnwrap(panel.results.firstIndex(of: "中"))
    XCTAssertTrue(panel.use(at: index))
    XCTAssertEqual(inserted, "中")
    XCTAssertFalse(panel.hasInk)
    XCTAssertTrue(panel.results.isEmpty)
    XCTAssertEqual(published, [], "Confirming a candidate did not clear the shared strip.")
    XCTAssertFalse(panel.use(at: index), "A stale candidate must not insert again")
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
  /// 面板只有书写卡片本身；它的 ⌫ 和「重写」是旁边的键盘按键，所以面板上每一点都会落墨。
  func testEveryPointOnThePanelAcceptsInk() throws {
    let panel = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 320, height: 200))
    panel.layoutIfNeeded()
    for point in [CGPoint(x: 20, y: 10), CGPoint(x: 120, y: 100), CGPoint(x: 40, y: 190),
                  CGPoint(x: 160, y: 4), CGPoint(x: 310, y: 190)] {
      XCTAssertTrue(panel.hitTest(point, with: nil) === panel.canvas)
    }
  }
  #endif

  func testHandwritingSchemeKeepsToolbarAndSwitchesBackToLetters() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    let controller = KeyboardViewController(); controller.loadViewIfNeeded()
    for width in [320.0, 414.0] {
      let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
      XCTAssertEqual(height.constant, KeyboardViewController.defaultKeyboardHeight)
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: height.constant); controller.view.layoutIfNeeded()
      let panel = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" } as? HandwritingInputView)
      XCTAssertFalse(panel.isHidden)
      // 书写区是操作行上方设计稿三排 42pt 键的高度，减去面板自身的内边距。
      XCTAssertGreaterThanOrEqual(panel.canvas.bounds.height, 130)
      XCTAssertGreaterThan(panel.canvas.bounds.width, 200)
      let shot = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { controller.view.layer.render(in: $0.cgContext) }); shot.name = "Handwriting keyboard \(Int(width))"; shot.lifetime = .keepAlways; add(shot)
    }
    let language = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "bottomLanguageKey" } as? UIButton)
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, KeyboardViewController.defaultKeyboardHeight)
    XCTAssertTrue(try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" }).isHidden)
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, KeyboardViewController.defaultKeyboardHeight)
    XCTAssertFalse(try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" }).isHidden)
  }
  func testHandwritingHeightTracksOrientationAndSymbolMode() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    let parent = UIViewController()
    let controller = KeyboardViewController()
    parent.addChild(controller)
    controller.loadViewIfNeeded()
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let toggle = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "layoutToggleButton" } as? UIButton)
    let panel = try XCTUnwrap(nodes(controller.view).first { $0 is HandwritingInputView } as? HandwritingInputView)
    let enter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "returnKey" })
    // Annotated rather than inferred: three tuples of four components, where one is an enum written with a leading dot and two are overloaded `+` on CGFloat, give the solver enough freedom that it gives up with "unable to type-check this expression in reasonable time". Naming the type leaves it nothing to solve.
    let layouts: [(UIUserInterfaceSizeClass, CGFloat, CGFloat, CGFloat)] = [
      (.regular, 414.0, KeyboardViewController.defaultKeyboardHeight, KeyboardViewController.defaultKeyboardHeight),
      (.compact, 812.0, KeyboardViewController.keyboardHeight(landscape: true, handwriting: true), KeyboardViewController.keyboardHeight(landscape: true)),
      (.regular, 320.0, KeyboardViewController.defaultKeyboardHeight, KeyboardViewController.defaultKeyboardHeight),
    ]
    for (verticalSize, width, writingHeight, typingHeight) in layouts {
      parent.setOverrideTraitCollection(UITraitCollection(verticalSizeClass: verticalSize), forChild: controller)
      controller.viewDidLayoutSubviews()
      XCTAssertEqual(height.constant, writingHeight)
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: writingHeight)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(enter.bounds.height, 44, accuracy: 0.5)
      XCTAssertGreaterThanOrEqual(panel.canvas.bounds.height, verticalSize == .compact ? 90 : 130)
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

  private func handwritingController(width: CGFloat) throws -> KeyboardViewController {
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func control(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton, identifier)
  }

  private func frame(_ view: UIView, in controller: KeyboardViewController) -> CGRect {
    view.convert(view.bounds, to: controller.view)
  }

  /// 手写键盘沿用 Android 的 rebuildHandwritingRows：左侧竖排 ，。？！，中间是书写卡片，右侧竖排 ⌫ 和「重写」，宽度按 Android 的 .7 / 3 / .8 比例分配；下方是 Android 的 designEntries 给手写用的手机 26 键底排（123 | ， | 手写 | 。 | 中 | return）。
  func testThePadHasAndroidsColumnsAndThePhoneBottomRow() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    let controller = try handwritingController(width: 390)
    let panel = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" } as? HandwritingInputView)
    XCTAssertFalse(panel.isHidden)
    let card = frame(panel, in: controller)
    let marks = try KeyboardViewController.handwritingPunctuation.map { try control("handwritingPunctuation\($0)", in: controller) }
    XCTAssertEqual(marks.map { $0.configuration?.title }, ["，", "。", "？", "！"])
    for mark in marks {
      XCTAssertLessThan(frame(mark, in: controller).maxX, card.minX)
      XCTAssertEqual(mark.bounds.width, card.width * 0.7 / 3, accuracy: 0.5)
    }
    // 四个标点从上到下平分卡片的高度。
    XCTAssertEqual(frame(marks[0], in: controller).minY, card.minY, accuracy: 0.5)
    XCTAssertEqual(frame(marks[3], in: controller).maxY, card.maxY, accuracy: 0.5)
    let delete = try control("handwritingDelete", in: controller)
    let rewrite = try control("handwritingRewrite", in: controller)
    XCTAssertEqual(rewrite.configuration?.title, "重写")
    for tool in [delete, rewrite] {
      XCTAssertGreaterThan(frame(tool, in: controller).minX, card.maxX)
      XCTAssertEqual(tool.bounds.width, card.width * 0.8 / 3, accuracy: 0.5)
    }
    XCTAssertLessThan(frame(delete, in: controller).maxY, frame(rewrite, in: controller).minY)
    XCTAssertTrue(KeyboardViewController.usesPhoneBottomRow(formFactor: .phone, nineKeyFrame: false, handwriting: true, kana: false))
    XCTAssertTrue(KeyboardViewController.usesPhoneBottomRow(formFactor: .tablet, nineKeyFrame: false, handwriting: true, kana: false))
    let comma = try control("quickPunctuationKey", in: controller)
    let period = try control("bottomPeriodKey", in: controller)
    let space = try control("spaceKey", in: controller)
    let language = try control("bottomLanguageKey", in: controller)
    let enter = try control("returnKey", in: controller)
    for key in [comma, period, space, language, enter] { XCTAssertFalse(key.isHidden, key.accessibilityIdentifier ?? "") }
    XCTAssertLessThan(frame(comma, in: controller).maxX, frame(space, in: controller).minX)
    XCTAssertLessThan(frame(space, in: controller).maxX, frame(period, in: controller).minX)
    XCTAssertLessThan(frame(period, in: controller).maxX, frame(language, in: controller).minX)
    XCTAssertLessThan(frame(language, in: controller).maxX, frame(enter, in: controller).minX)
    XCTAssertEqual(space.configuration?.title, "手写")
    XCTAssertNotNil(space.configuration?.image, "the space bar keeps its mic")
  }

  /// 有笔迹时 ⌫ 撤回最后一笔，与 Android 的 deleteFromHandwriting 一致；「重写」清空卡片。
  func testDeleteTakesBackAStrokeAndRewriteClears() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    let controller = try handwritingController(width: 390)
    let panel = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" } as? HandwritingInputView)
    panel.canvas.setTestStrokes([[CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 40)], [CGPoint(x: 20, y: 60), CGPoint(x: 80, y: 60)],
                                 [CGPoint(x: 50, y: 10), CGPoint(x: 50, y: 90)]])
    try control("handwritingDelete", in: controller).sendActions(for: .touchUpInside)
    XCTAssertEqual(panel.canvas.strokes.count, 2)
    try control("handwritingRewrite", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(panel.hasInk)
  }

  /// The simulator build says it cannot recognise, rather than just doing nothing.
  ///
  /// ML Kit's arm64 slice is device-only, so this target draws ink and recognises nothing. That is
  /// the honest answer and it has to be visible: silence is indistinguishable from a broken
  /// keyboard, because the user writes a character, nothing appears, and the panel explains none
  /// of it. On a device build this assertion does not apply — recognition is real there — so it
  /// only runs where the fallback is compiled.
  func testTheSimulatorPanelSaysItCannotRecognise() throws {
    #if targetEnvironment(simulator)
    let view = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 390, height: 200))
    view.layoutIfNeeded()
    let status = try XCTUnwrap(
      nodes(view).compactMap { $0 as? UILabel }
        .first { $0.accessibilityIdentifier == "handwritingStatus" },
      "the fallback panel has nothing that explains why writing produces no candidates")
    XCTAssertFalse(status.isHidden)
    XCTAssertEqual(status.text, HandwritingInputView.unavailableMessage)
    // It draws ink all the same: the panel is the same shape as the device one.
    view.canvas.setTestStrokes([[CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 40)]])
    XCTAssertTrue(view.hasInk)
    XCTAssertFalse(view.commitFirst(), "the fallback must never claim a recognition")
    #else
    throw XCTSkip("Device builds recognise for real; there is no fallback message to check.")
    #endif
  }

}

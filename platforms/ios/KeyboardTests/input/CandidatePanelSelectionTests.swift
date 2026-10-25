import UIKit
import XCTest

/// The expanded candidate panel lists the Engine's whole answer, so its entries are named by their index in that answer rather than by a slot on the visible page.
final class CandidatePanelSelectionTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-candidate-panel-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testAnEntryPastTheVisiblePageCommits() throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.cancel()
    let visible = bridge.handleCharacter("y").candidates
    let (generation, entries) = try panel(bridge)
    let later = try XCTUnwrap(entries.first { $0.position >= visible.count && $0.position >= 9 },
                              "the answer for y should run past the first page")

    XCTAssertFalse(bridge.isOnCurrentPage(generation: generation, globalIndex: later.index))
    let refused = bridge.selectCandidate(generation: generation, globalIndex: later.index)
    XCTAssertNil(refused.commitText, "the page-bounded select still refuses an off-page entry")

    let selected = bridge.selectAnyCandidate(generation: generation, globalIndex: later.index)
    XCTAssertEqual(selected.commitText, later.text)
  }

  /// #9193：只选掉前面一段、组字还在时面板留着；整句上屏、组字清空或引擎拒绝时收起。
  func testPanelStaysOpenOnlyWhileASelectionLeavesComposition() {
    XCTAssertTrue(CandidatePanelSelectionPolicy.keepsOpen(handled: true, editingText: "hao"))
    XCTAssertFalse(CandidatePanelSelectionPolicy.keepsOpen(handled: true, editingText: ""), "整句上屏后收起")
    XCTAssertFalse(CandidatePanelSelectionPolicy.keepsOpen(handled: false, editingText: "hao"), "过期的一代被拒绝时不留旧面板")
  }

  /// 笔画比的是刚被选掉的那个字，部分选词后清空；没有笔画时不发请求。
  func testAPartialSelectionClearsOnlyTheStrokePrefix() {
    XCTAssertEqual(CandidatePanelSelectionPolicy.strokesAfterPartialSelection("hs"), "")
    XCTAssertNil(CandidatePanelSelectionPolicy.strokesAfterPartialSelection(""))
  }

  func testAStaleGenerationIsRefused() throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.cancel()
    _ = bridge.handleCharacter("y")
    let (generation, entries) = try panel(bridge)
    let first = try XCTUnwrap(entries.first)
    XCTAssertTrue(bridge.isOnCurrentPage(generation: generation, globalIndex: first.index))
    _ = bridge.handleCharacter("i")

    XCTAssertFalse(bridge.isOnCurrentPage(generation: generation, globalIndex: first.index))
    XCTAssertNil(bridge.selectAnyCandidate(generation: generation, globalIndex: first.index).commitText)
  }

  @MainActor
  func testLargePanelReusesVisibleButtonsAndKeepsTailSelectionAfterRotation() throws {
    for threeColumns in [false, true] {
      var selections: [Int] = []
      let panel = KeyboardCandidatePanelView(
        candidates: (0..<10000).map { "合成词\($0)" }, preedit: "y", showsHeader: false,
        display: { $0 }, menuElements: { _ in [UIAction(title: "合成管理", handler: { _ in })] },
        onSelect: { selections.append($0) }, onClose: {},
        columns: threeColumns ? (UIView(), UIView()) : nil)
      let controller = UIViewController()
      controller.view = panel
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 300))
      window.rootViewController = controller
      window.isHidden = false
      defer { window.isHidden = true }
      panel.frame = window.bounds
      panel.layoutIfNeeded()
      let collection = try XCTUnwrap(descendants(panel).compactMap { $0 as? UICollectionView }.first)
      XCTAssertEqual(collection.numberOfItems(inSection: 0), 10000)
      XCTAssertLessThan(candidateButtons(panel).count, 200, "常驻按钮不能随完整候选数增长")
      for width in [390.0, 780.0] {
        window.frame.size.width = width
        panel.frame = window.bounds
        panel.setNeedsLayout()
        panel.layoutIfNeeded()
        XCTAssertGreaterThan(collection.bounds.width, width * 0.6)
        collection.scrollToItem(at: IndexPath(item: 9999, section: 0), at: .bottom, animated: false)
        collection.layoutIfNeeded()
        let last = try XCTUnwrap(candidateButtons(panel).first { $0.accessibilityIdentifier == "panelCandidate-10000" })
        XCTAssertTrue(last.accessibilityLabel?.contains("合成词9999") == true)
        XCTAssertNotNil(last.menu)
        last.sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(selections.last, 9999)
        XCTAssertLessThan(candidateButtons(panel).count, 200)
      }
      let oldOffset = collection.contentOffset.y
      panel.reload(candidates: (0..<10000).map { "合成词\($0)" }, annotations: [], markers: [], keepingScrollPosition: true)
      panel.layoutIfNeeded()
      XCTAssertEqual(collection.contentOffset.y, oldOffset, accuracy: 0.1, "只换代不应跳回开头")
      panel.updateAnnotations((0..<10000).map { _ in KeyboardCandidateAnnotation(text: "合成释义", accessibilityDescription: "合成释义") })
      panel.layoutIfNeeded()
      collection.scrollToItem(at: IndexPath(item: 9999, section: 0), at: .bottom, animated: false)
      collection.layoutIfNeeded()
      let annotated = try XCTUnwrap(candidateButtons(panel).first { $0.accessibilityIdentifier == "panelCandidate-10000" })
      XCTAssertTrue(annotated.accessibilityLabel?.contains("合成释义") == true)
      annotated.sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(selections, [9999, 9999, 9999], "复用不能保留上一条候选的动作")
      panel.reload(candidates: ["合成甲", "合成乙"], annotations: [], markers: [], keepingScrollPosition: true)
      panel.layoutIfNeeded()
      XCTAssertEqual(collection.contentOffset.y, 0, accuracy: 0.1)
      XCTAssertEqual(collection.numberOfItems(inSection: 0), 2)
      let first = try XCTUnwrap(candidateButtons(panel).first { $0.accessibilityIdentifier == "panelCandidate-1" })
      first.sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(selections.last, 0)
    }
  }

  @MainActor
  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  @MainActor
  private func candidateButtons(_ panel: UIView) -> [UIButton] {
    descendants(panel).compactMap { $0 as? UIButton }.filter { $0.accessibilityIdentifier?.hasPrefix("panelCandidate-") == true }
  }

  private func panel(_ bridge: MetasequoiaInputSessionBridge) throws -> (UInt64, [(position: Int, index: UInt64, text: String)]) {
    let answer = try bridge.allCandidates()
    let generation = try XCTUnwrap((answer["generation"] as? NSNumber)?.uint64Value)
    let rows = try XCTUnwrap(answer["candidates"] as? [[String: Any]])
    let entries = rows.enumerated().compactMap { position, row -> (position: Int, index: UInt64, text: String)? in
      guard let identity = row["id"] as? [String: Any], let index = (identity["index"] as? NSNumber)?.uint64Value,
            let text = row["text"] as? String else { return nil }
      return (position, index, text)
    }
    return (generation, entries)
  }
}

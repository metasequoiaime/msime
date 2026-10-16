import UIKit
import XCTest

/// Cloud, AI and pinned candidates are marked the way the Windows candidate window marks them.
final class CandidateMarkerTests: XCTestCase {
  func testCandidateSnapshotRejectsFractionalGenerationAndIndex() {
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": NSNumber(value: 4.5), "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0]]],
    ]))
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0.5]]],
    ]))
    let oversized = NSNumber(value: 1e40)
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": oversized, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": oversized, "index": 0]]],
    ]))
  }

  func testCandidateSnapshotRejectsFractionalMarkerMetadata() {
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0], "source": 2.5]],
    ]))
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 4, "index": 0], "fixed_position": 1.5]],
    ]))
  }

  func testCandidateSnapshotAcceptsIntegerZeroAndOneValues() throws {
    let snapshot = try CandidatePanelSnapshot.decode([
      "generation": 1, "preedit": "ni",
      "candidates": [["text": "你", "id": ["generation": 1, "index": 0], "source": 1,
                      "fixed_position": 0]],
    ])
    XCTAssertEqual(snapshot.generation, 1)
    XCTAssertEqual(snapshot.entries.first?.index, 0)
    XCTAssertEqual(snapshot.entries.first?.source, 1)
  }

  func testOnlyNetworkSourcesAndPinnedWordsAreMarked() {
    XCTAssertEqual(CandidateMarker.markers(source: 0, fixedPosition: 0), [])
    XCTAssertEqual(CandidateMarker.markers(source: 1, fixedPosition: 0), [], "a user word is still a dictionary word")
    XCTAssertEqual(CandidateMarker.markers(source: 2, fixedPosition: 0).map(\.spoken), ["云候选"])
    XCTAssertEqual(CandidateMarker.markers(source: 3, fixedPosition: 0).map(\.spoken), ["AI 候选"])
    XCTAssertEqual(CandidateMarker.markers(source: 0, fixedPosition: 3).map(\.spoken), ["已固定第 3 位"])
    for marker in CandidateMarker.markers(source: 2, fixedPosition: 0) + CandidateMarker.markers(source: 3, fixedPosition: 1) {
      XCTAssertNotNil(UIImage(systemName: marker.symbol), marker.symbol)
    }
  }

  func testThePanelKeepsEachCandidatesSourceAndPinnedSlot() throws {
    let snapshot = try CandidatePanelSnapshot.decode([
      "generation": 4, "preedit": "ni",
      "candidates": [
        ["text": "你", "id": ["generation": 4, "index": 0], "source": 0, "fixed_position": 1],
        ["text": "妮", "id": ["generation": 4, "index": 1], "source": 2, "fixed_position": 0],
        ["text": "泥", "id": ["generation": 4, "index": 2]],
      ],
    ])
    XCTAssertEqual(snapshot.entries.map(\.fixedPosition), [1, 0, 0])
    XCTAssertEqual(snapshot.entries.map(\.source), [0, 2, 0])
  }

  /// 展开的候选网格和候选条一样，把来源 12 的 AI 占位行画成次要色，读屏念等待中或没有结果（#6888）。共享内核已经不再产出这种行，画法留着，旧数据里出现时仍不当成真候选。
  func testTheExpandedPanelDrawsTheAIPlaceholderAsSecondary() throws {
    let panel = KeyboardCandidatePanelView(
      candidates: ["你", "妮", "…", "泥"], preedit: "ni", aiPlaceholders: [2],
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
    panel.layoutIfNeeded()
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    func chip(_ number: Int) throws -> UIButton {
      try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-\(number)" } as? UIButton)
    }
    let skin = KeyboardTheme.current
    XCTAssertEqual(try chip(3).configuration?.baseForegroundColor, skin.secondary)
    XCTAssertEqual(try chip(3).accessibilityLabel, "候选词 3：AI 候选等待中")
    XCTAssertEqual(try chip(4).configuration?.baseForegroundColor, skin.keyForeground)
    XCTAssertEqual(try chip(4).accessibilityLabel, "候选词 4：泥")

    panel.reload(candidates: ["你", "妮", "—"], annotations: [], markers: [], aiPlaceholders: [2])
    XCTAssertEqual(try chip(3).configuration?.baseForegroundColor, skin.secondary)
    XCTAssertEqual(try chip(3).accessibilityLabel, "候选词 3：AI 候选没有结果")
  }

  /// AI 的回答到达时换了一代，但最多只有 AI 行变了：第一条插在第一页最后一格（这里每页两个），其余接在末尾，别的行先后不变；回答的词全都已在列表里时换了一代、列表一行没变。两种情况展开的网格都原地刷新、不收起（#6888）。别的行变了、组字变了、行数变少、云候选到达都不算。
  func testOnlyAChangeInTheAISeatKeepsTheExpandedPanel() throws {
    func snapshot(_ generation: UInt64, _ rows: [(String, Int)], preedit: String = "nihao") throws -> CandidatePanelSnapshot {
      try CandidatePanelSnapshot.decode([
        "generation": generation, "preedit": preedit,
        "candidates": rows.enumerated().map { index, row in
          ["text": row.0, "id": ["generation": generation, "index": index], "source": row.1] as [String: Any]
        },
      ])
    }
    let waiting = try snapshot(4, [("你好", 0), ("倪好", 0), ("拟好", 0)])
    let answered = try snapshot(5, [("你好", 0), ("合成一", 3), ("倪好", 0), ("拟好", 0), ("合成二", 3)])
    XCTAssertTrue(answered.onlyTheAISeatChanged(since: waiting),
                  "the first answer takes the last first-page slot and the rest of the batch is appended")
    XCTAssertTrue(try snapshot(6, [("你好", 0), ("合成三", 3), ("倪好", 0), ("拟好", 0)]).onlyTheAISeatChanged(since: answered),
                  "a later batch replaces the earlier AI rows")
    XCTAssertFalse(try snapshot(5, [("你好", 0), ("合成一", 3), ("泥好", 0), ("拟好", 0)]).onlyTheAISeatChanged(since: waiting))
    XCTAssertFalse(try snapshot(5, [("倪好", 0), ("合成一", 3), ("你好", 0), ("拟好", 0)]).onlyTheAISeatChanged(since: waiting))
    XCTAssertFalse(try snapshot(5, [("你好", 0), ("合成一", 3), ("倪好", 0)]).onlyTheAISeatChanged(since: waiting))
    XCTAssertFalse(try snapshot(5, [("你好", 0), ("云", 2), ("合成一", 3), ("倪好", 0), ("拟好", 0)]).onlyTheAISeatChanged(since: waiting))
    XCTAssertFalse(try snapshot(5, [("你好", 0), ("合成一", 3), ("倪好", 0), ("拟好", 0)], preedit: "nihaoa").onlyTheAISeatChanged(since: waiting))
    XCTAssertTrue(try snapshot(5, [("你好", 0), ("倪好", 0), ("拟好", 0)]).onlyTheAISeatChanged(since: waiting),
                  "an answer whose words are all listed already adds nothing but still takes a new generation")
  }

  /// 原地刷新时网格留在用户滚到的位置：AI 回答插进第一页最后一格；九键面板换拼音那样的重建仍回到开头。
  func testReloadingTheAISeatKeepsTheScrollPosition() throws {
    var candidates = (1...80).map { "候选\($0)" }
    let panel = KeyboardCandidatePanelView(
      candidates: candidates, preedit: "nihao",
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
    panel.layoutIfNeeded()
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
    XCTAssertGreaterThan(scroll.contentSize.height, scroll.bounds.height + 100, "the grid has to scroll for this test")
    scroll.setContentOffset(CGPoint(x: 0, y: 100), animated: false)

    candidates.insert("合成一", at: 5)
    panel.reload(candidates: candidates, annotations: [], markers: [], keepingScrollPosition: true)
    panel.layoutIfNeeded()
    XCTAssertEqual(scroll.contentOffset.y, 100, accuracy: 0.5)
    let seat = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-6" } as? UIButton)
    XCTAssertEqual(seat.accessibilityLabel, "候选词 6：合成一")

    panel.reload(candidates: candidates, annotations: [], markers: [])
    XCTAssertEqual(scroll.contentOffset.y, 0, accuracy: 0.5)
  }
}

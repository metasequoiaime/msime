import UIKit
import XCTest

/// 引擎改过拼写的候选（View 的 `corrected`）在 iOS 上标 `*`、不再画原串、读屏念「纠错前的拼写」，与 Android 的 `CandidateCorrectionMarkPolicy` 和桌面候选窗的 `*` 同一条规则（#11844）。
final class CandidateCorrectionTests: XCTestCase {
  func testTheMarkIsDrawnAfterTheWordOnly() {
    XCTAssertEqual(CandidateCorrectionPolicy.markedWord("中国", corrected: true), "中国*")
    XCTAssertEqual(CandidateCorrectionPolicy.markedWord("中国", corrected: false), "中国")
    XCTAssertEqual(CandidateCorrectionPolicy.mark, "*", "与 Windows、macOS、Linux 候选窗的标记相同")
  }

  func testOnlyTheSpellingAHelpcodeDidNotReplaceIsDropped() {
    XCTAssertEqual(CandidateCorrectionPolicy.visibleAnnotation("zhognguo", corrected: true), "")
    XCTAssertEqual(CandidateCorrectionPolicy.visibleAnnotation("zhognguo", corrected: false), "zhognguo")
    XCTAssertEqual(CandidateCorrectionPolicy.visibleAnnotation("(mY)", corrected: true), "(mY)", "辅助码照旧画")
    XCTAssertEqual(CandidateCorrectionPolicy.visibleAnnotation("zhog wq", corrected: true), "wq", "五笔混输只去掉打头的原串")
    XCTAssertEqual(CandidateCorrectionPolicy.visibleAnnotation("", corrected: true), "")
  }

  func testTheSpokenFormIsTheSpellingTheCorrectionReplaced() {
    XCTAssertEqual(CandidateCorrectionPolicy.spokenAnnotation("zhognguo", corrected: true), "纠错前的拼写 zhognguo")
    XCTAssertEqual(CandidateCorrectionPolicy.spokenAnnotation("zhog wq", corrected: true), "纠错前的拼写 zhog")
    XCTAssertNil(CandidateCorrectionPolicy.spokenAnnotation("(mY)", corrected: true), "辅助码不是纠错前的拼写")
    XCTAssertNil(CandidateCorrectionPolicy.spokenAnnotation("zhognguo", corrected: false))
  }

  func testTheExpandedPanelMarksACorrectedWordAndKeepsTheMarkOutOfVoiceOver() throws {
    XCTAssertEqual(
      try CandidatePanelSnapshot.decode([
        "generation": 1, "preedit": "ni",
        "candidates": [["text": "你", "id": ["generation": 1, "index": 0], "corrected": true],
                       ["text": "呢", "id": ["generation": 1, "index": 1]]],
      ]).entries.map(\.corrected), [true, false], "缺省按未纠错读")
    let panel = KeyboardCandidatePanelView(
      candidates: ["你", "呢"], preedit: "ni", annotations: [], corrections: [0],
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
    panel.layoutIfNeeded()
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    func chip(_ number: Int) throws -> UIButton {
      try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-\(number)" } as? UIButton)
    }
    let title = NSAttributedString(try XCTUnwrap(try chip(1).configuration?.attributedTitle)).string
    XCTAssertEqual(title, "你*")
    XCTAssertEqual(try chip(1).accessibilityLabel, "候选词 1：你", "读屏不念 *")
    XCTAssertEqual(try chip(2).accessibilityLabel, "候选词 2：呢")
  }
}

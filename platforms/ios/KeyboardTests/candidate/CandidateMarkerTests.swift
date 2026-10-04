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
}

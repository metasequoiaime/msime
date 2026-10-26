import XCTest

final class CandidatePanelSnapshotTests: XCTestCase {
  func testDecodePreservesCompleteCandidateIdentityAndAnnotations() throws {
    let snapshot = try CandidatePanelSnapshot.decode([
      "generation": 7,
      "preedit": "wq",
      "candidates": [
        ["id": ["generation": 7, "index": 0], "text": "你", "code": "wq"],
        [
          "id": ["generation": 7, "index": 1], "text": "爷", "code": "wqb",
          "translation": "grandfather",
        ],
      ],
    ])
    XCTAssertEqual(snapshot.generation, 7)
    XCTAssertEqual(snapshot.preedit, "wq")
    XCTAssertEqual(snapshot.entries.map(\.index), [0, 1])
    XCTAssertEqual(snapshot.entries.map(\.code), ["wq", "wqb"])
    XCTAssertEqual(snapshot.entries.map(\.translation), ["", "grandfather"])
  }

  /// 展开面板按高亮决定哪个候选画候选词翻译（「仅当前高亮」），所以要读出每行的 `highlighted`，缺省为假。
  func testDecodeReadsWhichEntryIsHighlighted() throws {
    let snapshot = try CandidatePanelSnapshot.decode([
      "generation": 7,
      "preedit": "nihao",
      "candidates": [
        ["id": ["generation": 7, "index": 0], "text": "你好", "highlighted": true],
        ["id": ["generation": 7, "index": 1], "text": "泥嚎", "highlighted": false],
        ["id": ["generation": 7, "index": 2], "text": "拟好"],
      ],
    ])
    XCTAssertEqual(snapshot.entries.map(\.highlighted), [true, false, false])
  }

  func testDecodeRetainsEveryCandidateBeyondTheGlossRequestLimit() throws {
    for count in [4097, 10000] {
      let rows: [[String: Any]] = (0..<count).map { index in
        ["id": ["generation": 7, "index": index], "text": "合成词\(index)"]
      }
      let snapshot = try CandidatePanelSnapshot.decode([
        "generation": 7, "preedit": "y", "candidates": rows,
      ])
      XCTAssertEqual(snapshot.entries.count, count)
      XCTAssertEqual(snapshot.entries.last?.index, UInt64(count - 1))
      XCTAssertEqual(snapshot.entries.last?.text, "合成词\(count - 1)")
    }
  }

  func testDecodeRejectsStaleOrDuplicateCandidateIdentity() {
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 7,
      "preedit": "yi",
      "candidates": [
        ["id": ["generation": 6, "index": 0], "text": "一"],
      ],
    ]))
    XCTAssertThrowsError(try CandidatePanelSnapshot.decode([
      "generation": 7,
      "preedit": "yi",
      "candidates": [
        ["id": ["generation": 7, "index": 0], "text": "一"],
        ["id": ["generation": 7, "index": 0], "text": "乙"],
      ],
    ]))
  }
}

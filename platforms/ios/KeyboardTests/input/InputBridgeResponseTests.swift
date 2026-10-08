import XCTest

final class InputBridgeResponseTests: XCTestCase {
  func testMalformedEnvelopeReleasesNativeResponseOnce() throws {
    let pointer = strdup("[]")!
    var releases = 0
    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.decodeResponse(pointer) { pointer in
      releases += 1
      free(pointer)
    })
    XCTAssertEqual(releases, 1)
  }

  /// 全拼九键的读音和筛选状态随每次回应一起到，键盘不再另外问一次。
  func testSnapshotDecodesNineKeyReadingAndFilter() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-nine-key-decode-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let snapshot = try bridge.snapshot(from: [
      "handled": true,
      "view": [
        "preedit": "6464'224", "nine_key_spellings": ["bai", "cai", "B", "C"],
        "nine_key_reading": "ning'bai", "nine_key_single_character": true, "nine_key_strokes": "hs",
      ] as [String: Any],
    ])
    XCTAssertEqual(snapshot.nineKeyReading, "ning'bai")
    XCTAssertTrue(snapshot.nineKeySingleCharacter)
    XCTAssertEqual(snapshot.nineKeyStrokes, "hs")
    XCTAssertEqual(snapshot.nineKeySpellings, ["bai", "cai", "B", "C"])

    let bare = try bridge.snapshot(from: ["handled": true, "view": ["preedit": "64"]])
    XCTAssertEqual(bare.nineKeyReading, "")
    XCTAssertFalse(bare.nineKeySingleCharacter)
    XCTAssertEqual(bare.nineKeyStrokes, "")
    XCTAssertThrowsError(try bridge.snapshot(from: ["view": ["nine_key_single_character": 1]]))
  }
}

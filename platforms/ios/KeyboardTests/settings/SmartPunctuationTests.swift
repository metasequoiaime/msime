import XCTest

final class SmartPunctuationTests: XCTestCase {
  func testDocumentAdapterReturnsOnlyTheImmediatelyPrecedingScalar() {
    XCTAssertEqual(KeyboardPunctuationContext.precedingScalar(nil), 0)
    XCTAssertEqual(KeyboardPunctuationContext.precedingScalar(""), 0)
    XCTAssertEqual(KeyboardPunctuationContext.precedingScalar("fixture7"), UInt32(ascii: "7"))
    XCTAssertEqual(KeyboardPunctuationContext.precedingScalar("fixtureZ"), UInt32(ascii: "Z"))
    XCTAssertEqual(KeyboardPunctuationContext.precedingScalar("fixture中"), 0x4e2d)
    XCTAssertEqual(KeyboardPunctuationContext.precedingScalar("fixture🌲"), 0x1f332)
  }

  func testDisplayedPunctuationMapsToEngineAsciiInputs() {
    XCTAssertEqual(KeyboardPunctuationContext.engineInput(for: "，", japanese: false), ",")
    XCTAssertEqual(KeyboardPunctuationContext.engineInput(for: "。", japanese: false), ".")
    XCTAssertEqual(KeyboardPunctuationContext.engineInput(for: "：", japanese: false), ":")
    XCTAssertEqual(KeyboardPunctuationContext.engineInput(for: "「", japanese: true), "[")
    XCTAssertEqual(KeyboardPunctuationContext.engineInput(for: "・", japanese: true), "/")
    XCTAssertEqual(KeyboardPunctuationContext.engineInput(for: "@", japanese: false), "@")
    XCTAssertNil(KeyboardPunctuationContext.engineInput(for: "……", japanese: false))
    // Korean routes only ASCII keys; a Chinese mark from the symbol panel is typed as it is.
    XCTAssertEqual(KeyboardPunctuationContext.engineInput(for: ".", japanese: false, asciiMarks: true), ".")
    XCTAssertNil(KeyboardPunctuationContext.engineInput(for: "，", japanese: false, asciiMarks: true))
  }

  func testBridgeUsesSmartContextOnlyWhileEngineIsIdle() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-smart-punctuation-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    // What the bridge owes is the context: the preceding scalar reaches the shared layer, which
    // then decides. With the shipped defaults `smart_punctuation_direct_letter` is on - it follows
    // its parent switch rather than being off on its own - so a comma after a letter stays ASCII
    // and the shared route commits nothing Chinese for it. After a Han character there is no such
    // rule and the comma is 。's neighbour: 「，」.
    //
    // This assertion said the opposite until now, because the switch used to ship off; it changed
    // in `feat: honor the smart punctuation direct switches in the shared route` and nothing
    // noticed, since this suite is not in verify-local.sh and the committed Xcode project had
    // stopped building.
    let afterLetter = bridge.handlePunctuationWithContext(",", preceding: UInt32(ascii: "a"))
    XCTAssertNotEqual(afterLetter.commitText, "，")
    XCTAssertEqual(bridge.handlePunctuationWithContext(",", preceding: 0x4e2d).commitText, "，")

    _ = bridge.handleCharacter("n")
    _ = bridge.handleCharacter("i")
    // With a composition open the preceding scalar is not the document's any more - the candidate
    // is - so the direct-letter rule does not apply and the comma finishes the composition in
    // Chinese.
    let composed = try XCTUnwrap(
      bridge.handlePunctuationWithContext(",", preceding: UInt32(ascii: "a")).commitText)
    XCTAssertTrue(composed.hasSuffix("，"))
  }
}

private extension UInt32 {
  init(ascii character: Character) {
    self = UInt32(character.asciiValue!)
  }
}

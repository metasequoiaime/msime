import XCTest

/// 「全角输入」 reaches the runtime, survives the session being rebuilt, and starts from the shared `character_width` in its lower-case spelling.
final class CharacterWidthPreferenceTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-character-width-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private func commitRaw(_ bridge: MetasequoiaInputSessionBridge, _ letters: String) -> String? {
    _ = bridge.cancel()
    for letter in letters { _ = bridge.handleCharacter(String(letter)) }
    return bridge.commitRaw().commitText
  }

  /// What the runtime commits comes back converted once it is told the width, and a session rebuilt after the keyboard was put away is told again.
  func testRuntimeCommitsInTheWidthItIsToldAcrossRebuilds() throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(commitRaw(bridge, "ni"), "ni")
    bridge.setCharacterWidth(fullwidth: true)
    XCTAssertEqual(commitRaw(bridge, "ni"), "ｎｉ")

    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    XCTAssertEqual(commitRaw(bridge, "ni"), "ｎｉ")

    bridge.setCharacterWidth(fullwidth: false)
    XCTAssertEqual(commitRaw(bridge, "ni"), "ni")
  }

  /// The document's spelling is lower case; the runtime view's `Fullwidth` would silently read as off.
  func testStartsFromTheDocumentSpelling() {
    XCTAssertTrue(CharacterWidthPreference.startsFullwidth(in: ["character_width": "fullwidth"]))
    XCTAssertFalse(CharacterWidthPreference.startsFullwidth(in: ["character_width": "Fullwidth"]))
    XCTAssertFalse(CharacterWidthPreference.startsFullwidth(in: ["character_width": "halfwidth"]))
    XCTAssertFalse(CharacterWidthPreference.startsFullwidth(in: nil))
  }

  /// Only a change to `character_width` itself replaces the keyboard's switch.
  func testOnlyAWidthChangeOverridesTheSwitch() {
    XCTAssertFalse(CharacterWidthPreference.overridesToggle(previous: "halfwidth", next: "halfwidth"))
    XCTAssertTrue(CharacterWidthPreference.overridesToggle(previous: "halfwidth", next: "fullwidth"))
    XCTAssertTrue(CharacterWidthPreference.overridesToggle(previous: nil, next: "fullwidth"))
    XCTAssertFalse(CharacterWidthPreference.overridesToggle(previous: "fullwidth", next: nil))
  }
}

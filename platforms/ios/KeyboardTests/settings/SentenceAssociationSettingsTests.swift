import XCTest

/// The 输入 page's 整句联想 writes the same three levels as Android into `sentence_association`, keeps the object's other fields, and the keyboard that is already open accepts the result.
@MainActor
final class SentenceAssociationSettingsTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-sentence-association-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testLevelsReadTheDocumentAsAndroidDoes() {
    XCTAssertEqual(SentenceAssociationPreference.level(in: nil), .standard, "a missing object is the default 标准")
    XCTAssertEqual(SentenceAssociationPreference.level(in: ["sentence_association": [String: Any]()]), .standard)
    XCTAssertEqual(SentenceAssociationPreference.level(in: ["sentence_association": ["neural_keyboard": true]]), .enhanced)
    XCTAssertEqual(
      SentenceAssociationPreference.level(in: ["sentence_association": ["word_lattice": false, "neural_keyboard": true]]),
      .off, "the lattice switch off is 关闭 whatever the model switch says")
  }

  func testEachLevelRoundTripsAndKeepsTheOtherFields() {
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["sentence_association"] = ["neural_desktop": true, "show_next_on_duplicate": true]
    })
    for level in SentenceAssociationPreference.Level.allCases {
      XCTAssertTrue(SentenceAssociationPreference.save(level, stateRoot: state), "\(level)")
      let document = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
      XCTAssertEqual(SentenceAssociationPreference.level(in: document), level)
      let association = document?["sentence_association"] as? [String: Any]
      XCTAssertEqual(association?["word_lattice"] as? Bool, level != .off)
      XCTAssertEqual(association?["neural_keyboard"] as? Bool, level == .enhanced)
      XCTAssertEqual(association?["neural_desktop"] as? Bool, true, "\(level) kept neural_desktop")
      XCTAssertEqual(association?["show_next_on_duplicate"] as? Bool, true, "\(level) kept show_next_on_duplicate")
    }
  }

  func testTheOpenKeyboardAcceptsTheChangedLevel() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(SentenceAssociationPreference.save(.enhanced, stateRoot: state))
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the session refused a document carrying sentence_association")
  }
}

import XCTest

/// The settings app's helpcode page merges single fields into the helpcode objects of the shared document; the keyboard has to act on them, and needs the Engine's tables to act with.
@MainActor
final class HelpcodeSettingsTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-helpcode-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// Every scheme the page offers has its table beside the dictionaries; a missing one reads as helpcode narrowing nothing.
  func testEveryOfferedSchemaShipsItsTable() throws {
    let resources = try XCTUnwrap(Bundle.main.url(forResource: "EngineResources", withExtension: nil))
    let tables = ["helpcode.txt", "zrm_helpcode_big_unique.txt", "shouyou2_0_helpcode.txt",
                  "shouyouplus_helpcode.txt", "xiaohe_helpcode.txt", "jiajia_helpcode.txt",
                  "wubi86_helpcode.txt"]
    for table in tables {
      let path = resources.appendingPathComponent("helpcodes/\(table)").path
      XCTAssertTrue(FileManager.default.fileExists(atPath: path), "\(table) is not bundled")
    }
  }

  /// A Shift letter narrows the composition by helpcode, and turning helpcode off in the app reaches the session the keyboard already has.
  func testReloadTurnsHelpcodeOffInTheLiveSession() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let plain = compose(bridge, "shi", helpcode: nil)
    // 自然码 Y is the 讠 radical: 识 试 诗 lead instead of 是.
    let narrowed = compose(bridge, "shi", helpcode: "Y")
    XCTAssertEqual(plain.candidates.first, "是")
    XCTAssertEqual(narrowed.preedit, "shiY")
    XCTAssertEqual(narrowed.candidates.first, "识")

    update { $0["enabled"] = false }
    await reload(bridge)

    let off = compose(bridge, "shi", helpcode: "Y")
    XCTAssertEqual(off.preedit, "shi")
    XCTAssertEqual(off.candidates.first, "是")
  }

  /// "Show in the candidate bar" puts each candidate's helpcode in the annotations the strip draws.
  func testShownHelpcodeReachesTheCandidateAnnotations() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(compose(bridge, "shi", helpcode: nil).candidateAnnotations.first, "", "全拼 ships with helpcode hidden")

    update { $0["show_in_candidate_window"] = true }
    await reload(bridge)

    let shown = compose(bridge, "shi", helpcode: nil)
    let first = shown.candidateAnnotations.first ?? ""
    // Bracketed the way the Engine appends it on desktop; the strip drops the brackets when it draws.
    XCTAssertTrue(first.hasPrefix("(") && first.hasSuffix(")"), "\(first) is not a bracketed helpcode")
    XCTAssertTrue(first.dropFirst().dropLast().allSatisfy { $0.isLetter }, "\(first) is not a helpcode")
  }

  private func update(_ change: (inout [String: Any]) -> Void) {
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var helpcode = $0["quanpin_helpcode"] as? [String: Any] ?? [:]
      change(&helpcode)
      $0["quanpin_helpcode"] = helpcode
    })
  }

  private func reload(_ bridge: MetasequoiaInputSessionBridge) async {
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")
  }

  /// The snapshot after typing `letters` from an empty composition, then a Shift `helpcode` letter; the composition is then abandoned.
  private func compose(_ bridge: MetasequoiaInputSessionBridge, _ letters: String, helpcode: String?) -> MetasequoiaInputSnapshot {
    var snapshot = bridge.cancel()
    for letter in letters { snapshot = bridge.handleCharacter(String(letter)) }
    if let helpcode { snapshot = bridge.handleCharacter(helpcode, shifted: true) }
    _ = bridge.cancel()
    return snapshot
  }
}

import XCTest

/// 词库页的「自造词位置」读写共享文档的 `wubi_phrase_placement`（#11863），取值和回退与共享偏好、Windows 设置页一致。
final class WubiPhrasePlacementPreferenceTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-wubi-phrase-placement-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testOptionsMatchTheSharedPreferenceAndWindows() {
    XCTAssertEqual(WubiPhrasePlacement.allCases.map(\.rawValue), ["bottom", "top"])
    XCTAssertEqual(WubiPhrasePlacement.allCases.map(\.title), ["同码词后", "同码词前"])
  }

  func testMissingOrUnknownValuesReadAsBottom() {
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: nil), .bottom)
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: [:]), .bottom)
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: ["wubi_phrase_placement": "middle"]), .bottom)
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: ["wubi_phrase_placement": 1]), .bottom)
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: ["wubi_phrase_placement": "top"]), .top)
  }

  func testNewDocumentReadsAsBottom() {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    XCTAssertNil(document?["wubi_phrase_placement"])
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: document), .bottom)
  }

  func testTopIsWrittenAndBottomRemovesTheKey() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(WubiPhrasePlacementPreference.update(.top, stateRoot: state))
    var document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["wubi_phrase_placement"] as? String, "top")
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: document), .top)

    XCTAssertTrue(WubiPhrasePlacementPreference.update(.bottom, stateRoot: state))
    document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertNil(document["wubi_phrase_placement"], "默认值不写进文档，与共享偏好一致")
    XCTAssertEqual(WubiPhrasePlacementPreference.placement(in: document), .bottom)
  }
}

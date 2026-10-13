import XCTest

/// The settings app's local-mode page merges single fields into `local_modes`; the session the keyboard already has must stop opening a mode turned off there.
@MainActor
final class LocalModeSettingsTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-local-modes-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testReloadTurnsAModeOffInTheLiveSession() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(opens(bridge, "T"), "date and time ship on")
    XCTAssertTrue(opens(bridge, "U"), "Unicode ships on")

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var modes = $0["local_modes"] as? [String: Any] ?? [:]
      modes["date_time"] = false
      $0["local_modes"] = modes
    })
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")

    XCTAssertFalse(opens(bridge, "T"))
    XCTAssertTrue(opens(bridge, "U"), "turning one mode off leaves the others")
    let stored = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["local_modes"] as? [String: Any]
    XCTAssertEqual(stored?["unicode"] as? Bool, true, "the merge kept the object's other fields")
  }

  /// 拆字在触屏平台上缺键读作开，和设置页、键盘列表的 `?? true` 一致；从 C 进入，部件拼音列出要找的字，注释是带调读音。设置页关掉后，正在用的会话不再打开它。
  func testChaiziShipsOnAndTurnsOffInTheLiveSession() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(opens(bridge, "C"), "拆字 ships on for a touch keyboard")

    var snapshot = bridge.openLocalMode("C")
    XCTAssertEqual(snapshot.localMode, "chaizi")
    for letter in "huohuohuo" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertEqual(snapshot.candidates.first, "焱", "\(snapshot.candidates)")
    XCTAssertEqual(snapshot.candidateAnnotations.first, "yàn")
    _ = bridge.cancel()
    snapshot = bridge.openLocalMode("C")
    for letter in "nv'zi" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertEqual(snapshot.candidates.first, "好", "a separator between components is accepted: \(snapshot.candidates)")
    XCTAssertEqual(bridge.selectCandidate(at: 0).commitText, "好")
    XCTAssertFalse(bridge.isInLocalMode, "choosing the character leaves the mode")

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var modes = $0["local_modes"] as? [String: Any] ?? [:]
      modes["chaizi"] = false
      $0["local_modes"] = modes
    })
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")
    XCTAssertFalse(opens(bridge, "C"))
    XCTAssertTrue(opens(bridge, "U"), "turning 拆字 off leaves the others")
  }

  /// Whether the mode opened by `trigger` is running afterwards; the attempt is then abandoned.
  private func opens(_ bridge: MetasequoiaInputSessionBridge, _ trigger: String) -> Bool {
    _ = bridge.cancel()
    let opened = bridge.openLocalMode(trigger).isInLocalMode
    _ = bridge.cancel()
    return opened
  }
}

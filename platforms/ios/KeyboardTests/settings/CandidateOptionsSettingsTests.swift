import XCTest

/// The settings app's candidate page merges single fields into nested objects of the shared document; the keyboard has to act on them.
@MainActor
final class CandidateOptionsSettingsTests: XCTestCase {
  func testMinimumPrefixRejectsFractionalAndBooleanValues() {
    XCTAssertNil(CandidateOptionsSettingsView.minimumPrefix(NSNumber(value: 2.5)))
    XCTAssertNil(CandidateOptionsSettingsView.minimumPrefix(NSNumber(value: true)))
    XCTAssertEqual(CandidateOptionsSettingsView.minimumPrefix(NSNumber(value: 4)), 4)
  }

  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-candidate-options-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// Writing one field of `mixed_input` leaves the object's other fields as stored.
  func testNestedWriteKeepsTheRestOfTheObject() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let before = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["mixed_input"] as? [String: Any])

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var mixed = $0["mixed_input"] as? [String: Any] ?? [:]
      mixed["emoji"] = true
      $0["mixed_input"] = mixed
    })

    let after = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["mixed_input"] as? [String: Any])
    XCTAssertEqual(after["emoji"] as? Bool, true)
    XCTAssertEqual(after["english"] as? Bool, before["english"] as? Bool)
    XCTAssertEqual((after["minimum_prefix"] as? NSNumber)?.intValue, (before["minimum_prefix"] as? NSNumber)?.intValue)
  }

  /// 符号混输缺键读作开；页面写入 `mixed_input.symbols` 后，键盘的会话在下次出现时按它出或不出符号。
  func testSymbolMixedInputDefaultsOnAndReachesTheLiveSession() async throws {
    XCTAssertTrue(CandidateOptionsSettingsView.symbolsEnabled([:]))
    XCTAssertFalse(CandidateOptionsSettingsView.symbolsEnabled(["symbols": false]))
    XCTAssertTrue(CandidateOptionsSettingsView.symbolsEnabled(["symbols": "false"]))

    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let initial = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["mixed_input"] as? [String: Any])
    XCTAssertNil(initial["symbols"], "the default stays out of the document")
    XCTAssertTrue(firstCandidates(bridge, "pingfangmi").contains("㎡"), "symbol candidates ship on")

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var mixed = $0["mixed_input"] as? [String: Any] ?? [:]
      mixed["symbols"] = false
      $0["mixed_input"] = mixed
    })
    let stored = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["mixed_input"] as? [String: Any])
    XCTAssertEqual(stored["symbols"] as? Bool, false)
    XCTAssertFalse(CandidateOptionsSettingsView.symbolsEnabled(stored))
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")
    XCTAssertFalse(firstCandidates(bridge, "pingfangmi").contains("㎡"))
  }

  /// Typo correction is enabled by default and an explicit false still reaches the live session.
  func testReloadHandsTypoCorrectionToTheLiveSession() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    // The engine's own autocorrect check: with the default enabled, gau leads with 挂 (gua).
    XCTAssertEqual(firstCandidates(bridge, "gau").first, "挂", "transposition correction ships on")

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var quanpin = $0["quanpin"] as? [String: Any] ?? [:]
      quanpin["autocorrect_transposition"] = false
      $0["quanpin"] = quanpin
    })
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")

    XCTAssertNotEqual(firstCandidates(bridge, "gau").first, "挂")
  }

  /// 「双拼预编辑」 turned off in the app makes the live session spell out the pinyin behind the raw shuangpin keys the strip otherwise shows.
  func testReloadHandsShuangpinPreeditToTheLiveSession() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switch(toShuangpin: true)
    XCTAssertEqual(spelling(bridge, "ui"), "ui", "raw keys ship on")

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["shuangpin_preedit_uses_raw"] = false
    })
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")

    XCTAssertEqual(spelling(bridge, "ui"), "shi")
  }

  /// 「候选栏预编辑」 hides only what is being spelled: the chosen half of a phrase and a local mode's name stay on the strip.
  func testCandidatePreeditStyleKeepsThePhrasePrefixAndModeName() {
    XCTAssertEqual(CandidatePreeditStyle(in: nil), .pinyin)
    XCTAssertEqual(CandidatePreeditStyle(in: [CandidatePreeditStyle.key: "empty"]), .empty)
    XCTAssertEqual(CandidatePreeditStyle(in: [CandidatePreeditStyle.key: "raw"]), .pinyin)
    XCTAssertEqual(CandidatePreeditStyle(in: [CandidatePreeditStyle.key: 1]), .pinyin)

    let pinyin = CandidatePreeditStyle.pinyin
    XCTAssertEqual(pinyin.title(composition: "水杉shu'ru", phrasePrefix: "水杉", localModeName: nil), "水杉shu'ru")
    let empty = CandidatePreeditStyle.empty
    XCTAssertEqual(empty.title(composition: "shu'ru", phrasePrefix: "", localModeName: nil), "")
    XCTAssertEqual(empty.title(composition: "水杉shu'ru", phrasePrefix: "水杉", localModeName: nil), "水杉")
    XCTAssertEqual(empty.title(composition: "Vrq", phrasePrefix: "", localModeName: "日期与时间"), "日期与时间")
  }

  /// 设置 App 关闭回退后，活着的键盘会话保留已选文字，直到剩余拼写删空后一次上屏。
  func testBackspaceSelectionSwitchReachesAnExistingComposition() async throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.cancel()
    for letter in "woshi" { _ = bridge.handleCharacter(String(letter)) }
    let all = try bridge.allCandidates()
    let candidates = try XCTUnwrap(all["candidates"] as? [[String: Any]])
    let candidate = try XCTUnwrap(candidates.first { $0["text"] as? String == "我" })
    let identity = try XCTUnwrap(candidate["id"] as? [String: Any])
    let generation = try XCTUnwrap(identity["generation"] as? NSNumber).uint64Value
    let index = try XCTUnwrap(identity["index"] as? NSNumber).uint64Value
    let selected = bridge.selectAnyCandidate(generation: generation, globalIndex: index)
    XCTAssertEqual(selected.phrasePrefix, "我")
    XCTAssertEqual(selected.editingText, "shi")
    XCTAssertNil(selected.commitText)

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["backspace_reverts_selection"] = false
    })
    let reloaded = expectation(description: "重新加载退格偏好")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted)
    for spelling in ["sh", "s"] {
      let edited = bridge.handleBackspace()
      XCTAssertEqual(edited.phrasePrefix, "我")
      XCTAssertEqual(edited.editingText, spelling)
      XCTAssertNil(edited.commitText)
    }
    let ended = bridge.handleBackspace()
    XCTAssertEqual(ended.commitText, "我")
    XCTAssertTrue(ended.phrasePrefix.isEmpty)
    XCTAssertTrue(ended.editingText.isEmpty)
    XCTAssertNil(bridge.handleBackspace().commitText)
  }

  /// The composition after typing `letters` from an empty one, which is then abandoned.
  private func spelling(_ bridge: MetasequoiaInputSessionBridge, _ letters: String) -> String {
    var snapshot = bridge.cancel()
    for letter in letters { snapshot = bridge.handleCharacter(String(letter)) }
    _ = bridge.cancel()
    return snapshot.preedit
  }

  /// The first few candidate texts after typing `letters` from an empty composition, which is then abandoned.
  private func firstCandidates(_ bridge: MetasequoiaInputSessionBridge, _ letters: String) -> [String] {
    var snapshot = bridge.cancel()
    for letter in letters { snapshot = bridge.handleCharacter(String(letter)) }
    let texts = snapshot.candidates.prefix(5).map { String($0.split(separator: "\n").first ?? "") }
    _ = bridge.cancel()
    return texts
  }
}

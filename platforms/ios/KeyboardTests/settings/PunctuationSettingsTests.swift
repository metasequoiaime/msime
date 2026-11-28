import XCTest

/// The settings app's punctuation page writes the shared document; the keyboard has to act on it.
@MainActor
final class PunctuationSettingsTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-punctuation-settings-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// The app reads and writes the document without creating a session.
  func testSettingsAppRoundTripsTheSharedDocument() throws {
    // The keyboard prepares the state root; the app only ever reads one that exists.
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let before = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(before["paired_punctuation"] as? Bool, true)
    XCTAssertEqual(before["punctuation_lock"] as? String, "follow")

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["paired_punctuation"] = false
      $0["punctuation_lock"] = "english"
    })
    let after = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(after["paired_punctuation"] as? Bool, false)
    XCTAssertEqual(after["punctuation_lock"] as? String, "english")
    // Nothing else moved.
    XCTAssertEqual(after["smart_punctuation"] as? Bool, before["smart_punctuation"] as? Bool)
  }

  /// A switch turned off in the app stops working the next time the keyboard appears, without waiting for a new session.
  ///
  /// The keyboard has already written its own preferences by then - it applies the learning switches every time it appears - so the document's revision is not what the session last counted to.
  func testReloadHandsPunctuationSwitchesToTheLiveSession() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNotEqual(bridge.handlePunctuationWithContext(",", preceding: 0x61).commitText, "，",
                      "direct-after-letter ships on, so a comma after a letter stays ASCII")
    XCTAssertTrue(bridge.setFuzzyPinyinRules(1))
    XCTAssertTrue(bridge.setFuzzyPinyinRules(0))
    XCTAssertTrue(bridge.setTraditionalChineseOutput(false))

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["smart_punctuation_direct_letter"] = false
    })
    await reload(bridge)

    XCTAssertEqual(bridge.handlePunctuationWithContext(",", preceding: 0x61).commitText, "，")
  }

  /// 设置页写下的引号与方头括号样式在键盘下次出现时生效：快照报告新样式，标点键按它出字。缺键时是弯引号、实心括号。
  func testPunctuationStylesReachTheLiveSession() async throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let before = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertNil(before["quote_style"], "the default is left out of the document")
    XCTAssertNil(before["bracket_style"])
    let curly = bridge.handlePunctuationWithContext("'", preceding: 0)
    XCTAssertEqual(curly.commitText, "‘")
    XCTAssertFalse(curly.cornerQuotes)
    _ = bridge.handlePunctuationWithContext("'", preceding: 0)

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["quote_style"] = "corner"
      $0["bracket_style"] = "hollow"
    })
    await reload(bridge)

    let corner = bridge.handlePunctuationWithContext("'", preceding: 0)
    XCTAssertEqual(corner.commitText, "「")
    XCTAssertTrue(corner.cornerQuotes)
    XCTAssertTrue(corner.hollowBrackets)
    XCTAssertEqual(bridge.handlePunctuationWithContext("[", preceding: 0).commitText, "〖")
  }

  /// 设置页写下的「保留英文的标点」（#11890）在键盘下次出现时生效：快照报告这些键，它们不再出中文标点，键面画 ASCII；其余键照旧。缺键时为空。
  func testPreservedEnglishKeysReachTheLiveSession() async throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let before = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertNil(before["preserve_english_punctuation"], "the empty default is left out of the document")
    XCTAssertEqual(bridge.handlePunctuationWithContext("`", preceding: 0).commitText, "·")

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["preserve_english_punctuation"] = ["`", "\\"]
    })
    await reload(bridge)

    let backtick = bridge.handlePunctuationWithContext("`", preceding: 0)
    XCTAssertFalse(backtick.isHandled)
    XCTAssertNil(backtick.commitText)
    XCTAssertEqual(backtick.preserveEnglishPunctuation, "`\\")
    XCTAssertFalse(bridge.handlePunctuationWithContext("\\", preceding: 0).isHandled)
    XCTAssertEqual(bridge.handlePunctuationWithContext("$", preceding: 0).commitText, "￥")
    XCTAssertEqual(KeyboardViewController.chineseSymbolFace(
      "\\", cornerQuotes: false, hollowBrackets: false, preserveEnglish: "`\\"), "\\")
    XCTAssertEqual(KeyboardViewController.chineseSymbolFace(
      ",", cornerQuotes: false, hollowBrackets: false, preserveEnglish: "`\\"), "，")
    // 不在契约里的键，保存会被拒绝，文档不变。
    XCTAssertFalse(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["preserve_english_punctuation"] = ["/"]
    })
    let after = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(after["preserve_english_punctuation"] as? [String], ["`", "\\"])
  }

  private func reload(_ bridge: MetasequoiaInputSessionBridge) async {
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { loaded in
      accepted = loaded
      reloaded.fulfill()
    }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")
  }
}

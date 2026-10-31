import XCTest

/// 成对标点自动补全: the keyboard writes the closing half of a pair the Engine opened.
final class PairedPunctuationTests: XCTestCase {
  func testCompletionFollowsTheOpeningMarkAtTheEndOfTheCommit() {
    XCTAssertEqual(PairedPunctuationPolicy.completion("你好（", enabled: true),
                   PairedPunctuationCompletion(opening: "(", closing: "）"))
    XCTAssertEqual(PairedPunctuationPolicy.completion("《", enabled: true)?.closing, "》")
    XCTAssertEqual(PairedPunctuationPolicy.completion("〈", enabled: true)?.opening, "<")
    XCTAssertEqual(PairedPunctuationPolicy.completion("【", enabled: true)?.closing, "】")
    XCTAssertEqual(PairedPunctuationPolicy.completion("“", enabled: true)?.closing, "”")
    XCTAssertNil(PairedPunctuationPolicy.completion("（", enabled: false))
    XCTAssertNil(PairedPunctuationPolicy.completion("）", enabled: true))
    XCTAssertNil(PairedPunctuationPolicy.completion("，", enabled: true))
    XCTAssertNil(PairedPunctuationPolicy.completion("", enabled: true))
    XCTAssertNil(PairedPunctuationPolicy.completion(nil, enabled: true))
  }

  func testAQuotePressAlwaysOpensAPairWhilePairingIsOn() {
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("”", ascii: "\"", enabled: true), "“")
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("好’", ascii: "'", enabled: true), "好‘")
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("”", ascii: "\"", enabled: false), "”")
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("）", ascii: ")", enabled: true), "）")
    XCTAssertNil(PairedPunctuationPolicy.reopenQuote(nil, ascii: "\"", enabled: true))
  }

  /// 直角引号与空心方头括号（`quote_style`、`bracket_style`）补全、改回前引号和跨过都与弯引号一样。
  func testCornerQuotesAndHollowBracketsPairLikeTheDefaults() {
    XCTAssertEqual(PairedPunctuationPolicy.completion("「", enabled: true),
                   PairedPunctuationCompletion(opening: "'", closing: "」"))
    XCTAssertEqual(PairedPunctuationPolicy.completion("你好『", enabled: true),
                   PairedPunctuationCompletion(opening: "\"", closing: "』"))
    XCTAssertEqual(PairedPunctuationPolicy.completion("〖", enabled: true),
                   PairedPunctuationCompletion(opening: "[", closing: "〗"))
    XCTAssertNil(PairedPunctuationPolicy.completion("」", enabled: true))
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("」", ascii: "'", enabled: true), "「")
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("好』", ascii: "\"", enabled: true), "好『")
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("』", ascii: "'", enabled: true), "』")
    XCTAssertEqual(PairedPunctuationPolicy.reopenQuote("」", ascii: "'", enabled: false), "」")

    var stack = PairedPunctuationStack()
    stack.push(closing: "〗", editor: 3)
    stack.push(closing: "』", editor: 3)
    stack.push(closing: "」", editor: 3)
    XCTAssertTrue(stack.stepOver(ascii: "'", editor: 3, following: "」』〗"))
    XCTAssertTrue(stack.stepOver(ascii: "\"", editor: 3, following: "』〗"))
    XCTAssertTrue(stack.stepOver(ascii: "]", editor: 3, following: "〗"))
    XCTAssertTrue(stack.isEmpty)
  }

  func testReplacingTheCommitKeepsThePunctuationMarks() {
    let snapshot = MetasequoiaInputSnapshot(isHandled: true, commitText: "」", cornerQuotes: true, hollowBrackets: true)
    let reopened = snapshot.replacingCommit("「")
    XCTAssertTrue(reopened.cornerQuotes)
    XCTAssertTrue(reopened.hollowBrackets)
  }

  func testBalancedBookTitlesKeepOpeningTheOuterMark() {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-paired-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.cancel()
    XCTAssertEqual(bridge.handlePunctuationWithContext("<", preceding: 0).commitText, "《")
    XCTAssertTrue(bridge.balancePairedPunctuationAfterAutoClose(opening: "<"))
    XCTAssertEqual(bridge.handlePunctuationWithContext("<", preceding: 0).commitText, "《",
                   "the auto-closed 》 ends the first title, so the next one is not nested")
    XCTAssertFalse(bridge.balancePairedPunctuationAfterAutoClose(opening: "("))
  }

  func testReplacingTheCommitKeepsTheRestOfTheSnapshot() {
    let snapshot = MetasequoiaInputSnapshot(isHandled: true, commitText: "”", preedit: "", candidates: ["a"], candidateHighlighted: [true])
    let reopened = snapshot.replacingCommit("“")
    XCTAssertEqual(reopened.commitText, "“")
    XCTAssertEqual(reopened, MetasequoiaInputSnapshot(isHandled: true, commitText: "“", preedit: "", candidates: ["a"], candidateHighlighted: [true]))
  }

  /// `title:"${cursor}"` finished with `(` commits `title:""（` with suffix `"（`; rendering it puts the caret between the quotes, so nothing is auto-closed there and a closing quote is not reopened.
  func testAnExplicitCursorTemplateLandsAsWritten() {
    let template = MetasequoiaInputSnapshot(isHandled: true, commitText: "title:\"\"（", commitCursorSuffix: "\"（")
    let positioned = PairedPunctuationPolicy.punctuationCommit(template, ascii: "(", enabled: true)
    XCTAssertEqual(positioned.snapshot, template)
    XCTAssertNil(positioned.completion)
    let end = MetasequoiaInputSnapshot(isHandled: true, commitText: "末尾（", commitCursorSuffix: "")
    XCTAssertNil(PairedPunctuationPolicy.punctuationCommit(end, ascii: "(", enabled: true).completion)
    let quote = MetasequoiaInputSnapshot(isHandled: true, commitText: "好”", commitCursorSuffix: "”")
    let kept = PairedPunctuationPolicy.punctuationCommit(quote, ascii: "\"", enabled: true)
    XCTAssertEqual(kept.snapshot, quote)
    XCTAssertNil(kept.completion)
    // Without a cursor request the key reopens and closes the pair as before.
    let plain = PairedPunctuationPolicy.punctuationCommit(
      MetasequoiaInputSnapshot(isHandled: true, commitText: "好”"), ascii: "\"", enabled: true)
    XCTAssertEqual(plain.snapshot.commitText, "好“")
    XCTAssertEqual(plain.completion, PairedPunctuationCompletion(opening: "\"", closing: "”"))
    let off = PairedPunctuationPolicy.punctuationCommit(
      MetasequoiaInputSnapshot(isHandled: true, commitText: "你好（"), ascii: "(", enabled: false)
    XCTAssertEqual(off.snapshot.commitText, "你好（")
    XCTAssertNil(off.completion)
  }

  func testTheClosingKeyStepsOverTheInnermostClosedPair() {
    var stack = PairedPunctuationStack()
    stack.push(closing: "》", editor: 7)
    stack.push(closing: "”", editor: 7)
    XCTAssertFalse(stack.stepOver(ascii: ",", editor: 7, following: "”》"), "typing inside a pair keeps it")
    XCTAssertTrue(stack.stepOver(ascii: "\"", editor: 7, following: "”》"))
    XCTAssertTrue(stack.stepOver(ascii: ">", editor: 7, following: "》"))
    XCTAssertTrue(stack.isEmpty)
    XCTAssertFalse(stack.stepOver(ascii: ">", editor: 7, following: "》"), "nothing left to step over")

    stack.push(closing: "〉", editor: 7)
    XCTAssertTrue(stack.stepOver(ascii: ">", editor: 7, following: "〉"), "> closes a nested title too")
    stack.push(closing: "’", editor: 7)
    XCTAssertTrue(stack.stepOver(ascii: "'", editor: 7, following: nil), "a host that reports nothing leaves the stack as the evidence")
    stack.push(closing: "】", editor: 7)
    XCTAssertTrue(stack.stepOver(ascii: "]", editor: 7, following: "】abc"))
  }

  func testAPairThatIsNoLongerThereIsForgotten() {
    var stack = PairedPunctuationStack()
    stack.push(closing: "）", editor: 7)
    stack.push(closing: "）", editor: 7)
    XCTAssertFalse(stack.stepOver(ascii: ")", editor: 7, following: ""), "the caret is at the end, the closing half is gone")
    XCTAssertTrue(stack.isEmpty)

    stack.push(closing: "）", editor: 7)
    XCTAssertFalse(stack.stepOver(ascii: ")", editor: 8, following: "）"), "another editor")
    XCTAssertTrue(stack.isEmpty)

    stack.push(closing: "）", editor: 7)
    XCTAssertFalse(stack.stepOver(ascii: "]", editor: 7, following: "）"), "the wrong closing key ends the tracking")
    XCTAssertTrue(stack.isEmpty)

    stack.push(closing: "）", editor: 0)
    XCTAssertTrue(stack.isEmpty, "no document, nothing to be sure about")
  }

  func testTheStackKeepsTheInnermostSixteen() {
    var stack = PairedPunctuationStack()
    stack.push(closing: "】", editor: 1)
    for _ in 0..<PairedPunctuationStack.limit { stack.push(closing: "）", editor: 1) }
    for _ in 0..<PairedPunctuationStack.limit {
      XCTAssertTrue(stack.stepOver(ascii: ")", editor: 1, following: "）"))
    }
    XCTAssertTrue(stack.isEmpty, "the outermost 】 was dropped")
    XCTAssertEqual(PairedPunctuationStack.limit, 16)
  }
}

import XCTest

final class EnglishSuggestionPolicyTests: XCTestCase {
  @MainActor
  func testSharedCorrectionReturnsOriginalAndOnlyOneAlternative() {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    for (raw, corrected) in [("whatisthat", "what is that"), ("languaage", "language"),
                             ("languge", "language"), ("langiage", "language"),
                             ("imchinese", "I'm Chinese"), ("speskebglish", "speak English"),
                             ("i", "I"), ("youre", "you're")] {
      XCTAssertEqual(bridge.englishCorrections(forPrefix: raw), [raw, corrected])
    }
    for raw in ["language", "hello", "you're", "omg", "thx"] {
      XCTAssertEqual(bridge.englishCorrections(forPrefix: raw), [raw])
    }
  }

  /// 逐字母的英文建议在输入线程上算：结果与同步查询一致，快速连打时只送回最后一次请求，按空格时同步查询取到的仍是同一个结果。
  @MainActor
  func testAsynchronousCorrectionMatchesSynchronousAndDeliversOnlyTheLatestRequest() {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    var delivered: [[String]] = []
    let arrived = expectation(description: "latest correction")
    bridge.englishCorrections(forPrefix: "languge") { delivered.append($0) }
    bridge.englishCorrections(forPrefix: "langiage") { items in
      XCTAssertTrue(Thread.isMainThread)
      delivered.append(items)
      arrived.fulfill()
    }
    wait(for: [arrived], timeout: 10)
    XCTAssertEqual(delivered, [["langiage", "language"]])
    XCTAssertEqual(bridge.englishCorrections(forPrefix: "langiage"), ["langiage", "language"])
    XCTAssertEqual(bridge.englishCorrections(forPrefix: "languge"), ["languge", "language"])
  }

  @MainActor
  func testCandidateBarDisplayFollowsSharedCorrectionRule() {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    // 有纠错时只有原文与纠错。
    let corrected = bridge.englishSuggestions(forPrefix: "languge")
    XCTAssertEqual(corrected.items, ["languge", "language"])
    XCTAssertTrue(corrected.corrected)
    // 原文已是完整的词时只有原文。
    for raw in ["language", "hello", "you're", "omg"] {
      let display = bridge.englishSuggestions(forPrefix: raw)
      XCTAssertEqual(display.items, [raw])
      XCTAssertFalse(display.corrected)
    }
    // 只是前缀时原文后接补全；空格和标点边界的纠错查询不带补全，不会把前缀扩写成补全。
    let completions = bridge.englishSuggestions(forPrefix: "eng")
    XCTAssertFalse(completions.corrected)
    XCTAssertEqual(completions.items.first, "eng")
    XCTAssertTrue(completions.items.contains("English"))
    XCTAssertLessThanOrEqual(completions.items.count, MetasequoiaInputSessionBridge.englishSuggestionDisplayLimit)
    XCTAssertTrue(completions.items.dropFirst().allSatisfy { $0.lowercased().hasPrefix("eng") })
    XCTAssertEqual(bridge.englishCorrections(forPrefix: "eng"), ["eng"])
  }

  @MainActor
  func testAsynchronousSuggestionsKeepCompletionsAndCorrectionState() {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let completions = expectation(description: "prefix completions")
    bridge.englishSuggestions(forPrefix: "eng") { result in
      XCTAssertTrue(Thread.isMainThread)
      XCTAssertFalse(result.corrected)
      XCTAssertEqual(result.items.first, "eng")
      XCTAssertTrue(result.items.contains("English"))
      completions.fulfill()
    }
    wait(for: [completions], timeout: 10)
    XCTAssertEqual(bridge.englishCorrections(forPrefix: "eng"), ["eng"])
    let corrected = expectation(description: "latest corrected suggestions")
    bridge.englishSuggestions(forPrefix: "eng") { _ in XCTFail("过期请求不能回调") }
    bridge.englishSuggestions(forPrefix: "languge") { result in
      XCTAssertTrue(result.corrected)
      XCTAssertEqual(result.items, ["languge", "language"])
      corrected.fulfill()
    }
    wait(for: [corrected], timeout: 10)
    XCTAssertEqual(bridge.englishCorrections(forPrefix: "languge"), ["languge", "language"])
  }

  func testWordStopsAtNonAsciiLettersAndBoundaries() {
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "我用iph"), "iph")
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "こんにちはhel"), "hel")
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "hello wor"), "wor")
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "you're"), "you're")
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "done. "), "")
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "version2be"), "be")
  }

  func testWordNormalizesFullWidthLatinLetters() {
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "中文ｉｐｈ"), "iph")
    XCTAssertEqual(EnglishSuggestionPolicy.currentWord(before: "中文Ｈｅ"), "He")
  }

  func testReplacesTypedPrefixAndRestoresInitialCapital() {
    XCTAssertEqual(
      EnglishSuggestionPolicy.replacement(typed: "He", candidate: "hello", startedCapitalized: true),
      .init(deleteCount: 2, insert: "Hello"))
  }

  func testIdenticalCandidateDoesNotCreateAnEdit() {
    XCTAssertNil(EnglishSuggestionPolicy.replacement(typed: "hello", candidate: "hello", startedCapitalized: false))
  }

  func testLowercasePrefixStaysLowercase() {
    XCTAssertEqual(
      EnglishSuggestionPolicy.replacement(typed: "he", candidate: "hello", startedCapitalized: false),
      .init(deleteCount: 2, insert: "hello"))
  }

  func testFullWidthPrefixKeepsInitialCapitalSemanticsAndCharacterCount() {
    let typed = EnglishSuggestionPolicy.currentWord(before: "中文Ｈｅ")
    XCTAssertEqual(
      EnglishSuggestionPolicy.replacement(typed: typed, candidate: "hello", startedCapitalized: typed.first?.isUppercase ?? false),
      .init(deleteCount: 2, insert: "Hello"))
  }
  func testSplitCorrectionReplacesTheWholeTypedWord() {
    XCTAssertEqual(EnglishSuggestionPolicy.replacement(
      typed: "imchinese", candidate: "I'm Chinese", startedCapitalized: false),
      .init(deleteCount: 9, insert: "I'm Chinese"))
  }
}

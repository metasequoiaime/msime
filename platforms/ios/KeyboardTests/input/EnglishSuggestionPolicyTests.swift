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

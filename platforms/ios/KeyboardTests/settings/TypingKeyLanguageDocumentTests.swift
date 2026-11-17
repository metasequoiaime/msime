import XCTest

final class TypingKeyLanguageDocumentTests: XCTestCase {
  func testSharedLanguageBucketsReachTheNativeFilter() throws {
    let data = Data("""
    {"dailyKeys":{"2026-11-10":{"KeyA":5,"Space":2}},
     "dailyKeyLanguages":{"2026-11-10":{"english":{"KeyA":3},"unknown":{"KeyA":2},"common":{"Space":2}}}}
    """.utf8)
    let document = try JSONDecoder().decode(TypingStatistics.self, from: data)
    let languages = TypingKeyLanguages(dailyKeys: document.dailyKeys,
      dailyLanguages: document.dailyKeyLanguages, days: ["2026-11-10"])
    XCTAssertEqual(languages.available, [.all, .english, .common, .unknown])
    XCTAssertEqual(languages.keys(for: .all), ["KeyA": 5, "Space": 2])
    XCTAssertEqual(languages.keys(for: .english), ["KeyA": 3])
    XCTAssertEqual(languages.keys(for: .common), ["Space": 2])
    XCTAssertFalse(document.enabled)
  }

  func testOlderDocumentKeepsTheCompleteTotal() throws {
    let document = try JSONDecoder().decode(TypingStatistics.self,
      from: Data(#"{"dailyKeys":{"2026-11-10":{"KeyA":5}}}"#.utf8))
    XCTAssertTrue(document.dailyKeyLanguages.isEmpty)
    let languages = TypingKeyLanguages(dailyKeys: document.dailyKeys,
      dailyLanguages: document.dailyKeyLanguages, days: ["2026-11-10"])
    XCTAssertEqual(languages.available, [.all])
    XCTAssertEqual(languages.keys(for: .english), ["KeyA": 5])
  }

  func testMalformedBucketDoesNotLookLikeEmptyHistory() {
    XCTAssertThrowsError(try JSONDecoder().decode(TypingStatistics.self,
      from: Data(#"{"dailyKeyLanguages":{"2026-11-10":{"english":{"KeyA":"5"}}}}"#.utf8)))
  }
}

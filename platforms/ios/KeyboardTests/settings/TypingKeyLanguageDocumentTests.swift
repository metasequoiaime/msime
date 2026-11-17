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

  func testHeatmapTotalSaturatesWithoutDistortingPercentages() {
    let ids = Array(TypingKeyID.all.prefix(147))
    XCTAssertEqual(ids.count, 147)
    let perKey = 63_000_000_000_000_000
    let counts = Dictionary(uniqueKeysWithValues: ids.map { ($0, perKey) })
    let heatmap = TypingKeyHeatmap(counts: counts)
    XCTAssertEqual(heatmap.total, Int.max)
    XCTAssertEqual(heatmap.percent(ids[0]), 100.0 / 147, accuracy: 1e-12)
    XCTAssertEqual(ids.reduce(0.0) { $0 + heatmap.percent($1) }, 100, accuracy: 1e-10)
    XCTAssertEqual(heatmap.level(ids[0]), 1)
    let ordinary = TypingKeyHeatmap(counts: ["KeyA": 3, "Space": 1, "KeyB": -1])
    XCTAssertEqual(ordinary.total, 4)
    XCTAssertEqual(ordinary.percent("KeyA"), 75)
    XCTAssertEqual(TypingKeyHeatmap(counts: [:]).percent("KeyA"), 0)
  }

  func testMalformedBucketDoesNotLookLikeEmptyHistory() {
    XCTAssertThrowsError(try JSONDecoder().decode(TypingStatistics.self,
      from: Data(#"{"dailyKeyLanguages":{"2026-11-10":{"english":{"KeyA":"5"}}}}"#.utf8)))
  }
}

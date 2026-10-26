import Foundation
import XCTest

final class CandidateGlossModelTests: XCTestCase {
  func testRequestPreservesGenerationAndCandidateSources() throws {
    let data = try CandidateGlossModel.request(generation: 7, candidates: [
      ["text": "你好", "source": 0],
      ["text": "hello", "source": 1],
    ])
    let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    XCTAssertEqual((object["generation"] as? NSNumber)?.uint64Value, 7)
    let candidates = try XCTUnwrap(object["candidates"] as? [[String: Any]])
    XCTAssertEqual(candidates[0]["text"] as? String, "你好")
    XCTAssertEqual((candidates[1]["source"] as? NSNumber)?.intValue, 1)
  }

  func testATargetLanguageReachesTheRequestAsHostAPINamesIt() throws {
    let data = try CandidateGlossModel.request(generation: 7, candidates: [["text": "你好", "source": 0]],
                                               targetLanguage: "FR")
    let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    XCTAssertEqual(object["target_language"] as? String, "fr")
    let english = try CandidateGlossModel.request(generation: 7, candidates: [["text": "你好", "source": 0]])
    XCTAssertNil((try JSONSerialization.jsonObject(with: english) as? [String: Any])?["target_language"])
    XCTAssertThrowsError(try CandidateGlossModel.request(
      generation: 7, candidates: [["text": "你好", "source": 0]], targetLanguage: "EN"),
      "English is the default dictionary, never an offline target")
  }

  func testOfflineGlossLanguagesAreTheDictionariesBesideTheResources() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = root.appendingPathComponent("EngineResources", isDirectory: true)
    let glosses = root.appendingPathComponent("offline-glosses", isDirectory: true)
    try FileManager.default.createDirectory(at: resources, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: glosses, withIntermediateDirectories: true)
    XCTAssertEqual(CandidateTranslationPreference.offlineGlossLanguages(resources: resources.path), [])
    try Data().write(to: glosses.appendingPathComponent("zh-fr.db"))
    try Data().write(to: glosses.appendingPathComponent("zh-ja.db"))
    try Data().write(to: glosses.appendingPathComponent("zh-en.db"))
    XCTAssertEqual(CandidateTranslationPreference.offlineGlossLanguages(resources: resources.path), ["FR", "JA"])
    XCTAssertEqual(CandidateTranslationPreference.offlineGlossLanguages(resources: nil), [])
    let french = CandidateTranslationPreference.languages[4]
    XCTAssertTrue(CandidateTranslationPreference.needsNetwork(french))
    XCTAssertFalse(CandidateTranslationPreference.needsNetwork(french, offline: ["FR"]))
  }

  func testDecodeRejectsUnboundedTranslationEntries() throws {
    let value: [String: Any] = [
      "generation": 7,
      "translations": [["text": "你好", "translation": String(repeating: "x", count: 4097)]],
    ]
    XCTAssertThrowsError(try CandidateGlossModel.decode(value))
  }

  /// 离线释义 worker 的回答交给 `applyTranslations` 时每条都标 `"gloss": true`：候选词翻译的范围是「仅当前高亮」时，共享运行时据此把它们当作不受范围限制的离线释义，每一行都画，而不是只画在高亮行。
  func testDecodedOfflineGlossesAreMarkedAsGlosses() throws {
    let decoded = try CandidateGlossModel.decode([
      "generation": 7,
      "translations": [
        ["text": "你好", "translation": "hello"],
        ["text": "泥嚎", "translation": "mud howl"],
      ],
    ])
    XCTAssertEqual(decoded.generation, 7)
    let entries = try XCTUnwrap(JSONSerialization.jsonObject(with: decoded.translations) as? [[String: Any]])
    XCTAssertEqual(entries.map { $0["text"] as? String }, ["你好", "泥嚎"])
    XCTAssertEqual(entries.map { $0["translation"] as? String }, ["hello", "mud howl"])
    XCTAssertEqual(entries.map { $0["gloss"] as? Bool }, [true, true])
    let empty = try CandidateGlossModel.decode(["generation": 7, "translations": [[String: Any]]()])
    XCTAssertEqual(empty.translations, Data("[]".utf8), "空回答仍是空数组，键盘照旧不交给会话")
  }

  func testDecodeRejectsNegativeGeneration() {
    let value: [String: Any] = [
      "generation": -1,
      "translations": [["text": "你好", "translation": "hello"]],
    ]
    XCTAssertThrowsError(try CandidateGlossModel.decode(value))
  }

  func testIntegerValueRejectsLossyGenerationNumbers() {
    XCTAssertEqual(CandidateGlossModel.integerValue(NSNumber(value: 7), maximum: UInt64.max), 7)
    XCTAssertNil(CandidateGlossModel.integerValue(NSNumber(value: 7.5), maximum: UInt64.max))
    XCTAssertNil(CandidateGlossModel.integerValue(NSNumber(value: true), maximum: UInt64.max))
    XCTAssertNil(CandidateGlossModel.integerValue(NSNumber(value: -1), maximum: UInt64.max))
  }
}

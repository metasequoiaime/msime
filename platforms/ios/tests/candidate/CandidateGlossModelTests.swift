import Foundation
import SQLite3
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

  // MARK: - 候选读音

  private func key(_ language: String, _ text: String) -> String { CandidatePronunciationModel.key(language: language, text: text) }

  func testPronunciationLookupReadsEnglishCandidatesAndEachGlossInItsLanguage() {
    XCTAssertEqual(CandidatePronunciationModel.lookupText(candidate: "hello", gloss: nil, language: "en"), key("en", "hello"))
    XCTAssertEqual(CandidatePronunciationModel.lookupText(candidate: "well-known", gloss: "x", language: "fr"), key("en", "well-known"),
                   "an English candidate is read as itself, not through its gloss")
    XCTAssertEqual(CandidatePronunciationModel.lookupText(candidate: "你好", gloss: " hello; hi\nsecond ", language: "en"),
                   key("en", "hello; hi"), "a Chinese candidate is read through the first line of its English gloss")
    XCTAssertEqual(CandidatePronunciationModel.lookupText(candidate: "猫", gloss: "chat, chatte", language: "fr"),
                   key("fr", "chat, chatte"), "a French gloss is read as French, not English")
    XCTAssertEqual(CandidatePronunciationModel.lookupText(candidate: "猫", gloss: "猫", language: "ja"), key("ja", "猫"))
    XCTAssertNil(CandidatePronunciationModel.lookupText(candidate: "猫", gloss: "chat", language: "zh"),
                 "a language without a table is not looked up")
    XCTAssertNil(CandidatePronunciationModel.lookupText(candidate: "你好", gloss: nil, language: "en"))
    XCTAssertNil(CandidatePronunciationModel.lookupText(candidate: "你好", gloss: CandidatePronunciationModelTestsPlaceholder.pending,
                                                        language: "en"),
                 "the pending placeholder is not a gloss")
    XCTAssertNil(CandidatePronunciationModel.lookupText(candidate: "AI助手", gloss: nil, language: "en"))
    XCTAssertNil(CandidatePronunciationModel.lookupText(candidate: "爱", gloss: "to love\taffection", language: "en"),
                 "the shared request rejects a whole batch over one control character, so such a gloss is not looked up")
  }

  func testPronunciationRequestDeduplicatesAndCarriesThePackRoot() throws {
    let data = try CandidatePronunciationModel.request(keys: [key("en", "hello"), "", key("en", "hello"), key("en", "world"),
                                                              key("fr", "chat"), key("zh", "猫")],
                                                       supplementaryResources: "/synthetic/state/resource-packs")
    let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    XCTAssertEqual((object["generation"] as? NSNumber)?.intValue, 0)
    XCTAssertEqual(object["supplementary_resources"] as? String, "/synthetic/state/resource-packs")
    let items = try XCTUnwrap(object["items"] as? [[String: String]])
    XCTAssertEqual(items, [["text": "hello", "language": "en"], ["text": "world", "language": "en"],
                           ["text": "chat", "language": "fr"]])
    let many = try CandidatePronunciationModel.request(keys: (0..<400).map { key("en", "word\($0)") }, supplementaryResources: nil)
    let manyItems = try XCTUnwrap((JSONSerialization.jsonObject(with: many) as? [String: Any])?["items"] as? [Any])
    XCTAssertEqual(manyItems.count, CandidatePronunciationModel.maxItems)
    let clean = try CandidatePronunciationModel.request(keys: [key("en", "love\taffection"), key("en", "hello")],
                                                        supplementaryResources: nil)
    let cleanItems = try XCTUnwrap((JSONSerialization.jsonObject(with: clean) as? [String: Any])?["items"] as? [[String: String]])
    XCTAssertEqual(cleanItems, [["text": "hello", "language": "en"]], "a text with a control character is left out of the batch")
    XCTAssertThrowsError(try CandidatePronunciationModel.request(keys: [""], supplementaryResources: nil))
    XCTAssertThrowsError(try CandidatePronunciationModel.request(keys: [key("en", "hello")], supplementaryResources: "relative"))
  }

  func testPronunciationDecodeKeepsEachLanguagesReadingShape() throws {
    let readings = try CandidatePronunciationModel.decode(["generation": 0, "pronunciations": [
      ["text": "hello", "language": "en", "pronunciation": "/həˈləʊ/"],
      ["text": "bad", "language": "en", "pronunciation": "həˈləʊ"],
      ["text": "bracketed", "language": "en", "pronunciation": "[x]"],
      ["text": "bonjour", "language": "fr", "pronunciation": "/bɔ̃.ʒuʁ/"],
      ["text": "кошка", "language": "ru", "pronunciation": "[ˈkoʂkə]"],
      ["text": "猫", "language": "ja", "pronunciation": "neko"],
      ["text": "猫", "language": "zh", "pronunciation": "māo"],
      ["text": "", "language": "en", "pronunciation": "/x/"],
    ]])
    XCTAssertEqual(readings, [key("en", "hello"): "/həˈləʊ/", key("fr", "bonjour"): "/bɔ̃.ʒuʁ/",
                              key("ru", "кошка"): "[ˈkoʂkə]", key("ja", "猫"): "neko"])
    XCTAssertThrowsError(try CandidatePronunciationModel.decode(["generation": 0]))
    XCTAssertEqual(CandidatePronunciationModel.accessibilityLabel("[ˈkoʂkə]"), "音标 [ˈkoʂkə]")
    XCTAssertEqual(CandidatePronunciationModel.accessibilityLabel("neko"), "读音 neko")
  }

  /// 键盘经共享层读下载的资源包：资源目录旁没有表时，从 `<state_root>/resource-packs/pronunciations/` 读到合成的表。
  func testPronunciationsComeFromTheDownloadedPack() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = root.appendingPathComponent("EngineResources", isDirectory: true)
    let state = root.appendingPathComponent("state", isDirectory: true)
    try FileManager.default.createDirectory(at: resources, withIntermediateDirectories: true)
    XCTAssertFalse(CandidatePronunciationPreference.tableInstalled(resources: resources.path, stateRoot: state.path))
    let pack = URL(fileURLWithPath: CandidatePronunciationPreference.supplementaryResources(stateRoot: state.path))
      .appendingPathComponent("pronunciations", isDirectory: true)
    try FileManager.default.createDirectory(at: pack, withIntermediateDirectories: true)
    var database: OpaquePointer?
    XCTAssertEqual(sqlite3_open(pack.appendingPathComponent("en-phonetic.db").path, &database), SQLITE_OK)
    XCTAssertEqual(sqlite3_exec(database, """
      CREATE TABLE en_phonetics(word TEXT PRIMARY KEY, phonetic TEXT NOT NULL) WITHOUT ROWID;
      CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
      INSERT INTO meta VALUES('kind', 'en_phonetic');
      INSERT INTO en_phonetics VALUES('hello', 'həˈləʊ');
      INSERT INTO en_phonetics VALUES('love', 'lʌv');
      PRAGMA user_version = 1;
      """, nil, nil, nil), SQLITE_OK)
    sqlite3_close(database)
    XCTAssertTrue(CandidatePronunciationPreference.tableInstalled(resources: resources.path, stateRoot: state.path))
    let request = try CandidatePronunciationModel.request(
      keys: [key("en", "hello"), key("en", "to love"), key("en", "unknown")],
      supplementaryResources: CandidatePronunciationPreference.supplementaryResources(stateRoot: state.path))
    let response = try MetasequoiaInputSessionBridge.candidatePronunciations(request: request, resources: resources.path)
    XCTAssertEqual(try CandidatePronunciationModel.decode(response), [key("en", "hello"): "/həˈləʊ/", key("en", "to love"): "/lʌv/"])
  }

  /// 外语读音表在另一个资源包里：只有它、没有英文表时，主目标语言是法语也算装好了，法语释义经共享层从它读。
  func testTargetReadingsComeFromTheirOwnPack() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = root.appendingPathComponent("EngineResources", isDirectory: true)
    let state = root.appendingPathComponent("state", isDirectory: true)
    try FileManager.default.createDirectory(at: resources, withIntermediateDirectories: true)
    let pack = URL(fileURLWithPath: CandidatePronunciationPreference.supplementaryResources(stateRoot: state.path))
      .appendingPathComponent(CandidatePronunciationPreference.targetPackID, isDirectory: true)
    try FileManager.default.createDirectory(at: pack, withIntermediateDirectories: true)
    var database: OpaquePointer?
    XCTAssertEqual(sqlite3_open(pack.appendingPathComponent("fr-phonetic.db").path, &database), SQLITE_OK)
    XCTAssertEqual(sqlite3_exec(database, """
      CREATE TABLE fr_phonetics(word TEXT PRIMARY KEY, phonetic TEXT NOT NULL) WITHOUT ROWID;
      CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
      INSERT INTO meta VALUES('kind', 'fr_phonetic');
      INSERT INTO fr_phonetics VALUES('chat', '/ʃa/');
      PRAGMA user_version = 1;
      """, nil, nil, nil), SQLITE_OK)
    sqlite3_close(database)
    XCTAssertFalse(CandidatePronunciationPreference.tableInstalled(resources: resources.path, stateRoot: state.path))
    XCTAssertFalse(CandidatePronunciationPreference.tableInstalled(resources: resources.path, stateRoot: state.path, language: "de"))
    XCTAssertTrue(CandidatePronunciationPreference.tableInstalled(resources: resources.path, stateRoot: state.path, language: "fr"))
    let request = try CandidatePronunciationModel.request(
      keys: [key("fr", "chat, chatte"), key("en", "chat")],
      supplementaryResources: CandidatePronunciationPreference.supplementaryResources(stateRoot: state.path))
    let response = try MetasequoiaInputSessionBridge.candidatePronunciations(request: request, resources: resources.path)
    XCTAssertEqual(try CandidatePronunciationModel.decode(response), [key("fr", "chat, chatte"): "/ʃa/"])
  }

  func testPronunciationIsAHabitSettingThatDefaultsOff() {
    var document: [String: Any] = [:]
    var settings = InputHabitPreference.settings(in: document, fallback: InputHabitSettings(
      learning: true, frequencyMode: .halve, triggerCount: 3, linearStep: 4, glossEnabled: true,
      onlineTranslations: false, primaryLanguage: 0, secondaryLanguage: -1))
    XCTAssertFalse(settings.pronunciationEnabled, "the shared default is off")
    settings.pronunciationEnabled = true
    InputHabitPreference.write(settings, into: &document)
    XCTAssertEqual(document["candidate_pronunciation"] as? Bool, true)
    XCTAssertTrue(InputHabitPreference.settings(in: ["candidate_pronunciation": true], fallback: settings).pronunciationEnabled)
    XCTAssertFalse(InputHabitPreference.settings(in: ["candidate_pronunciation": false], fallback: settings).pronunciationEnabled)
  }
}

private enum CandidatePronunciationModelTestsPlaceholder {
  /// 候选条给还没到的释义留的占位（`KeyboardViewController.pendingGlossPlaceholder`）。
  static let pending = " "
}

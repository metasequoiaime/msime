import XCTest

/// 「输入习惯」 writes the shared document the keyboard mirrors from, so an App edit survives the keyboard's next reload.
final class InputHabitPreferenceTests: XCTestCase {
  private var state: URL!
  private var saved: InputHabitSettings!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-input-habit-\(UUID().uuidString)", isDirectory: true)
    saved = InputHabitPreference.mirrored
  }

  override func tearDown() {
    InputHabitPreference.mirror(saved)
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testDefaultsComeFromTheSharedDocument() {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let settings = InputHabitPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertTrue(settings.learning)
    XCTAssertEqual(settings.frequencyMode, .promote)
    XCTAssertEqual(settings.primaryLanguage, 0, "English")
    XCTAssertEqual(settings.secondaryLanguage, -1)
  }

  func testUpdateWritesTheDocumentAndMirrorsTheAppGroup() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let saved = try XCTUnwrap(InputHabitPreference.update(stateRoot: state) {
      $0.learning = false
      $0.frequencyMode = .disabled
      $0.triggerCount = 10
      $0.linearStep = 7
      $0.onlineTranslations = false
      $0.primaryLanguage = 6
      $0.secondaryLanguage = 1
    })
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["learning"] as? Bool, false)
    let frequency = try XCTUnwrap(document["frequency"] as? [String: Any])
    XCTAssertEqual(frequency["mode"] as? String, "disabled")
    XCTAssertEqual((frequency["trigger_count"] as? NSNumber)?.intValue, 10)
    XCTAssertEqual((frequency["linear_step"] as? NSNumber)?.intValue, 7)
    XCTAssertEqual(document["candidate_translations"] as? Bool, false)
    XCTAssertEqual(document["translation_target_language"] as? String, "ru")
    XCTAssertEqual(document["translation_secondary_language"] as? String, "ja")
    XCTAssertEqual(InputHabitPreference.settings(in: document), saved)

    XCTAssertFalse(DictionaryLearningPreference.enabled)
    XCTAssertEqual(FrequencyAdjustmentPreference.mode, .disabled)
    XCTAssertEqual(FrequencyAdjustmentPreference.triggerCount, 10)
    XCTAssertEqual(CandidateTranslationPreference.primary.code, "RU")
    XCTAssertEqual(CandidateTranslationPreference.secondary?.code, "JA")
  }

  func testNoSecondLanguageRemovesTheKeyAndTheSameLanguageTwiceIsNone() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNotNil(InputHabitPreference.update(stateRoot: state) { $0.secondaryLanguage = 2 })
    let same = try XCTUnwrap(InputHabitPreference.update(stateRoot: state) { $0.secondaryLanguage = $0.primaryLanguage })
    XCTAssertEqual(same.secondaryLanguage, -1)
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertNil(document["translation_secondary_language"])
  }

  func testOutOfRangeAndUnknownValuesKeepTheFallback() {
    let fallback = InputHabitSettings(learning: true, frequencyMode: .halve, triggerCount: 3, linearStep: 4,
                                      glossEnabled: true, onlineTranslations: true, primaryLanguage: 1, secondaryLanguage: 2)
    let document: [String: Any] = [
      "frequency": ["mode": "sideways", "trigger_count": 11, "linear_step": 0],
      "translation_target_language": "xx",
      "translation_secondary_language": "xx",
    ]
    let settings = InputHabitPreference.settings(in: document, fallback: fallback)
    XCTAssertEqual(settings.frequencyMode, .halve)
    XCTAssertEqual(settings.triggerCount, 3)
    XCTAssertEqual(settings.linearStep, 4)
    XCTAssertEqual(settings.primaryLanguage, 1)
    XCTAssertEqual(settings.secondaryLanguage, -1, "an unknown second language shows as none, as the keyboard mirrors it")
  }

  func testFractionalAndBooleanCountsKeepTheFallback() {
    let fallback = InputHabitSettings(learning: true, frequencyMode: .halve, triggerCount: 3, linearStep: 4,
                                      glossEnabled: true, onlineTranslations: true, primaryLanguage: 1, secondaryLanguage: 2)
    let document: [String: Any] = [
      "frequency": ["trigger_count": NSNumber(value: 2.5), "linear_step": NSNumber(value: true)],
    ]
    let settings = InputHabitPreference.settings(in: document, fallback: fallback)
    XCTAssertEqual(settings.triggerCount, 3)
    XCTAssertEqual(settings.linearStep, 4)
  }

  func testFrequencyWriteKeepsFieldsThePageDoesNotShow() throws {
    var document: [String: Any] = ["frequency": ["mode": "pin", "trigger_count": 2, "linear_step": 2, "future": true]]
    InputHabitPreference.write(InputHabitPreference.settings(in: document), into: &document)
    XCTAssertEqual((document["frequency"] as? [String: Any])?["future"] as? Bool, true)
  }
}

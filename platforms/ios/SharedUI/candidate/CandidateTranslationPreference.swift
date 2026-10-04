import Foundation

struct CandidateTranslationLanguage: Equatable, Sendable {
  let title: String
  let code: String
}

enum CandidateTranslationPreference {
  static let primaryKey = "candidate.translationLanguage"
  static let secondaryKey = "candidate.translationSecondaryLanguage"
  static let onlineKey = "candidate.translationOnline"
  static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }
  static let languages = [
    CandidateTranslationLanguage(title: "英语", code: "EN"),
    CandidateTranslationLanguage(title: "日语", code: "JA"),
    CandidateTranslationLanguage(title: "韩语", code: "KO"),
    CandidateTranslationLanguage(title: "西班牙语", code: "ES"),
    CandidateTranslationLanguage(title: "法语", code: "FR"),
    CandidateTranslationLanguage(title: "德语", code: "DE"),
    // Appended rather than sorted in: the App Group stores positions in this list.
    CandidateTranslationLanguage(title: "俄语", code: "RU"),
  ]
  static func language(at index: Int) -> CandidateTranslationLanguage {
    languages.indices.contains(index) ? languages[index] : languages[0]
  }
  static var primaryIndex: Int {
    get { languageIndex(defaults.object(forKey: primaryKey) as? Int ?? 0, fallback: 0) }
    set { defaults.set(languageIndex(newValue, fallback: 0), forKey: primaryKey) }
  }
  static var secondaryIndex: Int {
    get {
      guard let value = defaults.object(forKey: secondaryKey) as? Int else { return -1 }
      return languages.indices.contains(value) ? value : -1
    }
    set { defaults.set(languages.indices.contains(newValue) ? newValue : -1, forKey: secondaryKey) }
  }
  static var onlineEnabled: Bool {
    get { defaults.object(forKey: onlineKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: onlineKey) }
  }
  static var primary: CandidateTranslationLanguage { language(at: primaryIndex) }
  static var secondary: CandidateTranslationLanguage? {
    let index = secondaryIndex
    return languages.indices.contains(index) ? languages[index] : nil
  }
  /// `offline` is the codes whose offline gloss dictionary is installed (see `offlineGlossLanguages`); English always comes from the bundled msime-english.db.
  static func needsNetwork(_ language: CandidateTranslationLanguage, offline: Set<String> = []) -> Bool {
    language.code != "EN" && !offline.contains(language.code)
  }
  /// The languages an offline gloss dictionary can exist for, as host-api's OFFLINE_GLOSS_LANGUAGES names them.
  static let offlineGlossCodes = ["FR", "JA", "ES", "RU", "DE", "KO"]
  /// Which of them are installed beside the resource directory: `offline-glosses/zh-<code>.db`, the sibling host-api reads the non-English glosses from. The bundle does not change while the keyboard runs, so a caller may keep the answer.
  static func offlineGlossLanguages(resources: String?) -> Set<String> {
    guard let resources, !resources.isEmpty else { return [] }
    let directory = URL(fileURLWithPath: resources, isDirectory: true).deletingLastPathComponent()
      .appendingPathComponent("offline-glosses", isDirectory: true)
    return Set(offlineGlossCodes.filter {
      FileManager.default.fileExists(atPath: directory.appendingPathComponent("zh-\($0.lowercased()).db").path)
    })
  }
  private static func languageIndex(_ value: Int, fallback: Int) -> Int {
    languages.indices.contains(value) ? value : fallback
  }
}

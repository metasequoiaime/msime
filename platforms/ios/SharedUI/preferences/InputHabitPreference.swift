import Foundation
import CoreFoundation

/// 「输入习惯」 as one value: learning, frequency adjustment and candidate glosses.
struct InputHabitSettings: Equatable {
  var learning: Bool
  var frequencyMode: FrequencyAdjustmentMode
  var triggerCount: Int
  var linearStep: Int
  var glossEnabled: Bool
  var onlineTranslations: Bool
  /// Positions in `CandidateTranslationPreference.languages`; the secondary one is -1 for none.
  var primaryLanguage: Int
  var secondaryLanguage: Int
}

/// The shared preference document is the source of truth for these settings: the keyboard copies the document into the App Group every time it reloads, so a value written only to the App Group is overwritten on the next keyboard appearance. The App therefore writes the document and then mirrors the result into the App Group, which the keyboard's fallback paths read until that reload.
enum InputHabitPreference {
  static let learningKey = "learning"
  static let frequencyKey = "frequency"
  static let glossKey = "candidate_english_gloss"
  static let translationsKey = "candidate_translations"
  static let primaryLanguageKey = "translation_target_language"
  static let secondaryLanguageKey = "translation_secondary_language"

  /// The App Group copy, used when the document cannot be read and for any field it does not carry.
  static var mirrored: InputHabitSettings {
    InputHabitSettings(learning: DictionaryLearningPreference.enabled,
                       frequencyMode: FrequencyAdjustmentPreference.mode,
                       triggerCount: FrequencyAdjustmentPreference.triggerCount,
                       linearStep: FrequencyAdjustmentPreference.linearStep,
                       glossEnabled: CandidateGlossPreference.enabled,
                       onlineTranslations: CandidateTranslationPreference.onlineEnabled,
                       primaryLanguage: CandidateTranslationPreference.primaryIndex,
                       secondaryLanguage: CandidateTranslationPreference.secondaryIndex)
  }

  /// Read the settings the same way the keyboard mirrors them: a field that is missing or out of range keeps the fallback's value.
  static func settings(in document: [String: Any]?, fallback: InputHabitSettings = mirrored) -> InputHabitSettings {
    var settings = fallback
    guard let document else { return settings }
    if let learning = document[learningKey] as? Bool { settings.learning = learning }
    if let frequency = document[frequencyKey] as? [String: Any] {
      if let mode = (frequency["mode"] as? String).flatMap(FrequencyAdjustmentMode.init(rawValue:)) {
        settings.frequencyMode = mode
      }
      if let count = count(frequency["trigger_count"]) { settings.triggerCount = count }
      if let step = count(frequency["linear_step"]) { settings.linearStep = step }
    }
    if let gloss = document[glossKey] as? Bool { settings.glossEnabled = gloss }
    if let online = document[translationsKey] as? Bool { settings.onlineTranslations = online }
    if let primary = (document[primaryLanguageKey] as? String).flatMap(languageIndex) {
      settings.primaryLanguage = primary
    }
    if let secondary = document[secondaryLanguageKey] as? String {
      settings.secondaryLanguage = languageIndex(secondary) ?? -1
    } else if document.keys.contains(secondaryLanguageKey) {
      settings.secondaryLanguage = -1
    }
    return settings
  }

  /// Write every field, merging the frequency object so fields this page does not show are kept as stored.
  static func write(_ settings: InputHabitSettings, into document: inout [String: Any]) {
    let settings = normalized(settings)
    document[learningKey] = settings.learning
    var frequency = document[frequencyKey] as? [String: Any] ?? [:]
    frequency["mode"] = settings.frequencyMode.rawValue
    frequency["trigger_count"] = settings.triggerCount
    frequency["linear_step"] = settings.linearStep
    document[frequencyKey] = frequency
    document[glossKey] = settings.glossEnabled
    document[translationsKey] = settings.onlineTranslations
    // The document spells languages in lower case, the same as the Windows settings page.
    document[primaryLanguageKey] = CandidateTranslationPreference.language(at: settings.primaryLanguage).code.lowercased()
    if settings.secondaryLanguage >= 0 {
      document[secondaryLanguageKey] = CandidateTranslationPreference.languages[settings.secondaryLanguage].code.lowercased()
    } else {
      // Rust stores `None` by omitting the key, and a missing key keeps the App Group copy, so `mirror` must clear that copy too.
      document.removeValue(forKey: secondaryLanguageKey)
    }
  }

  static func mirror(_ settings: InputHabitSettings) {
    let settings = normalized(settings)
    KeyboardFeedbackPreference.defaults.set(settings.learning, forKey: DictionaryLearningPreference.key)
    FrequencyAdjustmentPreference.mode = settings.frequencyMode
    FrequencyAdjustmentPreference.triggerCount = settings.triggerCount
    FrequencyAdjustmentPreference.linearStep = settings.linearStep
    CandidateGlossPreference.enabled = settings.glossEnabled
    CandidateTranslationPreference.onlineEnabled = settings.onlineTranslations
    CandidateTranslationPreference.primaryIndex = settings.primaryLanguage
    CandidateTranslationPreference.secondaryIndex = settings.secondaryLanguage
  }

  /// Whether a live reload changes the candidate translation requests or their language keys.
  static func translationDisplaySettingsChanged(_ before: InputHabitSettings,
                                                 _ after: InputHabitSettings) -> Bool {
    before.onlineTranslations != after.onlineTranslations
      || before.primaryLanguage != after.primaryLanguage
      || before.secondaryLanguage != after.secondaryLanguage
  }

  /// Apply `change` to the stored settings inside one document update, so a concurrent keyboard write to another field is not lost. Returns the saved settings, or nil when the document could not be written; the App Group is only touched after a successful write.
  @discardableResult
  static func update(stateRoot: URL? = nil, _ change: (inout InputHabitSettings) -> Void) -> InputHabitSettings? {
    var saved: InputHabitSettings?
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { document in
      var current = Self.settings(in: document)
      change(&current)
      current = normalized(current)
      write(current, into: &document)
      saved = current
    }
    guard written, let saved else { return nil }
    mirror(saved)
    return saved
  }

  /// Counts inside the shared range, a known primary language, and a second language that is known and differs from the first, or -1.
  static func normalized(_ settings: InputHabitSettings) -> InputHabitSettings {
    var settings = settings
    let languages = CandidateTranslationPreference.languages.indices
    settings.triggerCount = FrequencyAdjustmentPreference.resolvedCount(settings.triggerCount)
    settings.linearStep = FrequencyAdjustmentPreference.resolvedCount(settings.linearStep)
    if !languages.contains(settings.primaryLanguage) { settings.primaryLanguage = 0 }
    if !languages.contains(settings.secondaryLanguage) || settings.secondaryLanguage == settings.primaryLanguage {
      settings.secondaryLanguage = -1
    }
    return settings
  }

  private static func count(_ value: Any?) -> Int? {
    let integer = (value as? NSNumber).flatMap(strictInteger)
    guard let integer, FrequencyAdjustmentPreference.countRange.contains(integer) else { return nil }
    return integer
  }

  private static func strictInteger(_ value: NSNumber) -> Int? {
    guard CFGetTypeID(value) != CFBooleanGetTypeID(),
          let integer = Int(value.stringValue),
          NSNumber(value: integer).compare(value) == .orderedSame else { return nil }
    return integer
  }

  private static func languageIndex(_ code: String) -> Int? {
    CandidateTranslationPreference.languages.firstIndex { $0.code == code.uppercased() }
  }
}

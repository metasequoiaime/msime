import Foundation

@MainActor
enum FuzzyPinyinPreference {
  static let ruleIDs = ["z-zh", "c-ch", "s-sh", "n-l", "f-h", "r-l", "an-ang", "en-eng", "in-ing", "ian-iang", "uan-uang"]
  static let documentKey = "fuzzy_pinyin"

  /// The document's `fuzzy_pinyin` object, which the settings app, the shared Tauri settings page and the keyboard all read.
  struct Settings: Equatable {
    var enabled: Bool
    var rules: Set<String>
    var seeded: Bool

    static let pristine = Settings(enabled: false, rules: [], seeded: false)

    var bits: UInt32 {
      guard enabled else { return 0 }
      return FuzzyPinyinPreference.ruleIDs.enumerated().reduce(UInt32(0)) { value, entry in
        rules.contains(entry.element) ? value | (1 << entry.offset) : value
      }
    }
  }

  /// `nil` when the document carries no readable `fuzzy_pinyin` object.
  static func settings(in preferences: [String: Any]?) -> Settings? {
    guard let fuzzy = preferences?[documentKey] as? [String: Any],
          let enabled = fuzzy["enabled"] as? Bool,
          let rules = fuzzy["rules"] as? [String] else { return nil }
    return Settings(enabled: enabled, rules: Set(rules).intersection(ruleIDs),
                    seeded: fuzzy["seeded"] as? Bool ?? false)
  }

  /// Write the document.
  ///
  /// PreferencesStore replaces the rules with every rule on the first disabled-to-enabled save of a document that is not yet seeded, judging by the stored document rather than the incoming one. A seeded selection therefore marks the document seeded in its own write before the selection is written, or the user's rules would be overwritten by all eleven.
  static func save(_ settings: Settings, stateRoot: URL? = nil) -> Bool {
    let stored = self.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: stateRoot))
    if settings.seeded, stored?.seeded != true {
      guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { preferences in
        var fuzzy = preferences[documentKey] as? [String: Any] ?? ["enabled": false, "rules": [String]()]
        fuzzy["seeded"] = true
        preferences[documentKey] = fuzzy
      }) else { return false }
    }
    return MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { preferences in
      preferences[documentKey] = ["enabled": settings.enabled, "seeded": settings.seeded,
                                  "rules": ruleIDs.filter(settings.rules.contains)]
    }
  }
}

import Foundation

/// 普通英文是否显示原文和唯一纠错，并在空格或标点边界采用纠错。
enum EnglishSuggestionsPreference {
  static let enabledKey = "english.suggestions"

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  static var isEnabled: Bool {
    get { defaults.object(forKey: enabledKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: enabledKey) }
  }
}

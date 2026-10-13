import Foundation

/// 英文输入时是否在候选栏提示常用单词。
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

import Foundation

/// 「打开键盘时 → 沿用上次」: a new keyboard starts in the Chinese/English mode the user last switched to instead of the shared `default_ime_mode`.
///
/// Desktop hosts keep this through `ime_mode_scope`, whose default remembers the mode per application. iOS does not tell a keyboard extension which application it is typing into, so the per-application map has nothing to key on; the one memory iOS can keep is the global one, and the keyboard process is torn down between appearances, so it lives in the App Group rather than the shared document. Only the user's own switches are recorded: the temporary English a URL or e-mail field brings is not a choice.
enum ImeModeMemoryPreference {
  static let enabledKey = "keyboard.input.remembersImeMode"
  static let lastChineseKey = "keyboard.input.lastImeModeChinese"

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  /// Turning it on or off forgets the recorded mode, so a switch made in an earlier stretch of remembering does not come back later.
  static func setEnabled(_ enabled: Bool, defaults: UserDefaults = defaults) {
    guard enabled != defaults.bool(forKey: enabledKey) else { return }
    defaults.set(enabled, forKey: enabledKey)
    defaults.removeObject(forKey: lastChineseKey)
  }

  static func isEnabled(defaults: UserDefaults = defaults) -> Bool {
    defaults.bool(forKey: enabledKey)
  }

  /// The mode a new keyboard starts in: the last switch when remembering and one was recorded, else `fallback` (the shared `default_ime_mode`).
  static func startsInChinese(fallback: Bool, defaults: UserDefaults = defaults) -> Bool {
    guard defaults.bool(forKey: enabledKey), let last = defaults.object(forKey: lastChineseKey) as? Bool else {
      return fallback
    }
    return last
  }

  static func record(chinese: Bool, defaults: UserDefaults = defaults) {
    guard defaults.bool(forKey: enabledKey) else { return }
    defaults.set(chinese, forKey: lastChineseKey)
  }
}

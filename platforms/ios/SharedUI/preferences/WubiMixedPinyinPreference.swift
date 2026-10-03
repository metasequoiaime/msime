import Foundation

// Answers a wubi code the table cannot spell with quanpin candidates for the same letters. A code
// the table does answer keeps its own candidates, so wubi as typed is unchanged. Lives in the app
// group because the keyboard extension reads what the host app writes.
@MainActor
enum WubiMixedPinyinPreference {
  static let enabledKey = "wubi.mixedPinyin"
  static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }

  /// 没存过时的值随版本：full 是关，五笔版是开。键盘每次重载都把它写回共享文档，所以不能按 `bool(forKey:)` 的缺省 false 读，否则五笔版首次启动就把混拼关掉了。
  static var isEnabled: Bool {
    get { defaults.object(forKey: enabledKey) as? Bool ?? MSIMEAppEdition.wubiMixedPinyinDefault }
    set { defaults.set(newValue, forKey: enabledKey) }
  }
}

import Foundation

extension ClipboardDeleteAuthenticationStore {
  /// 设置 App 写、键盘读的那一份，在 App Group 的 UserDefaults 里（见 `ClipboardDeleteAuthenticationStore`）。没有完全访问权限时键盘读不到 App Group，但那时剪贴板历史本身也不可用。
  static var shared: ClipboardDeleteAuthenticationStore {
    ClipboardDeleteAuthenticationStore(defaults: InputSchemePreference.appGroupDefaults)
  }
}

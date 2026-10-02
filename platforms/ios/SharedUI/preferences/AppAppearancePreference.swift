import UIKit

/// 颜色模式 and 设置界面主题 from the shared document: the global `theme` every surface falls back to, and `settings_theme`, the settings window's own choice. On iOS the settings window is this app, so `settings_theme` picks the app's interface style; left on `follow` it takes the global theme, and with that on `system` the app follows the device as before.
enum AppAppearancePreference {
  static let globalKey = "theme"
  static let settingsKey = "settings_theme"
  static let globalOptions: [(id: String, title: String)] = [("system", "跟随系统"), ("light", "浅色"), ("dark", "深色")]
  static let settingsOptions: [(id: String, title: String)] = [("follow", "跟随颜色模式"), ("light", "浅色"), ("dark", "深色")]
  /// Posted after the app writes either key, so the open windows restyle without waiting to come back to the foreground.
  static let didChange = Notification.Name("AppAppearancePreference.didChange")

  /// The style the app's windows take: `settings_theme`, then `theme`, then `.unspecified` to follow the device.
  static func style(in preferences: [String: Any]?) -> UIUserInterfaceStyle {
    KeyboardAppearancePreference.style(settingsKey, in: preferences)
  }
}

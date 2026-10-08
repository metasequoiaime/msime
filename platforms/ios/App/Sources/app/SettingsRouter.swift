import SwiftUI

/// 键盘发来的深链接落到哪里。键盘会打开 `<scheme>://settings[/<page>]`、`<scheme>://feedback`、`<scheme>://about` 和 `<scheme>://voice`（scheme 为 `MSIMEAppEdition.urlScheme`）；各 tab 视图观察发布出来的路由并推入页面，所以应用已在运行时打开的链接和冷启动落到同一个页面。
final class SettingsRouter: ObservableObject {
  /// 我的 tab 从链接或键盘菜单推入的页面。
  enum AccountRoute: String, Hashable, Identifiable {
    case feedback, about, appTheme

    var id: Self { self }
  }

  /// 要推入的设置页，页面弹出时由导航栈清空。
  @Published var settingsPage: SettingsPage?
  @Published var accountRoute: AccountRoute?

  /// 键盘菜单使用的、不是页面 id 的路径名。
  private static let settingsAliases: [String: SettingsPage] = [
    "lexicon": .dictionary,
    "typing": .input,
  ]

  /// 路由一个链接并返回它所属的 tab；本应用不认识的 host 返回 nil。`voice` 返回设置 tab，调用方继续为它弹出录音面板。
  func handle(_ url: URL) -> AppNavigation.Tab? {
    switch url.host {
    case "settings":
      if let name = url.pathComponents.first(where: { $0 != "/" })?.lowercased(),
         let page = SettingsPage(rawValue: name) ?? Self.settingsAliases[name] {
        settingsPage = page
      }
      return .settings
    case "feedback":
      accountRoute = .feedback
      return .account
    case "about":
      accountRoute = .about
      return .account
    case "voice":
      return .settings
    default:
      return nil
    }
  }
}

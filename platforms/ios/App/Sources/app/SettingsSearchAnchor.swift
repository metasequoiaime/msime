import Foundation

/// 设置搜索结果带着命中的关键词打开页面时的规则，与 Android `SettingsSearchAnchorPolicy` 相同。
///
/// 以前搜索结果只给页面，带分段的页面总从第一段开始：搜「社区插件」打开「插件」停在「我的插件」。现在结果把命中的关键词交给页面（`SettingsPage.destination(searchAnchor:)`），关键词就是页面某个分段的标题时，页面从那一段开始。只命中页面标题、没有关键词命中时不带目标，页面和从设置首页那一行进来一样打开。
enum SettingsSearchAnchor {
  /// 第一个包含查询（去掉首尾空白、忽略大小写，与 `SettingsPage.matching` 相同）的关键词，原样返回；没有或查询为空时为 nil。
  static func anchor(keywords: [String], query: String) -> String? {
    let query = query.trimmingCharacters(in: .whitespaces)
    guard !query.isEmpty else { return nil }
    return keywords.first { $0.localizedCaseInsensitiveContains(query) }
  }

  /// 目标就是页面的某个分段标题时返回这一段的标题，否则为 nil。
  static func section(_ sections: [String], anchor: String?) -> String? {
    guard let anchor = anchor?.trimmingCharacters(in: .whitespaces), !anchor.isEmpty else { return nil }
    return sections.first { $0 == anchor }
  }
}

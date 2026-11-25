import Foundation

/// How many candidates the strip numbers before the rest move to the expanded panel, the desktop's `candidate_page_size`.
///
/// The shared document's value (6 by default) is laid out for a desktop candidate window and cannot tell a chosen 6 from the default one, so iOS keeps its own in the App Group, starting at 9 so the strip's numbers cover every digit on the symbol layer and the iPad number row. The bridge lays it over the document as it does cloud candidates, and the strip reads the value the session was given, so the digits and the numbers never disagree. Never written to the shared document: a desktop keeps its own page size.
enum CandidatePageSizePreference {
  static let key = "candidate.pageSize"
  static let defaultSize = 9
  static let range = 1...9

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  static var size: Int {
    get { clamped(defaults.object(forKey: key) as? Int) }
    set { defaults.set(clamped(newValue), forKey: key) }
  }

  /// A stored or received value inside 1–9; anything else is the default.
  static func clamped(_ value: Int?) -> Int {
    guard let value, range.contains(value) else { return defaultSize }
    return value
  }
}

/// 展开候选面板的样式。
///
/// `card` 是原来那一套：64pt 起自动分列、格子画得像按键、候选条留在上方。`system` 是系统拼音的样子：固定六列、格子只有文字、首行与下面的格子同一条列线，面板从候选条那一行铺起，于是「点展开」看起来就是候选栏原地向下展开，不再多出一条候选。
///
/// 存在 App Group（与每页候选数一样属 iOS 自己的观感），不写共享文档：其他平台的面板各有各的样子。
enum CandidatePanelStylePreference {
  static let key = "candidate.panelStyle"

  enum Style: String, CaseIterable {
    case card
    case system
  }

  static let options: [(style: Style, title: String)] = [(.card, "卡片"), (.system, "系统样式")]

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  static var style: Style {
    get { Style(rawValue: defaults.string(forKey: key) ?? "") ?? .card }
    set { defaults.set(newValue.rawValue, forKey: key) }
  }

  static func title(for style: Style) -> String {
    options.first { $0.style == style }?.title ?? options[0].title
  }
}

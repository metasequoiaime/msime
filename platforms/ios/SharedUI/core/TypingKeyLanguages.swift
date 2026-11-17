import Foundation

/// 只合计共享 Rust 已分类的每日按键，不推断来源或回填历史语言。
struct TypingKeyLanguages {
  enum Language: String, CaseIterable, Identifiable {
    case all, chinese, english, japanese, korean, cantonese, vietnamese, tibetan, common, unknown

    var id: String { rawValue }
    var title: String {
      switch self {
      case .all: "全部"
      case .chinese: "中文"
      case .english: "英文"
      case .japanese: "日文"
      case .korean: "韩文"
      case .cantonese: "粤语"
      case .vietnamese: "越南文"
      case .tibetan: "藏文"
      case .common: "通用键"
      case .unknown: "未分类"
      }
    }
  }

  private let total: [String: Int]
  private let groups: [String: [String: Int]]

  init(dailyKeys: [String: [String: Int]],
       dailyLanguages: [String: [String: [String: Int]]], days: [String]) {
    var total: [String: Int] = [:]
    var groups: [String: [String: Int]] = [:]
    for day in Set(days) {
      Self.merge(dailyKeys[day] ?? [:], into: &total)
      for (language, keys) in dailyLanguages[day] ?? [:] {
        Self.merge(keys, into: &groups[language, default: [:]])
      }
    }
    self.total = total
    self.groups = groups
  }

  var available: [Language] {
    [.all] + Language.allCases.filter { $0 != .all && !(groups[$0.rawValue] ?? [:]).isEmpty }
  }

  /// 重新加载或时间窗口变化后，已消失的分组回到完整总计。
  func resolved(_ language: Language) -> Language {
    available.contains(language) ? language : .all
  }

  func keys(for language: Language) -> [String: Int] {
    let selection = resolved(language)
    return selection == .all ? total : (groups[selection.rawValue] ?? [:])
  }

  private static func merge(_ keys: [String: Int], into result: inout [String: Int]) {
    for (key, count) in keys where count > 0 {
      let old = result[key, default: 0]
      result[key] = old > Int.max - count ? Int.max : old + count
    }
  }
}

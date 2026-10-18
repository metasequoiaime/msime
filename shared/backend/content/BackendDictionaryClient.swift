import Foundation

/// 云词库页面读写的形状（macOS 的云词库窗口、iOS 的云词库页和共用的「完整词库目录」），请求经 Rust 的 `msime_client_cloud_dictionary`（#8201 迁移计划 4c）。
extension BackendAccountClient {
  enum DictionaryKind: String, CaseIterable, Identifiable, Codable, Sendable {
    /// `wubi98` 是 98 五笔码表的词条，云端路径同名；`wubi` 仍是 86 五笔。
    case pinyin, wubi, wubi98, quick, english
    var id: String { rawValue }
    var title: String {
      switch self { case .pinyin: return "拼音"; case .wubi: return "86 五笔"; case .wubi98: return "98 五笔"; case .quick: return "快捷短语"; case .english: return "英文" }
    }
  }
  struct DictionaryEntry: Decodable, Identifiable, Sendable {
    let id: String
    let kind: DictionaryKind
    let code: String
    let word: String
    let weight: Int64
    let revision: Int64
  }
  struct DictionaryPage: Decodable, Sendable {
    let entries: [DictionaryEntry]
    let has_more: Bool
    let offset: Int
  }
  struct DictionaryValue: Encodable, Sendable {
    let code: String
    let word: String
    let weight: Int64
  }
  enum DictionaryFileFormat: String, CaseIterable, Identifiable, Sendable {
    case standard, windows, hans
    var id: String { rawValue }
    var title: String {
      switch self { case .standard: return "标准 TSV"; case .windows: return "Windows TSV"; case .hans: return "汉字自动注音" }
    }
  }
  struct CatalogEntry: Decodable, Identifiable, Sendable {
    let kind: DictionaryKind
    let code: String
    let word: String
    let weight: Int64
    var id: String { "\(kind.rawValue):\(code.utf8.count):\(code)\(word)" }
    var value: DictionaryValue { .init(code: code, word: word, weight: weight) }
  }
  struct DictionaryCatalog: Decodable, Sendable {
    let entries: [CatalogEntry]
    let offset: Int
    let has_more: Bool
    let revision: Int64
    let normalized: String
  }
}

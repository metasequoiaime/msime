import Foundation

extension BackendAccountClient {
  enum DictionaryKind: String, CaseIterable, Identifiable, Codable, Sendable {
    case pinyin, wubi, quick, english
    var id: String { rawValue }
    var title: String {
      switch self { case .pinyin: return "拼音"; case .wubi: return "五笔"; case .quick: return "快捷短语"; case .english: return "英文" }
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
  struct DictionaryChange: Decodable, Sendable {
    let revision: Int64
    let previous: DictionaryEntry?
    let replacement: DictionaryEntry?
  }
  struct DictionaryChangePage: Decodable, Sendable {
    struct Entry: Decodable, Sendable {
      let id: String
      let kind: DictionaryKind
      let code: String
      let word: String
      let weight: Int64
      let revision: Int64
      let user_inserted: Bool?
    }
    struct Selection: Decodable, Sendable {
      let context: String
      let code: String
      let word: String
      let count: Int
    }
    struct Change: Decodable, Sendable {
      let revision: Int64
      let previous: Entry?
      let replacement: Entry?
      let reset: Bool?
      let ranking: [Entry]?
      let selection: Selection?
      // A zero position removes a fixed slot; retain it in the change feed.
      let position: FixedPosition?
    }
    let changes: [Change]
    let next: Int64
    let has_more: Bool
  }
  func dictionaryChanges(after: Int64, limit: Int = 100, token: String) async throws -> DictionaryChangePage {
    guard after >= 0, (1...100).contains(limit) else { throw Failure(status: 400) }
    let page: DictionaryChangePage = try await json("GET",
      "/v1/users/me/dictionary/changes?after=\(after)&limit=\(limit)", token: token)
    var cursor = after
    guard page.changes.count <= limit else { throw Failure(status: 502) }
    for change in page.changes {
      guard change.revision > cursor else { throw Failure(status: 502) }
      cursor = change.revision
    }
    guard page.next == cursor, !page.has_more || !page.changes.isEmpty else { throw Failure(status: 502) }
    return page
  }
  struct DictionaryValue: Encodable, Sendable {
    let code: String
    let word: String
    let weight: Int64
  }
  func dictionary(_ kind: DictionaryKind, search: String = "", offset: Int = 0, token: String) async throws -> DictionaryPage {
    guard (0...1_000_000).contains(offset), search.utf8.count <= 1024, !search.contains("\0") else { throw Failure(status: 400) }
    var url = URLComponents()
    url.path = "/v1/users/me/dictionaries/" + kind.rawValue
    url.queryItems = [.init(name: "q", value: search), .init(name: "offset", value: String(offset)), .init(name: "limit", value: "100")]
    guard let path = Self.encodedPath(url) else { throw Failure(status: 400) }
    let page: DictionaryPage = try await json("GET", path, token: token)
    guard page.entries.count <= 100, page.offset == offset,
          page.entries.allSatisfy({ entry in
            entry.kind == kind && Self.validDictionaryEntry(entry)
          }) else { throw Failure(status: 0) }
    return page
  }
  func addDictionary(_ kind: DictionaryKind, value: DictionaryValue, token: String) async throws -> DictionaryChange {
    try await json("POST", "/v1/users/me/dictionaries/" + kind.rawValue, token: token, body: JSONEncoder().encode(value))
  }
  func updateDictionary(_ entry: DictionaryEntry, value: DictionaryValue, token: String) async throws -> DictionaryChange {
    struct Body: Encodable { let code: String; let word: String; let weight: Int64; let revision: Int64 }
    return try await json("PUT", dictionaryEntryPath(entry), token: token,
      body: JSONEncoder().encode(Body(code: value.code, word: value.word, weight: value.weight, revision: entry.revision)))
  }
  func deleteDictionary(_ entry: DictionaryEntry, token: String) async throws -> DictionaryChange {
    struct Body: Encodable { let revision: Int64 }
    return try await json("DELETE", dictionaryEntryPath(entry), token: token, body: JSONEncoder().encode(Body(revision: entry.revision)))
  }
  enum DictionaryFileFormat: String, CaseIterable, Identifiable, Sendable {
    case standard, windows, hans
    var id: String { rawValue }
    var title: String {
      switch self { case .standard: return "标准 TSV"; case .windows: return "Windows TSV"; case .hans: return "汉字自动注音" }
    }
  }
  struct DictionaryImportResult: Decodable, Sendable { let imported: Int; let revision: Int64 }
  func importDictionary(_ kind: DictionaryKind, text: String, format: DictionaryFileFormat, token: String) async throws -> DictionaryImportResult {
    let body: Data
    let suffix: String
    if format == .hans {
      guard kind == .pinyin else { throw Failure(status: 400) }
      struct Body: Encodable { let text: String; let weight: Int64 }
      body = try JSONEncoder().encode(Body(text: text, weight: 100000)); suffix = "/import-hans"
    } else {
      struct Body: Encodable { let text: String; let format: String }
      body = try JSONEncoder().encode(Body(text: text, format: format.rawValue)); suffix = "/import"
    }
    guard !text.isEmpty, body.count <= 65536 else { throw Failure(status: 400) }
    return try await json("POST", "/v1/users/me/dictionaries/" + kind.rawValue + suffix, token: token, body: body)
  }
  func exportDictionary(_ kind: DictionaryKind, format: DictionaryFileFormat, token: String) async throws -> URL {
    guard format != .hans else { throw Failure(status: 400) }
    return try await download("/v1/users/me/dictionaries/" + kind.rawValue + "/export?format=" + format.rawValue,
      token: token, filename: "dictionary-" + kind.rawValue + ".tsv", maximumBytes: 384 * 1024 * 1024)
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
  func dictionaryCatalog(_ kind: DictionaryKind, code: String, offset: Int = 0, scheme: String = "pinyin", profile: String = "xiaohe", token: String) async throws -> DictionaryCatalog {
    guard (0...1_000_000).contains(offset), code.utf8.count <= 256, !code.contains("\0") else { throw Failure(status: 400) }
    var components = URLComponents()
    components.path = "/v1/users/me/dictionaries/" + kind.rawValue + "/catalog"
    components.queryItems = [.init(name: "q", value: code), .init(name: "offset", value: String(offset)), .init(name: "limit", value: "100"), .init(name: "scheme", value: scheme), .init(name: "profile", value: profile)]
    guard let path = Self.encodedPath(components) else { throw Failure(status: 400) }
    let page: DictionaryCatalog = try await json("GET", path, token: token)
    guard page.entries.count <= 100, page.offset == offset, page.revision >= 0,
          Self.validCatalogText(page.normalized, maximum: 256, empty: true),
          page.entries.allSatisfy({ entry in
            entry.kind == kind && Self.validCatalogText(entry.code, maximum: 256)
              && Self.validCatalogText(entry.word, maximum: 1024) && entry.weight >= 0
          }) else { throw Failure(status: 0) }
    return page
  }
  func editCatalog(_ entry: CatalogEntry, revision: Int64, replacement: DictionaryValue?, token: String) async throws -> DictionaryChange {
    guard revision >= 0 else { throw Failure(status: 400) }
    struct Identity: Encodable { let code: String; let word: String }
    struct Body: Encodable {
      let revision: Int64
      let previous: Identity
      let replacement: DictionaryValue?
      enum CodingKeys: String, CodingKey { case revision, previous, replacement }
      func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(revision, forKey: .revision)
        try values.encode(previous, forKey: .previous)
        // Deletion is an explicit null, not an omitted replacement field.
        try values.encode(replacement, forKey: .replacement)
      }
    }
    return try await json("POST", "/v1/users/me/dictionaries/" + entry.kind.rawValue + "/edit", token: token,
      body: JSONEncoder().encode(Body(revision: revision, previous: .init(code: entry.code, word: entry.word), replacement: replacement)))
  }
  private func dictionaryEntryPath(_ entry: DictionaryEntry) throws -> String {
    guard entry.revision > 0, entry.id.utf8.count == 64,
          entry.id.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else { throw Failure(status: 400) }
    return "/v1/users/me/dictionaries/" + entry.kind.rawValue + "/" + entry.id
  }

  private static func validDictionaryEntry(_ entry: DictionaryEntry) -> Bool {
    entry.id.utf8.count == 64
      && entry.id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
      && !entry.code.isEmpty
      && entry.code.utf8.count <= 256
      && !entry.code.unicodeScalars.contains { $0.properties.generalCategory == .control }
      && !entry.word.isEmpty
      && entry.word.utf8.count <= 1024
      && !entry.word.unicodeScalars.contains { $0.properties.generalCategory == .control }
      && entry.weight >= 0
      && entry.revision > 0
  }

  private static func validCatalogText(_ value: String, maximum: Int, empty: Bool = false) -> Bool {
    (empty || !value.isEmpty)
      && value.utf8.count <= maximum
      && !value.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }
}

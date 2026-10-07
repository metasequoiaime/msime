import Foundation

extension BackendAccountClient {
  enum ResourceKind: String, Codable, CaseIterable, Identifiable, Sendable {
    case dictionary, reply
    var id: String { rawValue }
    var title: String { self == .dictionary ? "共享词包" : "回复模板" }
  }
  enum ResourceScope: String, CaseIterable, Sendable { case all = "", mine, saved }
  struct SharedWord: Codable, Sendable {
    let kind: DictionaryKind
    let code: String
    let word: String
    let weight: Int64
  }
  struct ResourceContent: Codable, Sendable {
    var entries: [SharedWord]? = nil
    var prompt: String? = nil
  }
  struct CommunityResource: Decodable, Identifiable, Sendable {
    let id: UUID
    let kind: ResourceKind
    let name: String
    let description: String
    let author: String
    let content: ResourceContent
    let revision: Int
    let saves: Int
    let saved: Bool
    let owned: Bool
    let rating_count: Int
    let rating_average: Double
    let my_rating: Int
    /// "approved", "pending" or "removed" on the user's own items when the request asked for fields=moderation; absent otherwise.
    var moderation: String? = nil
    /// Post-moderation: an item is public at once, so only a removal is shown to its author, never a pending state or a reason.
    var removed: Bool { owned && moderation == "removed" }
  }
  struct ResourcePage: Decodable, Sendable {
    let items: [CommunityResource]
    let has_more: Bool
  }
  struct ResourcePublication: Decodable, Sendable {
    let id: UUID
    let revision: Int
  }
  // A page contains up to twenty complete 350 KB publications. JSON escaping can
  // expand a stored character sixfold; give only this endpoint a larger bound.
  func communityResources(_ kind: ResourceKind, scope: ResourceScope = .all, search: String = "",
                          offset: Int = 0, token: String? = nil) async throws -> ResourcePage {
    guard (0...1_000_000).contains(offset), Self.resourceText(search, minimum: 0, maximum: 128, multiline: false),
          scope == .all || token != nil else { throw Failure(status: 400) }
    var parts = URLComponents()
    parts.path = "/v1/community/resources"
    parts.queryItems = [.init(name: "kind", value: kind.rawValue), .init(name: "scope", value: scope.rawValue),
                        .init(name: "q", value: search), .init(name: "offset", value: String(offset))]
    // The author's own list carries each item's moderation state only when asked for.
    if scope == .mine { parts.queryItems?.append(.init(name: "fields", value: "moderation")) }
    guard let path = Self.encodedPath(parts) else { throw Failure(status: 400) }
    let page: ResourcePage = try await json("GET", path, token: token, maximumResponseBytes: 48 * 1024 * 1024)
    guard page.items.count <= 20, !page.has_more || !page.items.isEmpty,
          Set(page.items.map(\.id)).count == page.items.count,
          page.items.allSatisfy({ Self.validResponse($0, expectedKind: kind) }) else { throw Failure(status: 502) }
    return page
  }
  func communityResource(_ id: UUID, token: String? = nil) async throws -> CommunityResource {
    guard Self.validResourceID(id) else { throw Failure(status: 400) }
    let value: CommunityResource = try await json("GET", Self.resourcePath(id) + "?fields=moderation", token: token,
                                                 maximumResponseBytes: 3 * 1024 * 1024)
    guard value.id == id, Self.validResponse(value, expectedKind: value.kind) else { throw Failure(status: 502) }
    return value
  }
  func publishResource(id: UUID, kind: ResourceKind, name: String, description: String,
                       content: ResourceContent, revision: Int, token: String) async throws -> ResourcePublication {
    let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
    let description = description.trimmingCharacters(in: .whitespacesAndNewlines)
    guard Self.validResourceID(id), (0...50_000).contains(revision),
          Self.resourceText(name, minimum: 1, maximum: 32, multiline: false),
          Self.resourceText(description, minimum: 0, maximum: 280, multiline: true) else { throw Failure(status: 400) }
    switch kind {
    case .dictionary:
      guard content.prompt?.isEmpty != false, let entries = content.entries, (1...128).contains(entries.count),
            entries.allSatisfy(Self.validSharedWord),
            Set(entries.map { "\($0.kind.rawValue)\u{0}\($0.code)\u{0}\($0.word)" }).count == entries.count else { throw Failure(status: 400) }
    case .reply:
      guard content.entries?.isEmpty != false, let prompt = content.prompt,
            Self.resourceText(prompt, minimum: 1, maximum: 2000, multiline: true) else { throw Failure(status: 400) }
    }
    struct Body: Encodable { let id: String; let kind: ResourceKind; let name: String; let description: String; let content: ResourceContent; let revision: Int }
    let body = try JSONEncoder().encode(Body(id: id.uuidString.lowercased(), kind: kind, name: name,
                                           description: description, content: content, revision: revision))
    guard body.count <= 350000 else { throw Failure(status: 400) }
    let result: ResourcePublication = try await json("POST", "/v1/community/resources", token: token, body: body)
    guard result.id == id, result.revision > 0 else { throw Failure(status: 502) }
    return result
  }
  struct ResourceApplication: Decodable, Sendable {
    let revision: Int64
    let imported: Int
    let resource_revision: Int
  }
  func applyResource(_ id: UUID, resourceRevision: Int, dictionaryRevision: Int64,
                     token: String) async throws -> ResourceApplication {
    guard Self.validResourceID(id), (1...Int(UInt32.max)).contains(resourceRevision), dictionaryRevision >= 0 else { throw Failure(status: 400) }
    struct Body: Encodable { let resource_revision: Int; let dictionary_revision: Int64 }
    let result: ResourceApplication = try await json("POST", Self.resourcePath(id) + "/apply", token: token,
      body: JSONEncoder().encode(Body(resource_revision: resourceRevision, dictionary_revision: dictionaryRevision)))
    guard result.resource_revision == resourceRevision, (0...128).contains(result.imported),
          result.revision >= dictionaryRevision,
          result.revision - dictionaryRevision == Int64(result.imported) else { throw Failure(status: 502) }
    return result
  }
  func saveResource(_ id: UUID, saved: Bool, token: String) async throws {
    guard Self.validResourceID(id) else { throw Failure(status: 400) }
    struct Body: Codable { let saved: Bool }
    let response: Body = try await json("PUT", Self.resourcePath(id) + "/save", token: token,
                                        body: JSONEncoder().encode(Body(saved: saved)))
    guard response.saved == saved else { throw Failure(status: 502) }
  }
  func rateResource(_ id: UUID, stars: Int, token: String) async throws {
    guard Self.validResourceID(id), (1...5).contains(stars) else { throw Failure(status: 400) }
    struct Body: Codable { let stars: Int }
    let response: Body = try await json("PUT", Self.resourcePath(id) + "/rating", token: token,
                                        body: JSONEncoder().encode(Body(stars: stars)))
    guard response.stars == stars else { throw Failure(status: 502) }
  }
  func deleteResource(_ id: UUID, token: String) async throws {
    guard Self.validResourceID(id) else { throw Failure(status: 400) }
    struct Result: Decodable { let deleted: Bool }
    let response: Result = try await json("DELETE", Self.resourcePath(id), token: token)
    guard response.deleted else { throw Failure(status: 502) }
  }
  /// The fixed report reasons, in dialog order; each is the exact string the server accepts.
  static let reportReasons = ["侵权/抄袭", "色情低俗", "违法违规", "垃圾广告", "恶意插件", "其他"]
  /// Report another user's item to the moderators. kind is skins, candidate-skins, plugins, dictionaries or replies; any signed-in session counts, the device's anonymous account included.
  func reportContent(kind: String, itemID: UUID, reason: String, detail: String, token: String) async throws {
    let detail = detail.trimmingCharacters(in: .whitespacesAndNewlines)
    guard itemID != UUID(uuidString: "00000000-0000-0000-0000-000000000000")!,
          ["skins", "candidate-skins", "plugins", "dictionaries", "replies"].contains(kind),
          Self.reportReasons.contains(reason),
          Self.resourceText(detail, minimum: 0, maximum: 1000, multiline: true) else { throw Failure(status: 400) }
    struct Body: Encodable { let kind: String; let item_id: String; let reason: String; let detail: String? }
    struct Result: Decodable { let reported: Bool }
    let body = try JSONEncoder().encode(Body(kind: kind, item_id: itemID.uuidString.lowercased(), reason: reason,
                                             detail: detail.isEmpty ? nil : detail))
    let response: Result = try await json("POST", "/v1/community/reports", token: token, body: body)
    guard response.reported else { throw Failure(status: 502) }
  }
  private static func resourcePath(_ id: UUID) -> String { "/v1/community/resources/" + id.uuidString.lowercased() }
  private static func validResourceID(_ id: UUID) -> Bool {
    id != UUID(uuidString: "00000000-0000-0000-0000-000000000000")!
  }
  private static func validSharedWord(_ entry: SharedWord) -> Bool {
    entry.weight >= 0 && resourceText(entry.code, minimum: 1, maximum: 256, multiline: false) &&
      resourceText(entry.word, minimum: 1, maximum: 1_024, multiline: false)
  }
  private static func validResponse(_ value: CommunityResource, expectedKind: ResourceKind) -> Bool {
    guard value.kind == expectedKind,
          value.id != UUID(uuidString: "00000000-0000-0000-0000-000000000000")!, value.revision > 0,
          resourceText(value.name, minimum: 1, maximum: 32, multiline: false),
          value.name == value.name.trimmingCharacters(in: .whitespacesAndNewlines),
          resourceText(value.description, minimum: 0, maximum: 280, multiline: true),
          resourceText(value.author, minimum: 1, maximum: 128, multiline: false),
          value.author == value.author.trimmingCharacters(in: .whitespacesAndNewlines),
          value.saves >= 0, value.saves <= 9_007_199_254_740_991,
          value.rating_count >= 0, value.rating_count <= 9_007_199_254_740_991,
          value.my_rating >= 0, value.my_rating <= 5,
          value.rating_average.isFinite, (0...5).contains(value.rating_average),
          value.rating_count != 0 || value.rating_average == 0 else { return false }
    switch value.kind {
    case .reply:
      guard value.content.entries?.isEmpty != false, let prompt = value.content.prompt else { return false }
      return resourceText(prompt, minimum: 1, maximum: 2_000, multiline: true)
    case .dictionary:
      guard value.content.prompt == nil, let entries = value.content.entries,
            (1...128).contains(entries.count) else { return false }
      let keys = entries.map { "\($0.kind.rawValue)\u{0}\($0.code)\u{0}\($0.word)" }
      return entries.allSatisfy(validSharedWord) && Set(keys).count == entries.count
    }
  }
  private static func resourceText(_ text: String, minimum: Int, maximum: Int, multiline: Bool) -> Bool {
    text.trimmingCharacters(in: .whitespacesAndNewlines).unicodeScalars.count >= minimum &&
      text.unicodeScalars.count <= maximum && text.unicodeScalars.allSatisfy {
        $0.properties.generalCategory != .control || (multiline && ($0 == "\n" || $0 == "\t"))
      }
  }
}

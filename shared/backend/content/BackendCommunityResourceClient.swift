import Foundation

/// 社区词包与回复模板页读的形状（`shared/backend-ui/dictionary/BackendCommunityResourcesView.swift`），请求经 Rust 的 `msime_client_community`（#8201 迁移计划 4c）。
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
  /// The fixed report reasons, in dialog order; each is the exact string the server accepts.
  static let reportReasons = ["侵权/抄袭", "色情低俗", "违法违规", "垃圾广告", "恶意插件", "其他"]
}

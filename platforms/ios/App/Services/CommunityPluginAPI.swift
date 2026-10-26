import Foundation

/// 社区里的一个插件，对应 `client-services::plugins::community::CommunityPlugin`。
struct CommunityPlugin: Decodable, Identifiable, Hashable, Sendable {
  let id: String
  let kind: String
  /// 清单 id，也是装好以后的文件夹名。
  let plugin_id: String
  let name: String
  let description: String
  let author: String
  let version: String
  let license: String
  let size: Int
  let downloads: Int
  let rating_count: Int
  let rating_average: Double
  let owned: Bool
  var my_rating: Int
  /// 只在自己的作品上，`removed` 显示「已下架」；数据类插件的 `pending` 当作已通过。
  var moderation: String? = nil

  var pluginKind: PluginKind? { PluginKind(rawValue: kind) }
  var kindLabel: String { pluginKind?.label ?? kind }
  var removed: Bool { owned && moderation == "removed" }
}

struct CommunityPluginPage: Decodable, Sendable {
  let plugins: [CommunityPlugin]
  let has_more: Bool
  /// 本客户端装不了的类型，Rust 已经去掉，但仍计入下一页的偏移。
  var skipped: Int? = nil
}

/// 发布前本机打包的结果，不上传。
struct CommunityPluginPackPreview: Decodable, Equatable, Sendable {
  let suggestedName: String
  let suggestedDescription: String
  let version: String
  let license: String
  let fileCount: Int
  let size: Int
}

/// 社区插件：列表、详情、安装、评分、下架和发布，经 Rust 的 `msime_client_community`（`plugin_*`，与桌面外壳的插件社区命令相同），令牌不出 Rust。打包、发布和安装要带放 `plugins/` 的状态根目录，即 App Group 里键盘读的那一份，所以从社区装来的包键盘下次获得焦点时就能用上。举报走 `SkinCommunityAPI.report`。
enum CommunityPluginAPI {
  /// 手机上起作用、因此在社区里列出的类型，按 `PluginKind.phoneOrder`。
  static let kinds = PluginKind.phoneOrder

  static func list(offset: Int, search: String, kind: PluginKind?, mine: Bool) async throws -> CommunityPluginPage {
    var action: [String: Any] = ["operation": "plugin_list", "offset": offset, "search": search, "mine": mine]
    if let kind { action["kind"] = kind.rawValue }
    return try await call(action, as: CommunityPluginPage.self)
  }

  static func detail(_ id: String) async throws -> CommunityPlugin {
    try await call(["operation": "plugin_detail", "id": id], as: CommunityPlugin.self)
  }

  static func packPreview(kind: PluginKind, pluginID: String) async throws -> CommunityPluginPackPreview {
    try await call(["operation": "plugin_pack_preview", "kind": kind.rawValue, "plugin_id": pluginID],
                   as: CommunityPluginPackPreview.self, publishing: true)
  }

  /// `id` 是这次发布的标识：失败后重试沿用它，服务端据此去重。
  static func publish(kind: PluginKind, pluginID: String, id: String, name: String, description: String) async throws -> CommunityPlugin {
    try await call(["operation": "plugin_publish", "kind": kind.rawValue, "plugin_id": pluginID, "id": id,
                    "name": name, "description": description], as: CommunityPlugin.self, publishing: true)
  }

  /// 下载、校验并整包替换同类型同 id 的已安装包，回答装好的包。
  static func install(_ plugin: CommunityPlugin) async throws -> PluginPackage {
    let data = try await communityData(["operation": "plugin_install", "id": plugin.id, "kind": plugin.kind,
                                        "plugin_id": plugin.plugin_id], publishing: false)
    guard let value = try? JSONSerialization.jsonObject(with: data), let pack = PluginPackage(json: value) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return pack
  }

  static func rate(_ id: String, stars: Int) async throws {
    _ = try await communityData(["operation": "plugin_rate", "id": id, "stars": stars], publishing: false)
  }

  static func delete(_ id: String) async throws {
    _ = try await communityData(["operation": "plugin_delete", "id": id], publishing: false)
  }

  private static func call<T: Decodable>(_ action: [String: Any], as type: T.Type, publishing: Bool = false) async throws -> T {
    let data = try await communityData(action, publishing: publishing)
    do { return try JSONDecoder().decode(T.self, from: data) }
    catch { throw CommunityFailure(message: "社区暂时不可用，请稍后重试。") }
  }

  /// 与 `BackendRustAccount.community` 相同的请求，多带插件操作要的 `state_root`。
  private static func communityData(_ action: [String: Any], publishing: Bool) async throws -> Data {
    do {
      let account = try BackendRustAccount.current()
      return try await account.client.call(.community, [
        "session_store": account.store, "state_root": MetasequoiaInputSessionBridge.sharedStateDirectory, "action": action,
      ])
    } catch let failure as BackendRustClient.Failure {
      throw try Self.failure(failure, publishing: publishing)
    }
  }

  /// 插件社区的失败说法，与 `communityPluginMessage` 相同：插件自己的失败码先说，其余交给 `SkinCommunityAPI.failure`。
  static func failure(_ failure: BackendRustClient.Failure, publishing: Bool) throws -> CommunityFailure {
    let detail = ((try? JSONSerialization.jsonObject(with: failure.response)) as? [String: Any])?["detail"] as? String
    switch failure.code {
    case "community_forbidden": return CommunityFailure(message: "下载后才能评分，且不能给自己的作品评分。")
    case "community_conflict":
      return CommunityFailure(message: publishing ? "发布信息已变更，或已达到发布上限（最多 20 个、合计 32 MB）。" : "作品状态已变化，请刷新后重试。")
    case "community_rate_limited" where publishing: return CommunityFailure(message: "每小时最多发布 10 次，请稍后再试。")
    case "community_invalid" where publishing: return CommunityFailure(message: "服务器未接受这个插件：请确认名称、说明和包内容符合发布要求。")
    case "plugin_community_too_large": return CommunityFailure(message: "插件压缩后不能超过 8 MB。")
    case "plugin_community_checksum": return CommunityFailure(message: "下载的插件已损坏，请重试。")
    case "plugin_community_mismatch": return CommunityFailure(message: "下载的插件与作品信息不符，已停止安装。")
    case "storage": return CommunityFailure(message: "无法读写插件目录，请检查数据目录的权限。")
    default:
      if failure.code.hasPrefix("plugin_") {
        return CommunityFailure(message: PluginCatalogPolicy.errorMessage(.init(code: failure.code, detail: detail),
                                                                          fallback: "社区暂时不可用，请稍后重试。"))
      }
      return try SkinCommunityAPI.failure(failure)
    }
  }
}

/// 社区插件列表的翻页：按类型筛选之外，「全部」里这台设备用不上的类型（音乐包、特效包）在这里去掉，但仍计入下一页的偏移，与 Rust 去掉装不了的类型时计入 `skipped` 一样。
struct CommunityPluginPaging: Equatable {
  /// 下一页从服务端的哪一条开始。
  private(set) var offset = 0
  private(set) var items: [CommunityPlugin] = []
  private(set) var hasMore = false

  mutating func accept(_ page: CommunityPluginPage, append: Bool) {
    let shown = page.plugins.filter { $0.pluginKind.map(CommunityPluginAPI.kinds.contains) ?? false }
    let listed = page.plugins.count + (page.skipped ?? 0)
    if append {
      offset += listed
      items += shown.filter { item in !items.contains { $0.id == item.id } }
    } else {
      offset = listed
      items = shown
    }
    hasMore = page.has_more
  }

  mutating func replace(_ item: CommunityPlugin) {
    if let index = items.firstIndex(where: { $0.id == item.id }) { items[index] = item }
  }

  mutating func remove(_ id: String) { items.removeAll { $0.id == id } }
}

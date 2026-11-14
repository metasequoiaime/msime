import Foundation

@_silgen_name("msime_client_dictionary_collections")
private func msimeCollectionsRequest(_ bytes: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeCollectionsFree(_ value: UnsafeMutablePointer<CChar>?)

/// 命名分组只适配共享 Rust 协议，元数据与个人词库队列都放在原有 App Group 状态目录。
enum DictionaryCollectionsBridge {
  struct Source: Decodable, Sendable {
    let type: String
    let format: String?
    var title: String { type == "community" ? "社区词库" : type == "import" ? "文件导入" : "自建词库" }
  }
  struct Collection: Decodable, Identifiable, Sendable {
    let id: String
    let name: String
    let kind: String
    let source: Source
    let enabled: Bool
    let entry_count: Int
    let pending: Int
  }
  struct ImportReport: Decodable, Sendable {
    let imported: Int
    let duplicates: Int
    let failed: Int
    let truncated: Bool
  }
  struct Document: Decodable, Sendable {
    let collections: [Collection]
    let outbox_pending: Int
    let `import`: ImportReport?
  }
  enum Failure: LocalizedError {
    case rejected(String)
    var errorDescription: String? {
      guard case .rejected(let code) = self else { return nil }
      switch code {
      case "collections_name_invalid": return "分组名称须为 1–32 个字。"
      case "collections_limit": return "最多保存 32 个分组。"
      case "collections_source_conflict": return "此分组不能接收该来源，请新建分组导入。"
      case "collections_not_found": return "分组已不存在，请刷新。"
      case "collections_corrupt": return "分组数据损坏，原文件未改动。"
      case "collections_too_large": return "文件或分组超出容量限制。"
      case "dictionary maintenance busy": return "键盘正在使用词库，收起键盘后重试。"
      default: return "词库操作失败，请稍后重试。"
      }
    }
  }

  static func perform(_ action: [String: Any]) throws -> Document {
    let options = try MetasequoiaInputSessionBridge.preparedHostOptions()
    let value = try request(action, options: options)
    return try JSONDecoder().decode(Document.self, from: JSONSerialization.data(withJSONObject: value))
  }

  /// 仅在有集合待发项或集合请求时才释放会话；普通词条继续走已有的 Swift 同步路径。
  static func needsSynchronization(options: Data) throws -> Bool {
    guard let object = try JSONSerialization.jsonObject(with: options) as? [String: Any],
          let directory = object["preferences_directory"] as? String,
          FileManager.default.fileExists(atPath: directory + "/DictionaryCollections/index.json") else { return false }
    let value = try request(["operation": "load"], options: options)
    if (value["outbox_pending"] as? Int ?? 0) > 0 { return true }
    return try PersonalDictionaryStore().read().requests.contains { $0.status == .pending && $0.id.hasPrefix("collections-") }
  }

  static func synchronize(options: Data, budget: Int) throws -> Bool {
    let result = try request(["operation": "synchronize", "budget_millis": budget], options: options)
    return (result["applied_count"] as? Int ?? 0) > 0
  }

  static func request(_ action: [String: Any], options: Data) throws -> [String: Any] {
    let decoded = try JSONSerialization.jsonObject(with: options)
    let data = try JSONSerialization.data(withJSONObject: ["options": decoded, "action": action])
    let pointer = data.withUnsafeBytes { msimeCollectionsRequest($0.bindMemory(to: UInt8.self).baseAddress, UInt(data.count)) }
    guard let pointer else { throw Failure.rejected("unavailable") }
    defer { msimeCollectionsFree(pointer) }
    guard let envelope = try JSONSerialization.jsonObject(with: Data(String(cString: pointer).utf8)) as? [String: Any],
          envelope["ok"] as? Bool == true, let value = envelope["value"] as? [String: Any] else {
      let envelope = try? JSONSerialization.jsonObject(with: Data(String(cString: pointer).utf8)) as? [String: Any]
      throw Failure.rejected(envelope?["error"] as? String ?? "unavailable")
    }
    return value
  }
}

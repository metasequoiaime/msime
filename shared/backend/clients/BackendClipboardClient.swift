import Foundation

extension BackendAccountClient {
  /// `pinned` 和 `device` 是较新的字段：旧服务端不返回它们，两者默认都为 nil，现有调用方不传它们也能照常构造条目。
  struct ClipboardItem: Decodable, Identifiable, Sendable {
    let id: String
    let text: String
    let updated_at: String
    var pinned: Bool? = nil
    /// 写入该条目的设备名；未知时为 nil。
    var device: String? = nil
  }
  /// `retention_days` 是服务端保留条目的天数（1、7 或 30；0 表示保留到删除为止）。旧服务端不返回这个字段。
  struct ClipboardPage: Decodable, Sendable {
    let enabled: Bool
    let items: [ClipboardItem]
    var retention_days: Int? = nil
  }
  /// 服务端接受的保留天数选项；0 表示保留到删除为止。
  static let clipboardRetentionDays = [1, 7, 30, 0]

  func clipboard(token: String, search: String = "") async throws -> ClipboardPage {
    guard Self.validClipboardSearch(search) else { throw Failure(status: 400) }
    var components = URLComponents()
    components.path = "/v1/users/me/clipboard"
    components.queryItems = [URLQueryItem(name: "q", value: search)]
    guard let path = Self.encodedPath(components) else { throw Failure(status: 0) }
    let page: ClipboardPage = try await json("GET", path, token: token)
    guard page.items.count <= 50,
          page.items.allSatisfy(Self.validClipboardItem) else { throw Failure(status: 0) }
    // 较新的可选字段解析不了时降级处理，而不是让整页失败，和 Android 客户端一样：未知的保留天数当作缺省，空的或不可用的设备名直接丢弃。
    let items = page.items.map { item in
      var item = item
      if let device = item.device, device.isEmpty || !Self.validClipboardDevice(device) { item.device = nil }
      return item
    }
    let retention = page.retention_days.flatMap { Self.clipboardRetentionDays.contains($0) ? $0 : nil }
    return ClipboardPage(enabled: page.enabled, items: items, retention_days: retention)
  }
  func setClipboardPinned(id: String, pinned: Bool, token: String) async throws {
    guard Self.validClipboardID(id) else { throw Failure(status: 400) }
    struct Body: Encodable { let pinned: Bool }
    _ = try await request("PUT", "/v1/users/me/clipboard/" + id + "/pin", token: token,
                          body: JSONEncoder().encode(Body(pinned: pinned)))
  }
  /// `days` 取 `clipboardRetentionDays` 中的一个值；0 表示保留到删除为止。
  func setClipboardRetention(days: Int, token: String) async throws {
    guard Self.clipboardRetentionDays.contains(days) else { throw Failure(status: 400) }
    struct Body: Encodable { let days: Int }
    _ = try await request("PUT", "/v1/users/me/clipboard/retention", token: token,
                          body: JSONEncoder().encode(Body(days: days)))
  }
  func setClipboardEnabled(_ enabled: Bool, token: String) async throws {
    struct Body: Encodable { let enabled: Bool }
    _ = try await request("PUT", "/v1/users/me/clipboard/settings", token: token,
                          body: JSONEncoder().encode(Body(enabled: enabled)))
  }
  func addClipboard(_ text: String, token: String) async throws -> ClipboardItem {
    guard Self.validClipboardText(text) else { throw Failure(status: 400) }
    struct Body: Encodable { let text: String }
    let item: ClipboardItem = try await json("POST", "/v1/users/me/clipboard", token: token,
                                             body: JSONEncoder().encode(Body(text: text)))
    guard Self.validClipboardItem(item) else { throw Failure(status: 0) }
    return item
  }
  func deleteClipboard(id: String? = nil, token: String) async throws {
    if let id {
      guard Self.validClipboardID(id) else { throw Failure(status: 400) }
    }
    _ = try await request("DELETE", "/v1/users/me/clipboard" + (id.map { "/" + $0 } ?? ""), token: token)
  }

  private static func validClipboardText(_ text: String) -> Bool {
    !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && text.utf16.count <= 4000
      && !text.unicodeScalars.contains { scalar in
        scalar.properties.generalCategory == .control && ![9, 10, 13].contains(scalar.value)
      }
  }

  private static func validClipboardSearch(_ search: String) -> Bool {
    search.utf8.count <= 1024
      && !search.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }

  private static func validClipboardID(_ id: String) -> Bool {
    id.utf8.count == 64 && id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
  }

  private static func validClipboardDevice(_ device: String) -> Bool {
    device.utf8.count <= 128 && !device.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }

  private static func validClipboardItem(_ item: ClipboardItem) -> Bool {
    validClipboardID(item.id)
      && validClipboardText(item.text)
      && !item.updated_at.isEmpty
      && item.updated_at.utf8.count <= 128
      && !item.updated_at.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }
}

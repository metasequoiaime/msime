import Foundation

extension BackendAccountClient {
  struct ClipboardItem: Decodable, Identifiable, Sendable {
    let id: String
    let text: String
    let updated_at: String
  }
  struct ClipboardPage: Decodable, Sendable {
    let enabled: Bool
    let items: [ClipboardItem]
  }
  func clipboard(token: String, search: String = "") async throws -> ClipboardPage {
    var components = URLComponents()
    components.path = "/v1/users/me/clipboard"
    components.queryItems = [URLQueryItem(name: "q", value: search)]
    guard let path = Self.encodedPath(components) else { throw Failure(status: 0) }
    let page: ClipboardPage = try await json("GET", path, token: token)
    guard page.items.count <= 50,
          page.items.allSatisfy(Self.validClipboardItem) else { throw Failure(status: 0) }
    return page
  }
  func setClipboardEnabled(_ enabled: Bool, token: String) async throws {
    struct Body: Encodable { let enabled: Bool }
    _ = try await request("PUT", "/v1/users/me/clipboard/settings", token: token,
                          body: JSONEncoder().encode(Body(enabled: enabled)))
  }
  func addClipboard(_ text: String, token: String) async throws -> ClipboardItem {
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          text.utf16.count <= 4000, !text.contains("\0") else { throw Failure(status: 400) }
    struct Body: Encodable { let text: String }
    let item: ClipboardItem = try await json("POST", "/v1/users/me/clipboard", token: token,
                                             body: JSONEncoder().encode(Body(text: text)))
    guard Self.validClipboardItem(item) else { throw Failure(status: 0) }
    return item
  }
  func deleteClipboard(id: String? = nil, token: String) async throws {
    if let id {
      guard id.utf8.count == 64, id.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else { throw Failure(status: 400) }
    }
    _ = try await request("DELETE", "/v1/users/me/clipboard" + (id.map { "/" + $0 } ?? ""), token: token)
  }

  private static func validClipboardItem(_ item: ClipboardItem) -> Bool {
    item.id.utf8.count == 64
      && item.id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
      && !item.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && item.text.utf16.count <= 4000
      && !item.text.unicodeScalars.contains { scalar in
        scalar.value == 0 || (scalar.properties.generalCategory == .control
          && ![9, 10, 13].contains(scalar.value))
      }
      && !item.updated_at.isEmpty
      && item.updated_at.utf8.count <= 128
      && !item.updated_at.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }
}

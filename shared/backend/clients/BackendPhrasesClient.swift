import Foundation
import CoreFoundation

/// 无编码常用语的整列表同步（`/v1/users/me/phrases`），与偏好同一套 revision CAS：PUT 带上读到的 `revision` 整份替换，别的设备先写了就回 409 `revision_conflict`，调用方重新拉取、按 `BackendSyncPolicy` 合并后再写。解析规则与 Android 的 `SyncApi.parsePhrases` 相同。
extension BackendAccountClient {
  struct CloudPhrases: Equatable, Sendable {
    let revision: Int64
    let phrases: [BackendSyncPolicy.Phrase]
  }

  func phrases(token: String) async throws -> CloudPhrases {
    try Self.parsePhrases(try await request("GET", "/v1/users/me/phrases", token: token))
  }

  /// 整份替换，先经 `BackendSyncPolicy.normalized` 去重截断；`revision` 是读到的那一版。
  func putPhrases(revision: Int64, phrases: [BackendSyncPolicy.Phrase], token: String) async throws -> CloudPhrases {
    struct Row: Encodable { let id: String; let text: String; let group: String; let position: Int }
    struct Body: Encodable { let revision: Int64; let phrases: [Row] }
    guard revision >= 0 else { throw Failure(status: 400) }
    let rows = BackendSyncPolicy.normalized(phrases).map { Row(id: $0.id, text: $0.text, group: $0.group, position: $0.position) }
    let body = try JSONEncoder().encode(Body(revision: revision, phrases: rows))
    guard body.count <= 1024 * 1024 else { throw Failure(status: 400) }
    return try Self.parsePhrases(try await request("PUT", "/v1/users/me/phrases", token: token, body: body))
  }

  /// 成功的响应总是带着 `revision`（非负整数）和 `phrases` 数组（可以为空），最多 500 条；每条的 id 和正文必须是字符串，分组可以没有，位置不合规时按响应里的顺序。读不出时按无效响应处理。
  static func parsePhrases(_ data: Data) throws -> CloudPhrases {
    let invalid = Failure(status: 0)
    guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let revision = strictInteger(root["revision"]), revision >= 0,
          let rows = root["phrases"] as? [Any], rows.count <= BackendSyncPolicy.maxPhrases else { throw invalid }
    var phrases: [(phrase: BackendSyncPolicy.Phrase, index: Int)] = []
    for (index, value) in rows.enumerated() {
      guard let row = value as? [String: Any], let id = row["id"] as? String, let text = row["text"] as? String
      else { throw invalid }
      let group: String
      switch row["group"] {
      case nil, is NSNull: group = ""
      case let value as String: group = value
      default: throw invalid
      }
      let position = strictInteger(row["position"]).flatMap { (0..<Int64(BackendSyncPolicy.maxPhrases)).contains($0) ? Int($0) : nil }
      phrases.append((BackendSyncPolicy.Phrase(id: id, text: text, group: group, position: position ?? index), index))
    }
    // 稳定排序：位置相同的按响应里的顺序。
    phrases.sort { ($0.phrase.position, $0.index) < ($1.phrase.position, $1.index) }
    return CloudPhrases(revision: revision, phrases: phrases.map(\.phrase))
  }

  /// JSON 整数：不接受布尔值、浮点数和超出 Int64 的值。
  private static func strictInteger(_ value: Any?) -> Int64? {
    guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
          !["f", "d"].contains(String(cString: number.objCType)) else { return nil }
    return Int64(number.stringValue)
  }
}

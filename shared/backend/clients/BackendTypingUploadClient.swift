import Foundation

/// 「上传到云端」和「绑定 QQ 群助手」用到的账号接口：`PUT` / `DELETE /v1/users/me/typing-statistics`，以及 `GET` / `POST /v1/users/me/qq-links` 和 `DELETE /v1/users/me/qq-links/{id}`，与 Android 的 `TypingUploadApi` 相同。
///
/// 上传的请求体不在这里拼：共享层 `msime_client_typing_statistics` 的 `upload_request` 读本机统计、组好请求体、管设备标识和十分钟节流，这里只负责带着真实账号的令牌发出去（匿名账号不上传、不绑定）。各种失败的说法与共享设置页（`typing-statistics-upload.tsx`、`qq-link-card.tsx`）和 Android 一致。
extension BackendAccountClient {
  static let typingStatisticsPath = "/v1/users/me/typing-statistics"
  static let qqLinksPath = "/v1/users/me/qq-links"
  /// 服务端的绑定码是 8 位数字。
  static let qqLinkCodeDigits = 8
  /// 服务端每个账号最多绑定 10 个群友，多出来的行不显示。
  private static let maxQQLinks = 10
  /// 共享层给的请求体最大 64 KiB，与服务端的上限相同。
  private static let maxTypingUploadBytes = 64 * 1024

  /// 一条绑定：服务端只存群和成员标识的摘要，所以只有编号和绑定时间。
  struct QQLink: Equatable, Identifiable, Sendable {
    let id: String
    let createdAt: String
  }

  /// 把共享层给的请求体发出去，服务端回 204。`path` 必须是共享层交回的那一个，别的路径一律拒绝。
  func uploadTypingStatistics(path: String, body: Data, token: String) async throws {
    guard path == Self.typingStatisticsPath, !body.isEmpty, body.count <= Self.maxTypingUploadBytes
    else { throw Failure(status: 400) }
    _ = try await request("PUT", path, token: token, body: body)
  }

  /// 删除这个账号在云端的全部打字统计；没有数据也是 204。
  func deleteTypingStatistics(token: String) async throws {
    _ = try await request("DELETE", Self.typingStatisticsPath, token: token)
  }

  /// 本人的绑定，新的在前。
  func qqLinks(token: String) async throws -> [QQLink] {
    try Self.parseQQLinks(try await request("GET", Self.qqLinksPath, token: token))
  }

  /// 用绑定码绑定。`code` 先经 `normalizedQQLinkCode`，不合规时抛 400 `invalid_qq_link_code`，不联网。
  func redeemQQLink(code: String, token: String) async throws -> QQLink {
    guard let digits = Self.normalizedQQLinkCode(code) else { throw Failure(status: 400, code: "invalid_qq_link_code") }
    let body = try JSONSerialization.data(withJSONObject: ["code": digits])
    let data = try await request("POST", Self.qqLinksPath, token: token, body: body)
    guard let link = Self.qqLink(try? JSONSerialization.jsonObject(with: data)) else { throw Failure(status: 0) }
    return link
  }

  /// 解除一条绑定。
  func deleteQQLink(id: String, token: String) async throws {
    guard Self.validQQLinkID(id) else { throw Failure(status: 400) }
    _ = try await request("DELETE", Self.qqLinksPath + "/" + id, token: token)
  }

  /// 响应 `{links:[{id,created_at}]}`：没有 `links` 数组时按无效响应处理，格式不对的行跳过，最多 10 行。
  static func parseQQLinks(_ data: Data) throws -> [QQLink] {
    guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let rows = root["links"] as? [Any] else { throw Failure(status: 0) }
    return Array(rows.compactMap(qqLink).prefix(maxQQLinks))
  }

  private static func qqLink(_ value: Any?) -> QQLink? {
    guard let row = value as? [String: Any], let id = row["id"] as? String, validQQLinkID(id),
          let createdAt = row["created_at"] as? String, createdAt.count <= 64 else { return nil }
    return QQLink(id: id, createdAt: createdAt)
  }

  /// 服务端的编号是不以 0 开头的十进制整数，最多 19 位。
  static func validQQLinkID(_ id: String) -> Bool {
    guard (1...19).contains(id.count), id.first != "0" else { return false }
    return id.allSatisfy { ("0"..."9").contains($0) }
  }

  /// 去掉空白和连字符后正好 8 位 ASCII 数字时返回这 8 位，否则 nil。和服务端、共享设置页、Android 的规则相同。
  static func normalizedQQLinkCode(_ code: String) -> String? {
    var digits = ""
    for character in code {
      if character.isWhitespace || character == "-" { continue }
      guard ("0"..."9").contains(character), digits.count < qqLinkCodeDigits else { return nil }
      digits.append(character)
    }
    return digits.count == qqLinkCodeDigits ? digits : nil
  }

  /// 绑定失败时的说法，与共享设置页的 `qqLinkMessage` 相同。
  static func qqLinkMessage(_ error: Error) -> String {
    if let failure = error as? Failure {
      switch failure.code {
      case "invalid_qq_link_code": return "绑定码是 8 位数字，请检查后重试。"
      case "qq_link_code_not_found":
        return "绑定码不对或已过期（10 分钟内有效，只能用一次）。请在 QQ 群里重新发 /bind 拿一个新的。"
      case "qq_link_limit": return "这个账号已经绑定了 10 个 QQ 群友，先解绑一个再试。"
      default: break
      }
      if failure.status == 429 { return "尝试次数太多，请一小时后再试。" }
    }
    return accountServiceMessage(error)
  }

  /// 上传、删除失败时的说法，与共享设置页的 `uploadErrorMessage` 相同；`enabling` 是这次在打开还是关闭。
  static func typingUploadMessage(_ error: Error, enabling: Bool) -> String {
    if let failure = error as? Failure {
      if failure.status == 401 { return "先在「我的」页登录水杉账号，再打开上传。" }
      if failure.status == 429 { return "上传过于频繁，过一会儿会自动再传。" }
    }
    return enabling ? "已打开上传，但这次没有传上去，稍后会自动再试。" : "已关闭上传，但云端的数据没有删掉，请联网后打开再关闭一次。"
  }

  /// 列表和解绑失败时的说法，与 Android 的 `TypingUploadApi.generalMessage` 相同。
  static func accountServiceMessage(_ error: Error) -> String {
    if error is URLError { return "连不上服务器，请检查网络后再试。" }
    guard let failure = error as? Failure else { return "没有成功，请稍后再试。" }
    switch failure.status {
    case 401: return "登录已失效，请重新登录后再试。"
    case 404, 503: return "这项服务暂时不可用，请稍后再试。"
    default:
      let code = failure.code.map { " " + $0 } ?? ""
      return "没有成功，请稍后再试（HTTP \(failure.status)\(code)）。"
    }
  }
}

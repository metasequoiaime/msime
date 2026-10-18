import Foundation

/// 「绑定 QQ 群助手」的三个账号操作，都经 Rust，只用登录账号。没登录时抛 `Failure.signedOut`，不联网。
enum QQLinksService {
  enum Failure: Error { case signedOut }

  /// 一条绑定：服务端只存群和成员标识的摘要，所以只有编号和绑定时间。
  struct Link: Equatable, Identifiable, Sendable {
    let id: String
    let createdAt: String
  }

  /// 本人的绑定，新的在前。
  static func links(_ rust: () throws -> BackendRustAccount = { try BackendRustAccount.current() }) async throws -> [Link] {
    let value = try BackendRustAccount.object(try await signedIn(rust()).account(["operation": "qq_links"]))
    guard let rows = value["links"] as? [Any] else { throw BackendRustClient.Failure.invalidResponse }
    return rows.compactMap(link)
  }

  /// 用绑定码绑定。码的清洗（去掉空白和连字符）和校验在 Rust。
  static func redeem(_ code: String, _ rust: () throws -> BackendRustAccount = { try BackendRustAccount.current() }) async throws {
    _ = try await signedIn(rust()).account(["operation": "qq_link_redeem", "code": code])
  }

  /// 解除一条绑定。
  static func delete(_ id: String, _ rust: () throws -> BackendRustAccount = { try BackendRustAccount.current() }) async throws {
    _ = try await signedIn(rust()).account(["operation": "qq_link_delete", "id": id])
  }

  private static func signedIn(_ account: BackendRustAccount) async throws -> BackendRustAccount {
    guard try await account.status() != nil else { throw Failure.signedOut }
    return account
  }

  private static func link(_ value: Any) -> Link? {
    guard let row = value as? [String: Any], let id = row["id"] as? String, !id.isEmpty,
          let createdAt = row["created_at"] as? String else { return nil }
    return Link(id: id, createdAt: createdAt)
  }

  /// 去掉空白和连字符后正好 8 位 ASCII 数字时返回这 8 位，否则 nil。只用来决定「绑定」按钮能不能点，Rust 按同样的规则再清洗、校验一次。
  static func normalizedCode(_ code: String) -> String? {
    var digits = ""
    for character in code {
      if character.isWhitespace || character == "-" { continue }
      guard ("0"..."9").contains(character), digits.count < 8 else { return nil }
      digits.append(character)
    }
    return digits.count == 8 ? digits : nil
  }

  /// 失败时的说法，与共享设置页的 `qqLinkMessage` 和 Android 的 `TypingUploadApi.generalMessage` 相同；`redeeming` 是这次在绑定。
  static func message(_ error: Error, redeeming: Bool = false) -> String {
    guard let failure = error as? BackendRustClient.Failure else { return "没有成功，请稍后再试。" }
    switch failure.code {
    case "account_invalid" where redeeming: return "绑定码是 8 位数字，请检查后重试。"
    case "account_qq_link_code_not_found":
      return "绑定码不对或已过期（10 分钟内有效，只能用一次）。请在 QQ 群里重新发 /bind 拿一个新的。"
    case "account_qq_link_limit": return "这个账号已经绑定了 10 个 QQ 群友，先解绑一个再试。"
    case "account_rate_limited": return redeeming ? "尝试次数太多，请一小时后再试。" : "操作过于频繁，请稍后再试。"
    case "account_unauthorized": return "登录已失效，请重新登录后再试。"
    case "account_network": return "连不上服务器，请检查网络后再试。"
    case "account_not_found", "account_unavailable": return "这项服务暂时不可用，请稍后再试。"
    default: return failure.localizedDescription
    }
  }
}

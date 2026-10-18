import Foundation

/// Stable opaque identity for one saved login. This is shared by the backend
/// notification and the native input method's cache scope; neither side needs
/// to expose the account ID or session UUID to the other.
func backendAccountTranslationMarker(userID: String, sessionID: UUID?) -> UInt64 {
  let identity = "\(userID)|\(sessionID?.uuidString ?? "")"
  var hash: UInt64 = 14695981039346656037
  for byte in identity.utf8 {
    hash ^= UInt64(byte)
    hash &*= 1099511628211
  }
  return hash
}

/// Synchronous account state used by native settings surfaces that only need to
/// decide whether to offer sign-in. The access token itself never crosses this
/// bridge.
@_cdecl("MSIMEBackendAccountSignedIn")
public func msimeBackendAccountSignedIn() -> Bool {
  if ((try? BackendDesktopSessionFile().load()) ?? nil) != nil { return true }
  return ((try? BackendAnonymousAccount.sessionStorage().load()) ?? nil) != nil
}

/// Opaque, process-local scope for account candidate glosses. The input method only needs to
/// distinguish login generations; it never receives the account ID or session UUID itself.
@_cdecl("MSIMEBackendAccountTranslationMarker")
public func msimeBackendAccountTranslationMarker() -> UInt64 {
  guard let saved = (try? BackendDesktopSessionFile().load()) ?? nil else { return 0 }
  return backendAccountTranslationMarker(userID: saved.tokens.user.id, sessionID: saved.sessionID)
}

/// 输入源第一次激活后为只用键盘的服务（候选释义等）备好本机匿名账号。尽力而为：打字不等账号，断网也不拦输入。由 Rust 的 `ensure_anonymous` 注册（#8201 迁移计划 4b）：已登录时不调；已有匿名会话时 Rust 只读一次存储、不联网；读不了登录状态时这次不注册，免得把读不了当作没有登录。
private actor AnonymousAccountBootstrap {
  static let shared = AnonymousAccountBootstrap()
  private var attempted = false

  func runOnce() async {
    guard !attempted else { return }
    attempted = true
    guard let rust = BackendRustAccount.shared else { return }
    do { if try await rust.status() != nil { return } } catch { return }
    _ = try? await rust.account(["operation": "ensure_anonymous"])
  }
}

@_cdecl("MSIMEEnsureAnonymousAccount")
public func msimeEnsureAnonymousAccount() {
  Task { await AnonymousAccountBootstrap.shared.runOnce() }
}

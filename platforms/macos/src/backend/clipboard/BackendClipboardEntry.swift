import Foundation

@MainActor
enum BackendClipboardEntry {
  /// 已登录时为这个账号打开云剪贴板，没有登录或读不了登录状态时请用户登录。`status` 是 Rust 的 `status`（`BackendRustAccount.status`）。
  static func open(status: () async throws -> BackendAccountClient.User?,
                   present: (String) -> Void, signIn: () -> Void) async {
    do {
      let user = try await status()
      try Task.checkCancellation()
      guard let user else { signIn(); return }
      present(user.id)
    } catch is CancellationError {
    } catch {
      signIn()
    }
  }
}

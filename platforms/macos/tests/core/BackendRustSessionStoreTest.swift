// 真的 Rust 宿主库：macOS 输入法交给后端 dylib 的同一张函数表（`msime_rust_client_linked`），注册注入存储，Rust 经回调读写 `account-session.json` 和 App Group 文件（都在临时目录里）、拿 `account-refresh.lock`。只用本地操作，不联网，不碰钥匙串和真正的状态目录。
import CMSIMEClient
import Darwin
import Foundation

private func check(_ condition: @autoclosure () -> Bool, _ message: @autoclosure () -> String, line: UInt = #line) {
  if !condition() {
    FileHandle.standardError.write(Data("BackendRustSessionStoreTest:\(line): \(message())\n".utf8))
    exit(1)
  }
}

private func session(_ user: String, token: Character) -> BackendSavedSession {
  let tokens = BackendAccountClient.Tokens(access_token: String(repeating: token, count: 64),
    refresh_token: String(repeating: "f", count: 64), token_type: "Bearer", expires_in: 900,
    user: BackendAccountClient.User(id: user, display_name: "合成用户", created_at: "2026-10-18"))
  // 令牌是 64 位小写十六进制（`BackendAccountClient.validate`）。离过期还有 10 分钟：Rust 把 2001 年起的秒数读对了才不会去刷新（刷新要联网）。
  return BackendSavedSession(tokens: tokens, expiresAt: Date().addingTimeInterval(600), sessionID: UUID())
}

@main
enum BackendRustSessionStoreTest {
  static func main() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-rust-store-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let state = root.appendingPathComponent("state", isDirectory: true)
    let group = root.appendingPathComponent("group", isDirectory: true)
    try FileManager.default.createDirectory(at: group, withIntermediateDirectories: true)

    check(BackendRustClient.install(msime_rust_client_linked()), "the linked table was refused")
    guard let client = BackendRustClient.shared else { return check(false, "no installed client") }
    let layout = BackendRustStoreLayout(
      account: .ownerOnlyFile(state.appendingPathComponent(BackendDesktopSessionFile.fileName)),
      clipboardKeystore: .ownerOnlyFile(state.appendingPathComponent(BackendDesktopClipboardKeystoreFile.fileName)),
      anonymous: .localStore(directory: group, fileName: BackendAnonymousAccount.sessionFileName),
      anonymousIdentity: .localStore(directory: group, fileName: BackendAnonymousAccount.identityFileName),
      accountLock: state.appendingPathComponent("account-refresh.lock"),
      anonymousLock: group.appendingPathComponent("backend-account-refresh.lock"))
    let id: UInt64
    switch BackendRustSessionStore(layout: layout, backgroundActivity: nil).register(with: client, clientAgent: "msime-macos/0.52.0 (Mac; macOS 26; edition=full)") {
    case .success(let value): id = value
    case .failure(let failure): return check(false, "register failed: \(failure.code)")
    }

    func status() async throws -> String? {
      let value = try await client.call(.account, ["session_store": id, "action": ["operation": "status"]])
      let object = try JSONSerialization.jsonObject(with: value) as? [String: Any]
      return (object?["user"] as? [String: Any])?["id"] as? String
    }
    func accessToken() throws -> [String: Any] {
      let request = try JSONSerialization.data(withJSONObject: ["session_store": id])
      let raw = request.withUnsafeBytes { msime_client_account_access_token($0.bindMemory(to: UInt8.self).baseAddress, $0.count) }
      return try JSONSerialization.jsonObject(with: client.take(raw).get()) as? [String: Any] ?? [:]
    }

    let initially = try await status()
    check(initially == nil, "signed in before any session was stored")

    // Swift（输入法的 `BackendDesktopSessionFile`）写的登录会话，Rust 读出并按布局 1 解码。
    try BackendDesktopSessionFile(directory: state).save(session("synthetic-user", token: "a"))
    let signedIn = try await status()
    check(signedIn == "synthetic-user", "status read \(signedIn ?? "nil")")
    var token = try accessToken()
    check(token["access_token"] as? String == String(repeating: "a", count: 64) && token["anonymous"] as? Bool == false,
          "access_token \(token)")

    // forget 删掉登录会话和它的密钥串；匿名会话（Swift 存进 App Group 的）留着，access_token 退回它。
    try BackendDesktopSessionFile.writeOwnerOnly(Data("synthetic-keystore\n".utf8), named: BackendDesktopClipboardKeystoreFile.fileName, in: state)
    try BackendLocalStore(fileName: BackendAnonymousAccount.sessionFileName, directory: group).save(session("synthetic-anonymous", token: "e"))
    _ = try await client.call(.account, ["session_store": id, "action": ["operation": "forget"]])
    check(!FileManager.default.fileExists(atPath: state.appendingPathComponent(BackendDesktopSessionFile.fileName).path), "session survived forget")
    check(!FileManager.default.fileExists(atPath: state.appendingPathComponent(BackendDesktopClipboardKeystoreFile.fileName).path),
          "keystore survived forget")
    let afterForget = try await status()
    check(afterForget == nil, "still signed in after forget")
    token = try accessToken()
    check(token["access_token"] as? String == String(repeating: "e", count: 64) && token["anonymous"] as? Bool == true,
          "anonymous access_token \(token)")

    check(client.unregister(id) == .success(true), "unregister")
    print("BackendRustSessionStoreTest: ok")
  }
}

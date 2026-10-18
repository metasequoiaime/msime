import CMSIMEClient
import Foundation
import Security
import XCTest

/// 真的 Rust 宿主库：iOS 用链进来的 C 函数填函数表（`msime_rust_client_linked`），注册注入存储，Rust 经回调读写钥匙串（这里是内存替身，测试进程没有钥匙串权限）和 App Group 文件（临时目录）。只用本地操作，不联网。
final class RustSessionStoreTests: XCTestCase {
  private final class MemoryKeychain: @unchecked Sendable {
    private let lock = NSLock()
    private var items: [String: Data] = [:]
    private static func key(_ query: [String: Any]) -> String {
      "\(query[kSecAttrService as String] as? String ?? "")|\(query[kSecAttrAccount as String] as? String ?? "")"
    }
    func item(_ account: String) -> Data? {
      lock.lock(); defer { lock.unlock() }
      return items["\(BackendKeychain.defaultService)|\(account)"]
    }
    func set(_ account: String, _ data: Data) {
      lock.lock(); items["\(BackendKeychain.defaultService)|\(account)"] = data; lock.unlock()
    }
    var calls: BackendRustKeychain {
      BackendRustKeychain(
        copyMatching: { [self] query in
          lock.lock(); defer { lock.unlock() }
          return items[Self.key(query)].map { (errSecSuccess, $0) } ?? (errSecItemNotFound, nil)
        },
        update: { [self] query, attributes in
          lock.lock(); defer { lock.unlock() }
          guard items[Self.key(query)] != nil else { return errSecItemNotFound }
          items[Self.key(query)] = attributes[kSecValueData as String] as? Data
          return errSecSuccess
        },
        add: { [self] attributes in
          lock.lock(); defer { lock.unlock() }
          guard items[Self.key(attributes)] == nil else { return errSecDuplicateItem }
          items[Self.key(attributes)] = attributes[kSecValueData as String] as? Data
          return errSecSuccess
        },
        delete: { [self] query in
          lock.lock(); defer { lock.unlock() }
          return items.removeValue(forKey: Self.key(query)) == nil ? errSecItemNotFound : errSecSuccess
        })
    }
  }

  private var root: URL!

  override func setUpWithError() throws {
    root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-rust-store-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws { try? FileManager.default.removeItem(at: root) }

  private func session(_ user: String, token: Character) -> BackendSavedSession {
    let tokens = BackendAccountClient.Tokens(access_token: String(repeating: token, count: 64),
      refresh_token: String(repeating: "f", count: 64), token_type: "Bearer", expires_in: 900,
      user: BackendAccountClient.User(id: user, display_name: "合成用户", created_at: "2026-10-18"))
    // 令牌是 64 位小写十六进制（`BackendAccountClient.validate`）。离过期还有 10 分钟：Rust 把 2001 年起的秒数读对了才不会去刷新（刷新要联网）。
    return BackendSavedSession(tokens: tokens, expiresAt: Date().addingTimeInterval(600), sessionID: UUID())
  }

  private func accessToken(_ store: UInt64) throws -> [String: Any] {
    let request = try JSONSerialization.data(withJSONObject: ["session_store": store])
    let raw = request.withUnsafeBytes { msime_client_account_access_token($0.bindMemory(to: UInt8.self).baseAddress, $0.count) }
    let client = try XCTUnwrap(BackendRustClient(table: msime_rust_client_linked()))
    let value = try client.take(raw).get()
    return try XCTUnwrap(JSONSerialization.jsonObject(with: value) as? [String: Any])
  }

  /// iOS 的登录会话和密钥串是 App Group 访问组里的钥匙串项，两个 item 共用 App Group 的 `backend-account-refresh.lock`，与 `BackendKeychain`、`BackendAccountSession.defaultRefreshLock` 相同（#8201 迁移计划 4a）。
  func testStandardLayoutIsTheKeychainAndAppGroupOfTheSwiftSessions() {
    let layout = BackendRustStoreLayout.standard
    guard case let .keychain(service, account, group) = layout.account,
          case let .keychain(keystoreService, keystoreAccount, keystoreGroup) = layout.clipboardKeystore else {
      return XCTFail("iOS keeps the account in the keychain")
    }
    XCTAssertEqual(service, "app.msime.backend.account")
    XCTAssertEqual(account, "https://api.msime.app")
    XCTAssertEqual(group, MSIMEAppEdition.appGroupIdentifier)
    XCTAssertEqual(keystoreService, "app.msime.backend.account")
    XCTAssertEqual(keystoreAccount, "cloud-clipboard-keystore")
    XCTAssertEqual(keystoreGroup, MSIMEAppEdition.appGroupIdentifier)
    let lock = (BackendAccountSession.defaultRefreshLock as? BackendFileRefreshLock)?.url
    XCTAssertEqual(layout.accountLock, lock)
    XCTAssertEqual(layout.anonymousLock, lock)
  }

  func testRustReadsAndClearsSwiftSessionsThroughTheInjectedStore() async throws {
    let client = try XCTUnwrap(BackendRustClient(table: msime_rust_client_linked()))
    let agentValue = try await client.call(.clientAgent, ["platform": "ios", "version": "0.52.0", "device": "iPhone",
                                                          "system": "iOS 27", "edition": "full"])
    let agent = try XCTUnwrap(JSONSerialization.jsonObject(with: agentValue, options: [.fragmentsAllowed]) as? String)
    XCTAssertTrue(agent.hasPrefix("msime-ios/0.52.0 ("), agent)

    let keychain = MemoryKeychain()
    let lock = root.appendingPathComponent("backend-account-refresh.lock")
    let layout = BackendRustStoreLayout(
      account: .keychain(service: BackendKeychain.defaultService, account: BackendKeychain.sessionAccount, accessGroup: "group.synthetic"),
      clipboardKeystore: .keychain(service: BackendKeychain.defaultService, account: BackendKeychainClipboardKeystore.account,
                                   accessGroup: "group.synthetic"),
      anonymous: .localStore(directory: root, fileName: BackendAnonymousAccount.sessionFileName),
      anonymousIdentity: .localStore(directory: root, fileName: BackendAnonymousAccount.identityFileName),
      accountLock: lock, anonymousLock: lock)
    let id = try BackendRustSessionStore(layout: layout, keychain: keychain.calls, backgroundActivity: nil).register(with: client, clientAgent: agent).get()
    defer { _ = client.unregister(id) }

    func status() async throws -> String? {
      let value = try await client.call(.account, ["session_store": id, "action": ["operation": "status"]])
      let object = try XCTUnwrap(JSONSerialization.jsonObject(with: value) as? [String: Any])
      return (object["user"] as? [String: Any])?["id"] as? String
    }
    let initially = try await status()
    XCTAssertNil(initially)

    // Swift 存进 App Group 的匿名会话，Rust 原样读出、按布局 1 解码。
    try BackendLocalStore(fileName: BackendAnonymousAccount.sessionFileName, directory: root).save(session("synthetic-anonymous", token: "e"))
    var token = try accessToken(id)
    XCTAssertEqual(token["access_token"] as? String, String(repeating: "e", count: 64))
    XCTAssertEqual(token["anonymous"] as? Bool, true)

    // Swift 存进钥匙串的登录会话优先。
    keychain.set(BackendKeychain.sessionAccount, try JSONEncoder().encode(session("synthetic-user", token: "a")))
    keychain.set(BackendKeychainClipboardKeystore.account, Data("synthetic-keystore".utf8))
    let signedIn = try await status()
    XCTAssertEqual(signedIn, "synthetic-user")
    token = try accessToken(id)
    XCTAssertEqual(token["access_token"] as? String, String(repeating: "a", count: 64))
    XCTAssertEqual(token["anonymous"] as? Bool, false)

    // forget 在 lock(ACCOUNT) 里删掉登录会话和它的密钥串，匿名会话留着。
    _ = try await client.call(.account, ["session_store": id, "action": ["operation": "forget"]])
    XCTAssertNil(keychain.item(BackendKeychain.sessionAccount))
    XCTAssertNil(keychain.item(BackendKeychainClipboardKeystore.account))
    XCTAssertTrue(FileManager.default.fileExists(atPath: lock.path))
    let afterForget = try await status()
    XCTAssertNil(afterForget)
    XCTAssertEqual(try accessToken(id)["anonymous"] as? Bool, true)
  }
}

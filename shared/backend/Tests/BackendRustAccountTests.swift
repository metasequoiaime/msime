import CMSIMEClient
import Foundation
import XCTest
@testable import MSIMEBackend

/// 替身 Rust：记下注册时交来的 client_agent 和每个请求，按 `replies` 给回答。C 回调不能捕获变量，状态都放在这里；测试类内的用例串行运行。
private enum FakeAccountRust {
  static let lock = NSLock()
  static var registrations: [String?] = []
  static var agentRequests: [[String: Any]] = []
  static var requests: [(entry: String, body: [String: Any])] = []
  /// 原始请求字节：`cloud_clipboard_source` 的请求是一段目录路径，不是 JSON。
  static var raw: [(entry: String, bytes: Data)] = []
  static var agentReply = #"{"ok":true,"value":"msime-macos/0.52.0 (Mac14,2; macOS 26.0; edition=full)"}"#
  static var replies: [String: String] = [:]

  static func record(_ entry: String, _ bytes: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>? {
    let body = bytes.flatMap { (try? JSONSerialization.jsonObject(with: Data(bytes: $0, count: length))) as? [String: Any] } ?? [:]
    lock.lock(); defer { lock.unlock() }
    requests.append((entry, body))
    raw.append((entry, bytes.map { Data(bytes: $0, count: length) } ?? Data()))
    let operation = (body["action"] as? [String: Any])?["operation"] as? String ?? ""
    return strdup(replies["\(entry).\(operation)"] ?? #"{"ok":false,"error":"account_invalid"}"#)
  }

  static func table() -> msime_rust_client {
    var table = msime_rust_client()
    table.version = MSIME_RUST_CLIENT_VERSION
    table.size = UInt32(MemoryLayout<msime_rust_client>.stride)
    table.string_free = { free($0) }
    table.session_store_register = { store in
      let agent = store?.pointee.client_agent.map { String(cString: $0) }
      FakeAccountRust.lock.lock(); FakeAccountRust.registrations.append(agent); FakeAccountRust.lock.unlock()
      return strdup(#"{"ok":true,"value":{"session_store":9}}"#)
    }
    table.session_store_unregister = { _ in strdup(#"{"ok":true,"value":true}"#) }
    table.client_agent = { bytes, length in
      let body = bytes.flatMap { (try? JSONSerialization.jsonObject(with: Data(bytes: $0, count: length))) as? [String: Any] } ?? [:]
      FakeAccountRust.lock.lock(); defer { FakeAccountRust.lock.unlock() }
      FakeAccountRust.agentRequests.append(body)
      return strdup(FakeAccountRust.agentReply)
    }
    table.account = { FakeAccountRust.record("account", $0, $1) }
    table.account_cloud_clipboard = { FakeAccountRust.record("cloud_clipboard", $0, $1) }
    table.cloud_clipboard_source = { FakeAccountRust.record("cloud_clipboard_source", $0, $1) }
    table.relay_cloud_clipboard = { FakeAccountRust.record("relay_cloud_clipboard", $0, $1) }
    table.cloud_dictionary = { FakeAccountRust.record("cloud_dictionary", $0, $1) }
    return table
  }

  static func reset() {
    lock.lock(); defer { lock.unlock() }
    registrations = []; agentRequests = []; requests = []; raw = []; replies = [:]
    agentReply = #"{"ok":true,"value":"msime-macos/0.52.0 (Mac14,2; macOS 26.0; edition=full)"}"#
  }

  static var rawRequests: [(entry: String, bytes: Data)] {
    lock.lock(); defer { lock.unlock() }
    return raw
  }

  static func snapshot() -> (registrations: [String?], agents: [[String: Any]], requests: [(entry: String, body: [String: Any])]) {
    lock.lock(); defer { lock.unlock() }
    return (registrations, agentRequests, requests)
  }
}

final class BackendRustAccountTests: XCTestCase {
  override func setUp() { FakeAccountRust.reset() }

  private var directory: URL!

  override func setUpWithError() throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-rust-account-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws { try? FileManager.default.removeItem(at: directory) }

  private var layout: BackendRustStoreLayout {
    BackendRustStoreLayout(account: .localStore(directory: directory, fileName: "account-session.json"),
                           clipboardKeystore: .localStore(directory: directory, fileName: "cloud-clipboard-keystore"),
                           anonymous: .localStore(directory: directory, fileName: BackendAnonymousAccount.sessionFileName),
                           anonymousIdentity: .localStore(directory: directory, fileName: BackendAnonymousAccount.identityFileName),
                           accountLock: directory.appendingPathComponent("account-refresh.lock"),
                           anonymousLock: directory.appendingPathComponent("backend-account-refresh.lock"))
  }

  private let agent = BackendRustAccount.Agent(platform: "macos", version: "0.52.0", device: "Mac14,2", system: "macOS 26.0", edition: "full")

  func testRegistrationReportsTheAgentRustAssembled() throws {
    let client = try XCTUnwrap(BackendRustClient(table: FakeAccountRust.table()))
    let account = try BackendRustAccount.register(client: client, agent: agent, layout: layout, backgroundActivity: nil).get()
    XCTAssertEqual(account.store, 9)
    let seen = FakeAccountRust.snapshot()
    // 拼装交给 Rust：宿主只给平台、版本、设备、系统和版本 ID，注册时带的是 Rust 拼出的那一串。
    XCTAssertEqual(seen.agents.count, 1)
    XCTAssertEqual(seen.agents.first?["platform"] as? String, "macos")
    XCTAssertEqual(seen.agents.first?["device"] as? String, "Mac14,2")
    XCTAssertEqual(seen.agents.first?["system"] as? String, "macOS 26.0")
    XCTAssertEqual(seen.agents.first?["edition"] as? String, "full")
    XCTAssertEqual(seen.registrations, ["msime-macos/0.52.0 (Mac14,2; macOS 26.0; edition=full)"])
    // 设备和系统不知道时交 null，由 Rust 换成 unknown。
    _ = BackendRustAccount.clientAgentString(client: client, agent: .init(platform: "macos", version: "1", device: nil, system: nil, edition: "full"))
    XCTAssertTrue(FakeAccountRust.snapshot().agents.last?["device"] is NSNull)
  }

  func testRegistrationFailsWithoutAnAgentInsteadOfFallingBackToTheDefault() throws {
    FakeAccountRust.agentReply = #"{"ok":false,"error":"account_invalid"}"#
    let client = try XCTUnwrap(BackendRustClient(table: FakeAccountRust.table()))
    XCTAssertEqual(BackendRustAccount.register(client: client, agent: agent, layout: layout, backgroundActivity: nil).map(\.store),
                   .failure(BackendRustClient.Failure(code: "account_invalid",
                                                      response: Data(#"{"ok":false,"error":"account_invalid"}"#.utf8))))
    XCTAssertTrue(FakeAccountRust.snapshot().registrations.isEmpty)
  }

  func testSharedRegistrationHappensOncePerProcess() throws {
    let client = try XCTUnwrap(BackendRustClient(table: FakeAccountRust.table()))
    let first = try BackendRustAccount.registerShared(client: client, agent: agent, layout: layout, backgroundActivity: nil).get()
    let second = try BackendRustAccount.registerShared(client: client, agent: agent, layout: layout, backgroundActivity: nil).get()
    XCTAssertEqual(first.store, second.store)
    XCTAssertEqual(BackendRustAccount.shared?.store, first.store)
    XCTAssertEqual(FakeAccountRust.snapshot().registrations.count, 1)
  }

  func testRequestsCarryTheStoreAndNoDirectory() async throws {
    FakeAccountRust.replies = [
      "account.status": #"{"ok":true,"value":{"user":{"id":"synthetic-user","displayName":"合成用户","createdAt":"2026-10-18","email":"u@example.invalid"}}}"#,
      "cloud_clipboard.list": #"{"ok":true,"value":{"enabled":false,"items":[]}}"#,
      "cloud_dictionary.list": #"{"ok":true,"value":{"entries":[],"has_more":false,"offset":0}}"#,
    ]
    let account = BackendRustAccount(client: try XCTUnwrap(BackendRustClient(table: FakeAccountRust.table())), store: 9)
    let user = try await account.status()
    XCTAssertEqual(user, BackendAccountClient.User(id: "synthetic-user", display_name: "合成用户", created_at: "2026-10-18",
                                                   email: "u@example.invalid", avatar_url: nil))
    _ = try await account.cloudClipboard(["operation": "list", "search": ""], deviceName: "合成 Mac")
    _ = try await account.cloudDictionary(["operation": "list", "kind": "pinyin", "offset": 0, "search": ""],
                                          snapshotDirectory: directory, options: ["user_data": "/synthetic/user"])
    let seen = FakeAccountRust.snapshot().requests
    XCTAssertEqual(seen.map(\.entry), ["account", "cloud_clipboard", "cloud_dictionary"])
    XCTAssertTrue(seen.allSatisfy { $0.body["session_store"] as? Int == 9 && $0.body["directory"] == nil })
    XCTAssertEqual(seen[1].body["device_name"] as? String, "合成 Mac")
    XCTAssertEqual(seen[2].body["snapshot_directory"] as? String, directory.path)
    XCTAssertEqual((seen[2].body["options"] as? [String: String])?["user_data"], "/synthetic/user")
  }

  /// 自建中继的两个入口：`cloud_clipboard_source` 传偏好目录、答走哪条路；`relay_cloud_clipboard` 带偏好目录、密钥串目录、设备名和动作，不带会话存储。
  func testRelayRequestsCarryThePreferencesAndKeystoreDirectories() async throws {
    FakeAccountRust.replies = [
      "cloud_clipboard_source.": #"{"ok":true,"value":{"source":"relay","revision":7}}"#,
      "relay_cloud_clipboard.list": #"{"ok":true,"value":{"enabled":true,"items":[]}}"#,
    ]
    let account = BackendRustAccount(client: try XCTUnwrap(BackendRustClient(table: FakeAccountRust.table())), store: 9)
    let state = directory.appendingPathComponent("state", isDirectory: true)
    let source = try await account.cloudClipboardSource(preferencesDirectory: state)
    XCTAssertEqual(source, "relay")
    _ = try await account.relayCloudClipboard(["operation": "list", "search": ""], preferencesDirectory: state,
                                              keystoreDirectory: state, deviceName: "合成 Mac", expectedRevision: 7)
    let raw = FakeAccountRust.rawRequests
    XCTAssertEqual(raw.map(\.entry), ["cloud_clipboard_source", "relay_cloud_clipboard"])
    XCTAssertEqual(raw.first?.bytes, Data(state.path.utf8))
    let relay = FakeAccountRust.snapshot().requests.last
    XCTAssertEqual(relay?.body["preferences_directory"] as? String, state.path)
    XCTAssertEqual(relay?.body["directory"] as? String, state.path)
    XCTAssertEqual(relay?.body["device_name"] as? String, "合成 Mac")
    XCTAssertNil(relay?.body["session_store"])
    XCTAssertEqual(relay?.body["expected_revision"] as? Int, 7)
  }

  func testSignedOutIsNilButUnreadableStorageIsAFailure() async throws {
    let account = BackendRustAccount(client: try XCTUnwrap(BackendRustClient(table: FakeAccountRust.table())), store: 9)
    FakeAccountRust.replies = ["account.status": #"{"ok":true,"value":{"user":null}}"#]
    let signedOut = try await account.status()
    XCTAssertNil(signedOut)
    FakeAccountRust.replies = ["account.status": #"{"ok":false,"error":"account_storage"}"#]
    do { _ = try await account.status(); XCTFail("unreadable storage read as signed out") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure.code, "account_storage") }
    FakeAccountRust.replies = ["account.status": #"{"ok":true,"value":{"user":{"id":""}}}"#]
    do { _ = try await account.status(); XCTFail("malformed user accepted") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure, .invalidResponse) }
  }

  /// 账号错误码的说法由共享用例逐条核对（`BackendAccountErrorCaseTests`）；这里是用例之外的码：云剪贴板驱动的码、审核码与旧的 `BackendAccountClient.Failure` 共用一句，Swift 自己造的失败也有一句。
  func testFailuresReadInChinese() {
    func message(_ code: String) -> String? { BackendRustClient.Failure(code: code).errorDescription }
    XCTAssertEqual(message("clipboard_device_pending"), BackendAccountClient.Failure.clipboardMessages["clipboard_device_pending"])
    XCTAssertEqual(message("account_blocked_content"), BackendAccountClient.Failure.moderationMessages["blocked_content"])
    XCTAssertEqual(message("rust_client_unavailable"), "账号服务暂不可用，请稍后再试。")
    XCTAssertEqual(message("clipboard_source_changed"), "云剪贴板同步来源已更改，请关闭后重新打开。")
  }
  func testRateLimitSaysHowLongToWaitWhenKnown() {
    func message(_ seconds: Int?) -> String? {
      BackendRustClient.Failure(code: "account_rate_limited", retryAfterSeconds: seconds).errorDescription
    }
    XCTAssertEqual(message(nil), "操作过于频繁，请稍后再试。")
    XCTAssertEqual(message(42), "操作过于频繁，请约 42 秒后再试。")
    XCTAssertEqual(message(61), "操作过于频繁，请约 2 分钟后再试。")
    XCTAssertEqual(message(3600), "操作过于频繁，请约 1 小时后再试。")
    XCTAssertEqual(message(7201), "操作过于频繁，请约 3 小时后再试。")
    XCTAssertEqual(BackendRustClient.Failure(code: "community_rate_limited", retryAfterSeconds: 0).errorDescription, "操作过于频繁，请约 1 秒后再试。")
  }

  func testLoginShapesDecode() throws {
    let challenge = try BackendRustAccount.challenge(Data(#"{"challengeId":"c-1","expiresIn":300,"nonce":"n"}"#.utf8))
    XCTAssertEqual(challenge, .init(challenge_id: "c-1", expires_in: 300, nonce: "n"))
    XCTAssertNil(try BackendRustAccount.challenge(Data(#"{"challengeId":"c-1","expiresIn":300}"#.utf8)).nonce)
    for invalid in [#"{"challengeId":"","expiresIn":300}"#, #"{"challengeId":"c","expiresIn":0}"#,
                    #"{"challengeId":"c","expiresIn":true}"#, #"{"challengeId":"c","expiresIn":3,"nonce":1}"#] {
      XCTAssertThrowsError(try BackendRustAccount.challenge(Data(invalid.utf8)), invalid)
    }
    let profile = try BackendRustAccount.profile(Data(#"{"user":{"id":"u","displayName":"名","createdAt":"t","email":"u@example.invalid"},"providers":["google","email"],"providerNames":{"google":"合成谷歌方式","email":"合成邮箱方式"}}"#.utf8))
    XCTAssertEqual(profile.user.email, "u@example.invalid")
    XCTAssertEqual(profile.providers, ["google", "email"])
    XCTAssertEqual(profile.providerNames, ["google": "合成谷歌方式", "email": "合成邮箱方式"])
    XCTAssertThrowsError(try BackendRustAccount.profile(Data(#"{"user":{"id":"u","displayName":"名","createdAt":"t"}}"#.utf8)))
    XCTAssertEqual(try BackendRustAccount.signedInUser(Data(#"{"user":{"id":"u","displayName":"名","createdAt":"t"}}"#.utf8)).id, "u")
  }

  func testProvidersKeepOnlyBooleans() async throws {
    FakeAccountRust.replies = ["account.providers": #"{"ok":true,"value":{"email":true,"phone":false,"apple":1,"google":true}}"#]
    let account = BackendRustAccount(client: try XCTUnwrap(BackendRustClient(table: FakeAccountRust.table())), store: 9)
    let providers = try await account.providers()
    XCTAssertEqual(providers, ["email": true, "phone": false, "google": true])
  }

  func testProfileRejectsMalformedSharedProviderNames() throws {
    let base: [String: Any] = [
      "user": ["id": "synthetic-user", "displayName": "合成账号", "createdAt": "synthetic-date"],
      "providers": ["email"]
    ]
    XCTAssertThrowsError(try BackendRustAccount.profile(JSONSerialization.data(withJSONObject: base)))
    for invalid: Any in [NSNull(), [String](), ["email": 42], false] {
      var response = base
      response["providerNames"] = invalid
      XCTAssertThrowsError(try BackendRustAccount.profile(JSONSerialization.data(withJSONObject: response)))
    }
  }
}

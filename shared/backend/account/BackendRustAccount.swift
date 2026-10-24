import Foundation
import CMSIMEClient

/// 本进程注册到 Rust 的会话存储，以及经它发出的账号与云端请求（#8201 迁移计划 4b）。请求都带着这个存储的 id（`session_store`），令牌只在 Rust 里：迁到这里的页面不再自己读、刷新这份会话，失败一律是 `BackendRustClient.Failure`，错误码与 `msime_client.h` 相同。
///
/// 同一份存储在一个进程里只能注册一次（每次注册各有一套进程内的锁和会话缓存），所以生产代码都经 `shared` 拿到同一份；测试用替身函数表自己构造。
struct BackendRustAccount: Sendable {
  let client: BackendRustClient
  let store: UInt64

  private final class Registered: @unchecked Sendable {
    let lock = NSLock()
    var account: BackendRustAccount?
  }
  private static let registered = Registered()

  /// 本进程注册好的那一份；宿主还没注册或注册失败时为 nil。
  static var shared: BackendRustAccount? {
    registered.lock.lock(); defer { registered.lock.unlock() }
    return registered.account
  }

  /// `shared`，没有时抛 `rust_client_unavailable`。
  static func current() throws -> BackendRustAccount {
    guard let account = shared else { throw BackendRustClient.Failure.unavailable }
    return account
  }

  /// 宿主自报的设备信息，拼成 `msime_client_client_agent` 的请求。`device` 和 `system` 可以为 nil，Rust 换成 unknown。
  struct Agent: Sendable, Equatable {
    var platform: String
    var version: String
    var device: String?
    var system: String?
    var edition: String

    var request: [String: Any] {
      ["platform": platform, "version": version, "device": device ?? NSNull(), "system": system ?? NSNull(), "edition": edition]
    }
  }

  /// 用 `layout` 注册本进程的会话存储，成功后 `shared` 就是它。已经注册过时直接返回那一份，不注册第二次。不联网，可以在主线程上调用。
  ///
  /// `backgroundActivity` 故意没有默认值：iOS App 必须交 `beginBackgroundTask`，否则挂起时还持有 App Group 文件锁会被系统杀掉（0xdead10cc）；不会被这样挂起的宿主（macOS）明确交 nil。
  @discardableResult
  static func registerShared(client: BackendRustClient, agent: Agent, layout: BackendRustStoreLayout = .standard,
                             backgroundActivity: BackendRustSessionStore.BackgroundActivity?)
    -> Result<BackendRustAccount, BackendRustClient.Failure> {
    registered.lock.lock(); defer { registered.lock.unlock() }
    if let account = registered.account { return .success(account) }
    return register(client: client, agent: agent, layout: layout, backgroundActivity: backgroundActivity).map { account in
      registered.account = account
      return account
    }
  }

  /// 拼出 client_agent 再注册一份存储，不记进 `shared`。client_agent 由 Rust 拼（服务端「我的设备」据此显示平台和设备），拼不出时整次注册失败：不带它注册，之后的登录都会以默认的 MSIME/Android 发出。
  static func register(client: BackendRustClient, agent: Agent, layout: BackendRustStoreLayout,
                       backgroundActivity: BackendRustSessionStore.BackgroundActivity?)
    -> Result<BackendRustAccount, BackendRustClient.Failure> {
    clientAgentString(client: client, agent: agent).flatMap { clientAgent in
      BackendRustSessionStore(layout: layout, backgroundActivity: backgroundActivity)
        .register(with: client, clientAgent: clientAgent)
        .map { BackendRustAccount(client: client, store: $0) }
    }
  }

  /// `msime_client_client_agent` 拼出的 User-Agent。
  static func clientAgentString(client: BackendRustClient, agent: Agent) -> Result<String, BackendRustClient.Failure> {
    let request: Data
    do { request = try BackendRustClient.request(agent.request) } catch { return .failure(.invalidRequest) }
    return client.callBlocking(.clientAgent, request).flatMap { value in
      guard let text = (try? JSONSerialization.jsonObject(with: value, options: [.fragmentsAllowed])) as? String,
            !text.isEmpty else { return .failure(.invalidResponse) }
      return .success(text)
    }
  }

  // MARK: - 请求

  /// `msime_client_account` 的一个 action，成功时是 `value` 的 JSON。
  func account(_ action: [String: Any]) async throws -> Data {
    try await client.call(.account, ["session_store": store, "action": action])
  }

  /// `msime_client_account_cloud_clipboard` 的一个 action。`deviceName` 只在本机还没登记端到端加密时发给服务端，给其他设备的设备列表看。
  func cloudClipboard(_ action: [String: Any], deviceName: String?, preferencesDirectory: URL? = nil) async throws -> Data {
    var request: [String: Any] = ["session_store": store, "action": action]
    if let preferencesDirectory { request["preferences_directory"] = preferencesDirectory.path }
    if let deviceName { request["device_name"] = deviceName }
    return try await client.call(.accountCloudClipboard, request)
  }

  /// `msime_client_cloud_clipboard_source`：偏好里配置了自建中继时是 `relay`，否则是 `account`。`preferencesDirectory` 是共享偏好目录（macOS 是 `account-session.json` 所在的那一个）。不联网，偏好读不了或中继配置不合规时抛错。
  struct ClipboardSource: Equatable, Sendable {
    let source: String
    let revision: UInt64
  }

  func cloudClipboardSelection(preferencesDirectory: URL) async throws -> ClipboardSource {
    let value = try await client.call(.cloudClipboardSource, Data(preferencesDirectory.path.utf8))
    guard let source = (try? JSONSerialization.jsonObject(with: value)) as? [String: Any],
          let text = source["source"] as? String, ["account", "relay"].contains(text),
          let revision = source["revision"] as? NSNumber,
          CFGetTypeID(revision) != CFBooleanGetTypeID(),
          !["f", "d"].contains(String(cString: revision.objCType)),
          let number = UInt64(revision.stringValue) else {
      throw BackendRustClient.Failure.invalidResponse
    }
    return ClipboardSource(source: text, revision: number)
  }

  func cloudClipboardSource(preferencesDirectory: URL) async throws -> String {
    try await cloudClipboardSelection(preferencesDirectory: preferencesDirectory).source
  }

  /// `msime_client_relay_cloud_clipboard` 的一个 action：动作经用户自建的中继完成，中继地址和令牌从 `preferencesDirectory` 的偏好里读。`keystoreDirectory` 放中继的密钥串 `cloud-clipboard-relay-keystore`，同一台设备上的各个进程要传同一个，否则会登记成几台设备。
  func relayCloudClipboard(_ action: [String: Any], preferencesDirectory: URL, keystoreDirectory: URL,
                           deviceName: String?, expectedRevision: UInt64? = nil) async throws -> Data {
    var request: [String: Any] = ["preferences_directory": preferencesDirectory.path,
                                  "directory": keystoreDirectory.path, "action": action]
    if let expectedRevision { request["expected_revision"] = expectedRevision }
    if let deviceName { request["device_name"] = deviceName }
    return try await client.call(.relayCloudClipboard, request)
  }

  /// `msime_client_community` 的一个 action（键盘皮肤、词包与回复模板、举报）。键盘皮肤的下载和举报没有登录时 Rust 用本机匿名账号。
  func community(_ action: [String: Any]) async throws -> Data {
    try await client.call(.community, ["session_store": store, "action": action])
  }

  /// `msime_client_cloud_dictionary` 的一个 action。`snapshotDirectory` 是本进程私有的中转目录（`snapshot_*` 和 `export_file` 必需，两个同时运行的进程不能共用一个），`options` 是本机的 HostOptions 文档（`snapshot_preview`、`snapshot_enqueue`、`snapshot_status`、`snapshot_cancel` 必需）。
  func cloudDictionary(_ action: [String: Any], snapshotDirectory: URL? = nil, options: [String: Any]? = nil) async throws -> Data {
    var request: [String: Any] = ["session_store": store, "action": action]
    if let snapshotDirectory { request["snapshot_directory"] = snapshotDirectory.path }
    if let options { request["options"] = options }
    return try await client.call(.cloudDictionary, request)
  }

  /// 登录账号（`status`），没有登录时为 nil。只读存储、不联网；存储读不了时是 `account_storage`，不当作没有登录。
  func status() async throws -> BackendAccountClient.User? {
    let object = try Self.object(try await account(["operation": "status"]))
    return try Self.optionalUser(object["user"])
  }

  /// 服务端开着的登录方式（`providers`），值不是布尔的项丢掉。
  func providers() async throws -> [String: Bool] {
    try Self.object(try await account(["operation": "providers"])).compactMapValues { value in
      guard let flag = value as? NSNumber, CFGetTypeID(flag) == CFBooleanGetTypeID() else { return nil }
      return flag.boolValue
    }
  }

  // MARK: - 解码

  /// 登录用的挑战：`request_code` 的回答只有前两项，`apple_challenge` 和 `google_challenge` 还带 nonce。属性名沿用 Swift 页面一直用的服务端写法。
  struct Challenge: Sendable, Equatable {
    let challenge_id: String
    let expires_in: Int
    var nonce: String? = nil
  }

  /// `{challengeId, expiresIn, nonce?}`；缺字段或类型不对时是 `rust_client_invalid_response`。
  static func challenge(_ data: Data) throws -> Challenge {
    let object = try object(data)
    guard let id = object["challengeId"] as? String, !id.isEmpty,
          let expires = object["expiresIn"] as? NSNumber, CFGetTypeID(expires) != CFBooleanGetTypeID(), expires.intValue > 0
    else { throw BackendRustClient.Failure.invalidResponse }
    let nonce = object["nonce"]
    if nonce != nil, !(nonce is String) { throw BackendRustClient.Failure.invalidResponse }
    return Challenge(challenge_id: id, expires_in: expires.intValue, nonce: nonce as? String)
  }

  /// 资料页：`providers` 是已绑定的登录方式，`providerNames` 是 Rust 给出的显示名称表。
  struct Profile: Sendable {
    let user: BackendAccountClient.User
    let providers: [String]
    let providerNames: [String: String]
  }

  static func profile(_ data: Data) throws -> Profile {
    let object = try object(data)
    guard let providers = object["providers"] as? [String],
          let names = object["providerNames"] as? [String: String],
          names.values.allSatisfy({ !$0.isEmpty }),
          providers.allSatisfy({ names[$0] != nil })
    else { throw BackendRustClient.Failure.invalidResponse }
    return Profile(user: try user(object["user"]), providers: providers, providerNames: names)
  }

  /// 登录类操作的回答 `{user}` 里的账号。
  static func signedInUser(_ data: Data) throws -> BackendAccountClient.User {
    try user(try object(data)["user"])
  }

  /// 回答里的 JSON 对象；不是对象时是 `rust_client_invalid_response`。
  static func object(_ data: Data) throws -> [String: Any] {
    guard let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
      throw BackendRustClient.Failure.invalidResponse
    }
    return object
  }

  /// 按 `Decodable` 读回答，读不懂时是 `rust_client_invalid_response`。
  static func decode<T: Decodable>(_ type: T.Type, _ data: Data) throws -> T {
    do { return try JSONDecoder().decode(type, from: data) } catch { throw BackendRustClient.Failure.invalidResponse }
  }

  /// 账号页形状的 user（`{id,displayName,createdAt,email?,avatarUrl?}`）换成 Swift 页面一直用的 `BackendAccountClient.User`。
  static func user(_ value: Any?) throws -> BackendAccountClient.User {
    guard let object = value as? [String: Any], let id = object["id"] as? String, !id.isEmpty,
          let name = object["displayName"] as? String, let created = object["createdAt"] as? String else {
      throw BackendRustClient.Failure.invalidResponse
    }
    return BackendAccountClient.User(id: id, display_name: name, created_at: created,
                                     email: object["email"] as? String, avatar_url: object["avatarUrl"] as? String)
  }

  /// 同上，`null` 或没有这个字段时为 nil。
  static func optionalUser(_ value: Any?) throws -> BackendAccountClient.User? {
    if value == nil || value is NSNull { return nil }
    return try user(value)
  }
}

extension BackendRustClient.Failure: LocalizedError {
  /// 页面显示的中文提示。账号码的措辞与共享账号页（`packages/ui/src/account/account-errors.ts`）和 Windows 原生设置相同；云剪贴板驱动和审核的码沿用 `BackendAccountClient.Failure` 的那几句。
  var errorDescription: String? {
    if let message = BackendAccountClient.Failure.clipboardMessages[code] { return message }
    switch code {
    case "account_blocked_content", "community_blocked_content": return BackendAccountClient.Failure.moderationMessages["blocked_content"]
    case "account_screening_unavailable", "community_screening_unavailable":
      return BackendAccountClient.Failure.moderationMessages["screening_unavailable"]
    case "account_banned", "community_account_banned": return BackendAccountClient.Failure.moderationMessages["account_banned"]
    case "account_invalid", "invalid_cloud_clipboard", "invalid_cloud_dictionary", "community_invalid", "snapshot_invalid":
      return "填写的内容无效，请检查后重试。"
    case "account_unauthorized", "community_unauthorized": return "登录已失效，请重新登录。"
    case "account_conflict", "snapshot_conflict", "community_conflict": return "云端内容已被其他设备更新，请刷新后重新确认。"
    case "account_rate_limited", "community_rate_limited":
      return retryAfterSeconds.map { "操作过于频繁，请约 \(Self.wait($0))后再试。" } ?? "操作过于频繁，请稍后再试。"
    case "account_storage": return "无法安全读取登录状态，请检查设备安全设置。"
    case "clipboard_source_changed": return "云剪贴板同步来源已更改，请关闭后重新打开。"
    case "account_cancelled", "community_cancelled": return "操作已取消，请重试。"
    case "account_network": return "网络连接失败，请检查网络后重试。"
    case "account_recent_login_required": return "需要重新登录后再注销：请先退出登录，重新登录后再试。"
    case "account_too_large": return "内容超出大小限制，请缩减后重试。"
    case "snapshot_busy": return "词库正在处理另一项操作，请稍后重试。"
    case "snapshot_unavailable": return "词库快照暂时不可用，请稍后重试。"
    default: return "账号服务暂不可用，请稍后再试。"
    }
  }

  /// 服务端 Retry-After 的秒数说成「N 秒」「N 分钟」「N 小时」，向上取整。
  static func wait(_ seconds: Int) -> String {
    if seconds < 60 { return "\(max(1, seconds)) 秒" }
    if seconds < 3600 { return "\((seconds + 59) / 60) 分钟" }
    return "\((seconds + 3599) / 3600) 小时"
  }
}

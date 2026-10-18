// Ported from MSIME-Apple develop at 2b0250f4dd7012520392b310dfcc0288c3208a75.
// Uses synthetic fixtures and in-memory credentials only; no live service access.
import AppKit
import Foundation

private final class MemoryCredentials: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var saved: BackendSavedSession?
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return saved }
  func save(_ value: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; saved = value }
  func clear() throws { lock.lock(); defer { lock.unlock() }; saved = nil }
}
private final class AccountFixture: URLProtocol, @unchecked Sendable {
  static var failLogout = false
  static var allowDelete = false
  static var rejectFirstRename = false
  static var renameRequests = 0
  static var rejectFirstDelete = false
  static var deleteRequests = 0
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let body: String
    var status = 200
    switch (request.httpMethod!, request.url!.path) {
    case (_, "/v1/auth/providers"): body = #"{"providers":{"email":true,"phone":false,"apple":false}}"#
    case (_, "/v1/auth/challenges"): body = #"{"challenge_id":"synthetic","expires_in":300}"#
    case (_, "/v1/auth/login"):
      let token = String(repeating: "a", count: 64), refresh = String(repeating: "b", count: 64)
      body = "{\"access_token\":\"\(token)\",\"refresh_token\":\"\(refresh)\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"synthetic-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08\"}}"
    case (_, "/v1/auth/refresh"):
      let token = String(repeating: "c", count: 64), refresh = String(repeating: "d", count: 64)
      body = "{\"access_token\":\"\(token)\",\"refresh_token\":\"\(refresh)\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"synthetic-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08\"}}"
    case (_, "/v1/auth/logout"): body = ""; status = Self.failLogout ? 503 : 204
    case ("PATCH", "/v1/users/me"):
      Self.renameRequests += 1
      if Self.rejectFirstRename { Self.rejectFirstRename = false; body = #"{"error":{"code":"invalid_credentials"}}"#; status = 401 }
      else { body = ""; status = 204 }
    case ("GET", "/v1/users/me"): body = #"{"user":{"id":"synthetic-user","display_name":"新昵称","created_at":"2026-09-08"},"identities":[]}"#
    case ("DELETE", "/v1/users/me"):
      Self.deleteRequests += 1
      if Self.rejectFirstDelete { Self.rejectFirstDelete = false; body = #"{"error":{"code":"invalid_credentials"}}"#; status = 401 }
      else { body = Self.allowDelete ? "" : #"{"error":{"code":"recent_login_required"}}"#; status = Self.allowDelete ? 204 : 403 }
    default: body = "{}"; status = 404
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type":"application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8)); client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
/// 替身 Rust 对账号 action 的回答（#8201 迁移计划 4b）。字段改了以后的下一次调用就按新的回答。
private final class AccountScript: @unchecked Sendable {
  var status: Any = NSNull()
  var statusFailure: String?
  var deleteFailure: String?
  var logoutFailure: String?
  func respond(_ call: FakeRust.Call) -> String {
    switch call.operation {
    case "status": return statusFailure.map(FakeRust.failure) ?? FakeRust.ok(["user": status])
    case "providers": return FakeRust.ok(["email": true, "phone": false, "apple": false, "google": false, "apple_web": false])
    case "request_code": return FakeRust.ok(["challengeId": "synthetic", "expiresIn": 300])
    case "login": return FakeRust.ok(["user": FakeRust.user("synthetic-user")])
    case "rename": return FakeRust.ok(["user": FakeRust.user("synthetic-user", call.action["display_name"] as? String ?? ""), "providers": ["email"]])
    case "account_delete": return deleteFailure.map(FakeRust.failure) ?? FakeRust.ok(["voice_samples": ["ok": true]])
    case "logout": return logoutFailure.map(FakeRust.failure) ?? FakeRust.ok(NSNull())
    default: return FakeRust.failure("account_invalid")
    }
  }
}
/// 替身 Rust 的设置同步：服务端的字段表、云端文档和按修订号比较并交换的上传。`omitted` 是云端文档里还没有的键，`themeSchema` 和 `darkSkinSchema` 为假时字段表缺主题组或深色槽位。
private final class CloudPreferences: @unchecked Sendable {
  var revision = 1
  var settings: [String: Any] = ["platform.macos.candidate_font_size": 18, "platform.macos.candidate_learning": true, "platform.ios.nine_key": true,
                                 "platform.macos.quanpin_helpcode_schema": 6, "platform.macos.shuangpin_helpcode_schema": 9]
  var omitted: String?
  var themeSchema = true
  var darkSkinSchema = true
  /// 云端现在的文档。
  var document: BackendAccountClient.Preferences {
    try! JSONDecoder().decode(BackendAccountClient.Preferences.self, from: JSONSerialization.data(withJSONObject: ["revision": revision, "settings": settings]))
  }
  func respond(_ call: FakeRust.Call) -> String {
    switch call.operation {
    case "status": return FakeRust.ok(["user": FakeRust.user("synthetic-user")])
    case "preferences_schema":
      var fields: [String: Any] = ["platform.macos.global_theme": ["type": "string"], "platform.macos.candidate_font_size": ["type": "integer"],
                                   "platform.macos.candidate_learning": ["type": "boolean"], "platform.macos.shuangpin_preedit_uses_raw": ["type": "boolean"],
                                   "platform.ios.nine_key": ["type": "boolean"]]
      // 服务端只登记了主题组的一部分，就像早于自定义主题的服务端那样。
      if themeSchema {
        fields["platform.macos.custom_theme_base"] = ["type": "string"]; fields["platform.macos.custom_candidate_skin"] = ["type": "string"]
        if darkSkinSchema { fields["platform.macos.custom_candidate_skin_dark"] = ["type": "string"] }
      }
      return FakeRust.ok(["fields": fields, "maximumBytes": 1048576, "updateMode": "replace", "revisionRequired": true])
    case "preferences_load":
      return FakeRust.ok(["revision": revision, "settings": settings.filter { $0.key != omitted }])
    case "preferences_put":
      guard call.action["user_id"] as? String == "synthetic-user" else { return FakeRust.failure("account_cancelled") }
      guard call.action["revision"] as? Int == revision, let document = call.action["document"] as? [String: Any] else { return FakeRust.failure("account_conflict") }
      revision += 1; settings = document
      return FakeRust.ok(["revision": revision, "settings": settings])
    default: return FakeRust.failure("account_invalid")
    }
  }
}
@main struct AccountTests {
  struct Failure: Error {}
  @MainActor static func require(_ value: Bool) throws { if !value { throw Failure() } }
  @MainActor static func finished(_ model: MacAccountModel) async throws {
    let deadline = Date().addingTimeInterval(5)
    while model.busy && Date() < deadline { try await Task.sleep(nanoseconds: 5_000_000) }
    try require(!model.busy)
  }
  @MainActor static func finished(_ model: MacClipboardModel) async throws {
    let deadline = Date().addingTimeInterval(5)
    while model.busy && Date() < deadline { try await Task.sleep(nanoseconds: 5_000_000) }
    try require(!model.busy)
  }
  @MainActor static func finished(_ model: MacDictionaryModel) async throws {
    let deadline = Date().addingTimeInterval(5)
    while model.busy && Date() < deadline { try await Task.sleep(nanoseconds: 5_000_000) }
    try require(!model.busy)
  }
  @MainActor static func finished(_ model: MacSnapshotModel) async throws {
    let deadline = Date().addingTimeInterval(5)
    while model.busy && Date() < deadline { try await Task.sleep(nanoseconds: 5_000_000) }
    try require(!model.busy)
  }
  @MainActor static func finished(_ model: MacSettingsModel) async throws {
    let deadline = Date().addingTimeInterval(5)
    while model.busy && Date() < deadline { try await Task.sleep(nanoseconds: 5_000_000) }
    try require(!model.busy)
  }
  /// 「本机账号」卡片留在 Swift（负责人决定）：匿名账号的改名、注销走 Swift 的匿名会话，不经 Rust；登录账号仍以 Rust 的 `status` 为准；验证码登录成功后丢掉匿名会话。
  @MainActor static func anonymousAccountFallback() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AccountFixture.self]
    let client = BackendAccountClient(configuration: configuration)
    let anonymousStorage = MemoryCredentials()
    let anonymousSession = BackendAccountSession(api: client, storage: anonymousStorage)
    try await anonymousSession.signIn(challenge: "synthetic", credential: "123456")
    let script = AccountScript()
    FakeRust.answer(script.respond)
    var discarded = 0
    let model = MacAccountModel(rust: FakeRust.account, client: client, anonymousAccount: anonymousSession,
                                closeAccountWindows: {}, discardAnonymous: { discarded += 1 })
    model.load(); try await finished(model)
    try require(model.user?.id == "synthetic-user" && model.anonymous)
    model.name = "匿名昵称"; model.rename(); try await finished(model)
    try require(model.user?.display_name == "新昵称" && anonymousStorage.load()?.tokens.user.display_name == "新昵称")
    model.name = "有效\n中文"; model.rename(); try await finished(model)
    let storedName = try anonymousStorage.load()?.tokens.user.display_name
    try require(model.message != nil && storedName == "新昵称" && FakeRust.calls("rename").isEmpty)
    script.status = FakeRust.user("synthetic-user")
    let priorityModel = MacAccountModel(rust: FakeRust.account, client: client, anonymousAccount: anonymousSession)
    priorityModel.load(); try await finished(priorityModel)
    try require(priorityModel.user?.id == "synthetic-user" && !priorityModel.anonymous)
    script.status = NSNull()
    AccountFixture.allowDelete = true
    defer { AccountFixture.allowDelete = false }
    AccountFixture.deleteRequests = 0
    AccountFixture.rejectFirstDelete = true
    model.logout(delete: true); try await finished(model)
    try require(AccountFixture.deleteRequests == 2 && model.user == nil && !model.anonymous
                && discarded == 1 && anonymousStorage.load() == nil && FakeRust.calls("account_delete").isEmpty)
    // 本机账号下用验证码登录：登录经 Rust，成功后丢掉匿名会话和身份。
    try await anonymousSession.signIn(challenge: "synthetic", credential: "123456")
    let upgrading = MacAccountModel(rust: FakeRust.account, client: client, anonymousAccount: anonymousSession,
                                    closeAccountWindows: {}, discardAnonymous: { discarded += 1 })
    upgrading.load(); try await finished(upgrading)
    try require(upgrading.anonymous)
    upgrading.target = "synthetic@example.invalid"; upgrading.requestCode(); try await finished(upgrading)
    upgrading.code = "123456"; upgrading.codeLogin(); try await finished(upgrading)
    try require(upgrading.user?.id == "synthetic-user" && !upgrading.anonymous && discarded == 2
                && anonymousStorage.load() == nil && FakeRust.calls("login").count == 1)
  }
  /// 登录账号经 Rust（#8201 迁移计划 4b）：每个操作发出的 action、带的存储 id，以及收到回答后页面的状态。
  @MainActor static func rustAccountModel() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AccountFixture.self]
    let client = BackendAccountClient(configuration: configuration)
    let script = AccountScript()
    FakeRust.answer(script.respond)
    let rust = FakeRust.account
    var clipboardAccount: String?
    var signInRequests = 0
    await BackendClipboardEntry.open(status: { try await rust.status() }, present: { clipboardAccount = $0 }, signIn: { signInRequests += 1 })
    try require(clipboardAccount == nil && signInRequests == 1)
    var windowClosures = 0
    // 默认的匿名会话在真实的 App Group 容器里，未签名的测试程序可能建不出它，也绝不能读或丢掉它，所以这个模型用一份空的内存会话。
    let model = MacAccountModel(rust: rust, client: client,
                                anonymousAccount: BackendAccountSession(api: client, storage: MemoryCredentials()),
                                closeAccountWindows: { windowClosures += 1 }, discardAnonymous: {})
    model.load(); try await finished(model)
    try require(model.providers["email"] == true && model.providers["phone"] == false && model.user == nil && !model.anonymous)
    // 没有开放的登录方式不发请求。
    model.channel = "phone"; model.target = "+10000000000"
    model.requestCode(); try await finished(model)
    try require(model.challenge == nil && model.message == "账号服务暂不可用，请稍后再试。" && FakeRust.calls("request_code").isEmpty)
    model.channel = "email"; model.target = " synthetic@example.invalid "
    model.requestCode(); try await finished(model)
    let requested = FakeRust.calls("request_code").last?.action
    try require(model.challenge == .init(id: "synthetic", expiresIn: 300) && model.resendAt > Date()
                && requested?["provider"] as? String == "email" && requested?["target"] as? String == "synthetic@example.invalid")
    model.code = "123456"; model.codeLogin(); try await finished(model)
    let login = FakeRust.calls("login").last?.action
    try require(model.user?.id == "synthetic-user" && model.code.isEmpty && model.target.isEmpty
                && login?["challenge_id"] as? String == "synthetic" && login?["code"] as? String == "123456")
    // 每个请求都带注册给出的存储 id，不带目录：会话只在注入的存储里。
    try require(FakeRust.calls.allSatisfy { $0.entry == "account" && $0.request["session_store"] as? Int == 7 && $0.request["directory"] == nil })
    script.status = FakeRust.user("synthetic-user")
    await BackendClipboardEntry.open(status: { try await rust.status() }, present: { clipboardAccount = $0 }, signIn: { signInRequests += 1 })
    try require(clipboardAccount == "synthetic-user" && signInRequests == 1)
    model.name = "新昵称"; model.rename(); try await finished(model)
    try require(model.user?.display_name == "新昵称" && FakeRust.calls("rename").last?.action["display_name"] as? String == "新昵称")
    // 注销需要最近登录：本机登录留着、提示先退出再重新登录，私有窗口不关。
    script.deleteFailure = "account_recent_login_required"
    model.logout(delete: true); try await finished(model)
    try require(model.user != nil && model.message == "需要重新登录后再注销：请先退出登录，重新登录后再试。" && windowClosures == 0)
    model.logout(all: true); try await finished(model)
    try require(model.user == nil && windowClosures == 1 && FakeRust.calls("logout").last?.action["all"] as? Bool == true)
    // 断网时 Rust 已经先清掉了本机会话：页面显示已退出和失败的原因。
    model.load(); try await finished(model)
    try require(model.user?.id == "synthetic-user")
    script.status = NSNull(); script.logoutFailure = "account_network"
    model.logout(); try await finished(model)
    try require(windowClosures == 2 && model.user == nil && model.message == "网络连接失败，请检查网络后重试。")
    // 读不了登录状态（钥匙串锁着、状态文件权限不对）是失败：显示原因，不当作没有登录，也不退回本机账号。
    let anonymousSession = BackendAccountSession(api: client, storage: MemoryCredentials())
    try await anonymousSession.signIn(challenge: "synthetic", credential: "123456")
    script.statusFailure = "account_storage"
    let locked = MacAccountModel(rust: rust, client: client, anonymousAccount: anonymousSession,
                                 closeAccountWindows: {}, discardAnonymous: {})
    locked.load(); try await finished(locked)
    try require(locked.user == nil && !locked.anonymous && locked.message == "无法安全读取登录状态，请检查设备安全设置。")
    // 宿主没有注册会话存储时每个操作都失败，不退回 Swift 的会话。
    let unregistered = MacAccountModel(rust: nil, client: client, anonymousAccount: anonymousSession,
                                       closeAccountWindows: {}, discardAnonymous: {})
    unregistered.load(); try await finished(unregistered)
    try require(unregistered.user == nil && unregistered.message == "账号服务暂不可用，请稍后再试。")
    model.code = "123456"; model.target = "synthetic@example.invalid"; model.close()
    try require(model.code.isEmpty && model.target.isEmpty && model.challenge == nil)
  }
  // Three pages asked for while the first is still on the network send two requests: the first runs to the end and the third replaces the second in the waiting slot, so a slow connection never has more than one page in flight.
  @MainActor static func candidateGlossSingleFlight() async throws {
    let original = BackendCandidateGloss.perform
    defer { BackendCandidateGloss.perform = original }
    var started: [UInt64] = []
    var release: [CheckedContinuation<Void, Never>] = []
    BackendCandidateGloss.perform = { request in
      started.append(request.generation)
      await withCheckedContinuation { release.append($0) }
    }
    func settle(_ count: Int) async throws {
      let deadline = Date().addingTimeInterval(5)
      while release.count < count && Date() < deadline { try await Task.sleep(nanoseconds: 5_000_000) }
      try await Task.sleep(nanoseconds: 50_000_000)
    }
    for generation: UInt64 in 1...3 { BackendCandidateGloss.fetch(words: ["测试"], primary: "en", secondary: "ja", generation: generation) }
    try await settle(1)
    try require(started == [1] && release.count == 1)
    release.removeFirst().resume()
    try await settle(1)
    try require(started == [1, 3] && release.count == 1)
    release.removeFirst().resume()
    try await Task.sleep(nanoseconds: 50_000_000)
    try require(started == [1, 3] && release.isEmpty)
    // The slot is free again once the last request finishes, and an empty page never takes it.
    BackendCandidateGloss.fetch(words: [], primary: "en", secondary: "", generation: 4)
    BackendCandidateGloss.fetch(words: ["再见"], primary: "en", secondary: "", generation: 5)
    try await settle(1)
    try require(started == [1, 3, 5])
    // The input method stopping its use of the account drops the waiting page, so nothing goes out after the user opted out; the request in flight still finishes.
    BackendCandidateGloss.fetch(words: ["谢谢"], primary: "en", secondary: "", generation: 6)
    msimeCancelAccountCandidateGlosses()
    release.removeFirst().resume()
    try await Task.sleep(nanoseconds: 50_000_000)
    try require(started == [1, 3, 5] && release.isEmpty)
  }
  // The input method files a reply under the target it names and drops one without it, and it caches an empty answer as "the account has nothing", so every word asked about must be in the table.
  @MainActor static func candidateGlossPayload() throws {
    let info = BackendCandidateGloss.payload(words: ["测试", "东京", "空白", "测试", "你好", "你好"],
                                             values: ["", "东京", "", "test", "hello", ""], code: "ja", generation: 7,
                                             marker: 42)
    try require(info["target"] as? String == "ja" && info["generation"] as? UInt64 == 7
                && info["marker"] as? UInt64 == 42)
    // An unchanged value is sent as "", an empty one stays, and a duplicate keeps its non-empty answer whichever comes first.
    try require(info["translations"] as? [String: String] == ["测试": "test", "东京": "", "空白": "", "你好": "hello"])
  }
  /// 云剪贴板窗口经 Rust（#8201 迁移计划 4c）：每个按钮发出的 action 和设备名，开着时才多读一次设备状态，操作自己带回设备状态时不再读，等待批准时留着草稿，打开窗口的账号不在了就丢掉结果。
  @MainActor static func rustClipboardModel() async throws {
    final class Cloud: @unchecked Sendable {
      var enabled = false
      var text: String?
      var pending = false
    }
    let cloud = Cloud()
    let code = "0421 9873 5530 1176"
    let itemID = String(repeating: "c", count: 64)
    func devices() -> [String: Any] {
      ["enabled": true, "status": cloud.pending ? "pending" : "ready", "device_id": String(repeating: "a", count: 32), "code": code, "devices": [
        ["device_id": String(repeating: "a", count: 32), "name": "合成 Mac", "created_at": "2026-10-11T08:00:00Z", "approved": !cloud.pending, "current": true, "code": code],
        ["device_id": String(repeating: "b", count: 32), "name": "合成 iPhone", "created_at": "2026-10-11T09:00:00Z", "approved": false, "current": false, "code": "1111 2222 3333 4444"],
      ]]
    }
    FakeRust.answer { call in
      if call.entry == "account" { return FakeRust.ok(["user": FakeRust.user("synthetic-user")]) }
      switch call.operation {
      case "list":
        if cloud.enabled && cloud.pending {
          return FakeRust.ok(["enabled": true, "retention_days": 0, "items": [], "e2e": ["status": "pending", "code": code, "pending_devices": 0, "undecryptable": 0]])
        }
        let items: [[String: Any]] = cloud.text.map { [["id": itemID, "text": $0, "updated_at": "2026-09-08", "pinned": false, "device": "合成 Mac"]] } ?? []
        return FakeRust.ok(["enabled": cloud.enabled, "retention_days": 0, "items": items,
                            "e2e": ["status": cloud.enabled ? "ready" : "disabled", "code": code, "pending_devices": 1, "undecryptable": 0]])
      case "set_enabled":
        cloud.enabled = call.action["enabled"] as? Bool ?? false
        if !cloud.enabled { cloud.text = nil }
        return FakeRust.ok(["enabled": cloud.enabled])
      case "add":
        if cloud.pending { return FakeRust.failure("clipboard_device_pending") }
        cloud.text = call.action["text"] as? String
        return FakeRust.ok(["id": itemID, "text": cloud.text ?? "", "updated_at": "2026-09-08"])
      case "delete", "clear": cloud.text = nil; return FakeRust.ok([String: Any]())
      case "devices", "approve", "reject", "remove", "reset": return FakeRust.ok(devices())
      default: return FakeRust.failure("invalid_cloud_clipboard")
      }
    }
    func sent() -> [String] { FakeRust.calls.filter { $0.entry == "cloud_clipboard" }.compactMap(\.operation) }
    let clipboard = MacClipboardModel(accountID: "synthetic-user", rust: FakeRust.account, deviceName: "合成 Mac")
    clipboard.refresh(); try await finished(clipboard)
    try require(clipboard.loaded && !clipboard.enabled && clipboard.items.isEmpty && clipboard.devices == nil && sent() == ["list"])
    clipboard.setEnabled(true); try await finished(clipboard)
    // 开着时列表之后再取一次设备状态，等待本机批准的另一台设备显示出来。
    try require(clipboard.enabled && !clipboard.isPending && clipboard.awaitingApproval.map(\.name) == ["合成 iPhone"]
                && Array(sent().suffix(3)) == ["set_enabled", "list", "devices"])
    // 每个请求都带注册给出的存储 id 和本机的设备名，不带目录。
    try require(FakeRust.calls.filter { $0.entry == "cloud_clipboard" }.allSatisfy {
      $0.request["session_store"] as? Int == 7 && $0.request["device_name"] as? String == "合成 Mac" && $0.request["directory"] == nil })
    clipboard.text = "合成剪贴板内容"; clipboard.upload(); try await finished(clipboard)
    try require(clipboard.items.first?.text == "合成剪贴板内容" && clipboard.text.isEmpty
                && FakeRust.calls("add").last?.action["text"] as? String == "合成剪贴板内容")
    // 批准本身带回设备状态，不再多取一次。
    let before = sent().count
    clipboard.change(.approve(deviceID: String(repeating: "b", count: 32))); try await finished(clipboard)
    try require(Array(sent().dropFirst(before)) == ["approve", "list"] && clipboard.message == nil
                && FakeRust.calls("approve").last?.action["device_id"] as? String == String(repeating: "b", count: 32))
    // 本机等待批准：看不到条目，显示核对码；上传失败时提示去批准，草稿留着。
    cloud.pending = true
    clipboard.refresh(); try await finished(clipboard)
    try require(clipboard.isPending && clipboard.items.isEmpty && clipboard.code == code && clipboard.awaitingApproval.isEmpty)
    clipboard.text = "等待批准时的草稿"; clipboard.upload(); try await finished(clipboard)
    try require(clipboard.text == "等待批准时的草稿" && clipboard.loaded
                && clipboard.message == BackendAccountClient.Failure.clipboardMessages["clipboard_device_pending"])
    cloud.pending = false
    clipboard.text = ""; clipboard.refresh(); try await finished(clipboard)
    try require(!clipboard.isPending && clipboard.items.first?.text == "合成剪贴板内容")
    clipboard.delete(id: clipboard.items.first!.id); try await finished(clipboard)
    try require(clipboard.items.isEmpty && FakeRust.calls("delete").last?.action["id"] as? String == itemID)
    // 「清空云端历史」是 clear：删掉全部条目、开关不变。
    clipboard.text = "合成清理内容"; clipboard.upload(); try await finished(clipboard)
    clipboard.delete(); try await finished(clipboard)
    try require(clipboard.items.isEmpty && clipboard.enabled && FakeRust.calls("clear").count == 1)
    clipboard.setEnabled(false); try await finished(clipboard)
    try require(!clipboard.enabled && clipboard.items.isEmpty)
    // 打开窗口时的账号已经不是登录的那个：什么都不发，也不显示。
    let wrongAccount = MacClipboardModel(accountID: "previous-account", rust: FakeRust.account, deviceName: "合成 Mac")
    let sentBefore = sent().count
    wrongAccount.refresh(); try await finished(wrongAccount)
    try require(!wrongAccount.loaded && wrongAccount.items.isEmpty && sent().count == sentBefore)
    clipboard.text = "未上传的草稿"; clipboard.close()
    try require(clipboard.text.isEmpty && clipboard.items.isEmpty)
  }
  /// 云词库窗口经 Rust（#8201 迁移计划 4c）：列表、增删改、导入和导出发出的 action，以及导出由 Rust 直接写进用户选的位置。
  @MainActor static func rustDictionaryModel() async throws {
    final class Cloud: @unchecked Sendable { var entries: [[String: Any]] = [] }
    let cloud = Cloud()
    let entryID = String(repeating: "d", count: 64)
    FakeRust.answer { call in
      if call.entry == "account" { return FakeRust.ok(["user": FakeRust.user("synthetic-user")]) }
      let action = call.action
      func entry(revision: Int) -> [String: Any] {
        ["id": entryID, "kind": action["kind"] ?? "pinyin", "code": action["code"] ?? "", "word": action["word"] ?? "", "weight": action["weight"] ?? 0, "revision": revision]
      }
      switch call.operation {
      case "list":
        return FakeRust.ok(["entries": cloud.entries.filter { $0["kind"] as? String == action["kind"] as? String }, "has_more": false, "offset": action["offset"] ?? 0])
      case "add":
        cloud.entries = [entry(revision: 1)]
        return FakeRust.ok(["revision": 1, "previous": NSNull(), "replacement": cloud.entries[0]])
      case "update":
        guard action["revision"] as? Int == cloud.entries.first?["revision"] as? Int else { return FakeRust.failure("account_conflict") }
        let previous = cloud.entries[0]
        cloud.entries = [entry(revision: 2)]
        return FakeRust.ok(["revision": 2, "previous": previous, "replacement": cloud.entries[0]])
      case "delete":
        let previous = cloud.entries[0]
        cloud.entries = []
        return FakeRust.ok(["revision": 3, "previous": previous, "replacement": NSNull()])
      case "import": return FakeRust.ok(["imported": 2, "revision": 4])
      case "export_file":
        guard let path = action["path"] as? String else { return FakeRust.failure("invalid_cloud_dictionary") }
        try? Data("合成\tci\t100\n".utf8).write(to: URL(fileURLWithPath: path))
        return FakeRust.ok(["path": path, "filename": "dictionary-pinyin.tsv", "bytes": 14])
      default: return FakeRust.failure("invalid_cloud_dictionary")
      }
    }
    let model = MacDictionaryModel(accountID: "synthetic-user", rust: FakeRust.account)
    model.refresh(); try await finished(model)
    let list = FakeRust.calls("list").last
    try require(model.page?.entries.isEmpty == true && list?.action["kind"] as? String == "pinyin" && list?.action["offset"] as? Int == 0
                && list?.request["snapshot_directory"] as? String == (try MacCloudTransferDirectory.url()).path
                && list?.request["session_store"] as? Int == 7)
    model.save(.init(code: "ce shi", word: "测试", weight: 100000)); try await finished(model)
    let added = FakeRust.calls("add").last?.action
    try require(model.page?.entries.first?.word == "测试" && added?["code"] as? String == "ce shi" && added?["weight"] as? Int == 100000)
    let row = model.page!.entries[0]
    model.save(.init(code: "ce shi", word: "测验", weight: 100000), replacing: row); try await finished(model)
    let updated = FakeRust.calls("update").last?.action
    try require(model.page?.entries.first?.word == "测验" && updated?["id"] as? String == entryID && updated?["revision"] as? Int == 1)
    // 拿旧的版本号再改一次：云端已被改过，冲突原样报出来，页面清空等刷新。
    model.save(.init(code: "ce shi", word: "又改", weight: 1), replacing: row); try await finished(model)
    try require(model.page == nil && model.message == "云端内容已被其他设备更新，请刷新后重新确认。")
    model.refresh(); try await finished(model)
    model.delete(model.page!.entries[0]); try await finished(model)
    try require(model.page?.entries.isEmpty == true && FakeRust.calls("delete").last?.action["revision"] as? Int == 2)
    model.importText = "合成\tci\t100\n测验\tce yan\t100\n"; model.uploadImport(); try await finished(model)
    try require(model.importText == nil && model.message == "已导入 2 个词条。"
                && FakeRust.calls("import").last?.action["format"] as? String == "standard")
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    let destination = directory.appendingPathComponent("水杉-拼音.tsv")
    model.export(to: destination); try await finished(model)
    let exported = FakeRust.calls("export_file").last?.action
    try require(model.message == "云词库已导出。" && exported?["path"] as? String == destination.path && exported?["format"] as? String == "standard"
                && (try? String(contentsOf: destination, encoding: .utf8)) == "合成\tci\t100\n")
    // 打开窗口时的账号已经不是登录的那个：不发云词库请求。
    let wrongAccount = MacDictionaryModel(accountID: "previous-account", rust: FakeRust.account)
    let before = FakeRust.calls.filter { $0.entry == "cloud_dictionary" }.count
    wrongAccount.refresh(); try await finished(wrongAccount)
    try require(wrongAccount.page == nil && FakeRust.calls.filter { $0.entry == "cloud_dictionary" }.count == before)
    model.close()
  }
  /// 整份快照窗口经 Rust（#8201 迁移计划 4c）：「应用到本机」带本机的 HostOptions 排进 Rust 的快照队列，没能马上应用时窗口开着就问状态；备份、恢复按用户选的路径交给 Rust。
  @MainActor static func rustSnapshotModel() async throws {
    final class Queue: @unchecked Sendable { var enqueued = "applied"; var later = "applied" }
    let queue = Queue()
    let metadata: [String: Any] = ["cloudRevision": 5, "sha256": String(repeating: "e", count: 64), "bytes": 120, "records": 7,
                                   "entries": 3, "overlays": 2, "positions": 1, "selections": 4]
    FakeRust.answer { call in
      if call.entry == "account" { return FakeRust.ok(["user": FakeRust.user("synthetic-user")]) }
      let request: (String) -> [String: Any] = { ["id": "synthetic-request", "cloudRevision": 5, "fileSha256": String(repeating: "f", count: 64), "status": $0] }
      switch call.operation {
      case "snapshot_preview": return FakeRust.ok(["previewToken": "preview-1", "snapshot": metadata, "localVersion": "local-1"])
      case "snapshot_enqueue":
        // 处理队列要等输入会话放开词库，与真的 Rust 一样占住调用线程一会儿。
        usleep(300_000)
        return FakeRust.ok(["localVersion": "local-1", "request": request(queue.enqueued)])
      case "snapshot_status": return FakeRust.ok(["localVersion": "local-1", "request": request(queue.later)])
      case "snapshot_export_file":
        guard let path = call.action["path"] as? String else { return FakeRust.failure("account_invalid") }
        try? Data("{}\n".utf8).write(to: URL(fileURLWithPath: path))
        return FakeRust.ok(["path": path, "filename": "msime-dictionary-snapshot.ndjson", "snapshot": metadata])
      case "snapshot_restore_preview_file": return FakeRust.ok(["snapshot": metadata, "expectedRevision": 5])
      case "snapshot_restore_file": return FakeRust.ok(["revision": 6, "reset": true])
      default: return FakeRust.failure("invalid_cloud_dictionary")
      }
    }
    let options: [String: Any] = ["user_data": "/synthetic/state/user", "dictionaries": "/synthetic/dictionaries"]
    let model = MacSnapshotModel(accountID: "synthetic-user", rust: FakeRust.account, hostOptions: { options })
    model.downloadForLocal(); try await finished(model)
    try require(model.localPreview && model.preview?.entries == 3 && model.preview?.selections == 4
                && (FakeRust.calls("snapshot_preview").last?.request["options"] as? [String: String])?["user_data"] == "/synthetic/state/user")
    var wakes = 0
    let observer = NotificationCenter.default.addObserver(forName: Notification.Name("MSIMEDictionaryMaintenanceWillBeginNotification"),
                                                          object: nil, queue: nil) { _ in wakes += 1 }
    defer { NotificationCenter.default.removeObserver(observer) }
    model.applyLocal(); try await finished(model)
    // 排队处理期间叫醒本进程里握着词库的输入会话，它们才会放手。
    try require(wakes > 0)
    let enqueued = FakeRust.calls("snapshot_enqueue").last
    try require(model.preview == nil && model.message == "云词库已应用到本机，后续输入将使用新词库。"
                && enqueued?.action["token"] as? String == "preview-1" && (enqueued?.request["options"] as? [String: String])?["dictionaries"] == "/synthetic/dictionaries")
    // 输入会话一时放不开词库：请求留在队列里，窗口开着时问状态，直到应用完。
    queue.enqueued = "queued"
    model.downloadForLocal(); try await finished(model)
    model.applyLocal(); try await finished(model)
    try require(model.message == "已排进本机的词库队列，输入法放开词库后应用。关掉窗口也不会丢。")
    let deadline = Date().addingTimeInterval(8)
    while model.message != "云词库已应用到本机，后续输入将使用新词库。" && Date() < deadline { try await Task.sleep(nanoseconds: 50_000_000) }
    try require(model.message == "云词库已应用到本机，后续输入将使用新词库。" && !FakeRust.calls("snapshot_status").isEmpty)
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    let backup = directory.appendingPathComponent("水杉完整云词库.ndjson")
    model.backup(to: backup); try await finished(model)
    try require(model.message == "已保存完整云词库备份。" && FakeRust.calls("snapshot_export_file").last?.action["path"] as? String == backup.path
                && FileManager.default.fileExists(atPath: backup.path))
    model.prepare(backup); try await finished(model)
    try require(model.preview?.entries == 3 && !model.localPreview)
    model.restore(); try await finished(model)
    let restored = FakeRust.calls("snapshot_restore_file").last?.action
    try require(model.preview == nil && model.message == "完整备份已恢复到云端。本机词库尚未替换。"
                && restored?["path"] as? String == backup.path && restored?["expected_sha256"] as? String == String(repeating: "e", count: 64)
                && restored?["revision"] as? Int == 5)
    // 独立设置窗口没有输入会话，拿不到 HostOptions：报原因，不发请求。
    let previews = FakeRust.calls("snapshot_preview").count
    let standalone = MacSnapshotModel(accountID: "synthetic-user", rust: FakeRust.account,
                                      hostOptions: { throw BackendRustClient.Failure(code: "snapshot_unavailable") })
    standalone.downloadForLocal(); try await finished(standalone)
    try require(standalone.preview == nil && standalone.message == "词库快照暂时不可用，请稍后重试。"
                && FakeRust.calls("snapshot_preview").count == previews)
    model.close()
  }
  /// 桌面设置同步经 Rust（#8201 迁移计划 4e）：字段表（camelCase）、云端文档和上传都是 `msime_client_account` 的 action，`platform.macos.*` 的映射留在 Swift。
  @MainActor static func rustSettingsModel() async throws {
    let cloud = CloudPreferences()
    FakeRust.answer(cloud.respond)
    var localSettings: MacSettingsAccess.Values = ["platform.macos.global_theme": .string("shuishan"), "platform.macos.custom_theme_base": .string("system"), "platform.macos.custom_candidate_skin": .string(""), "platform.macos.custom_candidate_skin_dark": .string("night-card"), "platform.macos.candidate_font_size": .integer(16), "platform.macos.candidate_learning": .boolean(false), "platform.macos.shuangpin_preedit_uses_raw": .boolean(false)]
    let settings = MacSettingsModel(accountID: "synthetic-user", rust: FakeRust.account, local: .init(snapshot: { localSettings }, validate: { values in
      guard values.count == 7 else { throw Failure() }
    }, apply: { localSettings = $0 }))
    settings.download(); try await finished(settings)
    try require(settings.preview?["platform.macos.candidate_font_size"] == .integer(18))
    try require(settings.preview?["platform.macos.global_theme"] == .string("shuishan"))
    try require(settings.preview?["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(false))
    // 云端的辅助码方案下标 9 是本机没有的方案，预览换成本机的选择；6（五笔 86）本机认得，照常进预览。
    let helpcodeLocal: MacSettingsAccess.Values = ["platform.macos.candidate_font_size": .integer(16), "platform.macos.quanpin_helpcode_schema": .integer(0), "platform.macos.shuangpin_helpcode_schema": .integer(2)]
    let helpcodeSettings = MacSettingsModel(accountID: "synthetic-user", rust: FakeRust.account, local: .init(snapshot: { helpcodeLocal }, validate: { _ in }, apply: { _ in }))
    helpcodeSettings.download(); try await finished(helpcodeSettings)
    try require(helpcodeSettings.preview?["platform.macos.quanpin_helpcode_schema"] == .integer(6))
    try require(helpcodeSettings.preview?["platform.macos.shuangpin_helpcode_schema"] == .integer(2))
    helpcodeSettings.close()
    cloud.omitted = "platform.macos.candidate_font_size"
    settings.download(); try await finished(settings)
    try require(settings.preview == nil && settings.message != nil)
    cloud.omitted = nil
    settings.download(); try await finished(settings)
    localSettings["platform.macos.candidate_font_size"] = .integer(20)
    settings.apply(); try await finished(settings)
    try require(settings.message != nil && localSettings["platform.macos.candidate_font_size"] == .integer(20))
    settings.download(); try await finished(settings)
    settings.apply(); try await finished(settings)
    try require(localSettings["platform.macos.candidate_font_size"] == .integer(18))
    try require(localSettings["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(false))
    // A cloud-supported explicit value must win over the local fallback on restore.
    localSettings["platform.macos.shuangpin_preedit_uses_raw"] = .boolean(true)
    settings.upload(); try await finished(settings)
    let savedPreferences = cloud.document
    // 上传带读文档时的修订号和打开窗口时的账号（P4 的 `preferences_put`）。
    let put = FakeRust.calls("preferences_put").last?.action
    try require(put?["user_id"] as? String == "synthetic-user" && put?["revision"] as? Int == 1 && put?["document"] is [String: Any])
    try require(savedPreferences.settings["platform.ios.nine_key"] == .boolean(true))
    try require(savedPreferences.settings["platform.macos.global_theme"] == .string("shuishan"))
    try require(savedPreferences.settings["platform.macos.custom_theme_base"] == .string("system") && savedPreferences.settings["platform.macos.custom_candidate_skin"] == .string(""))
    try require(savedPreferences.settings["platform.macos.custom_candidate_skin_dark"] == .string("night-card"))
    try require(settings.message == "本机设置已上传，其他平台的云端设置已保留。")
    try require(savedPreferences.settings["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(true))
    localSettings["platform.macos.shuangpin_preedit_uses_raw"] = .boolean(false)
    settings.download(); try await finished(settings)
    try require(settings.preview?["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(true))
    settings.apply(); try await finished(settings)
    try require(localSettings["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(true))
    // 另一台设备在这期间改过云端文档：按读到时的修订号比较并交换，冲突原样报出来，不重放旧文档。
    cloud.revision += 1
    settings.upload(); try await finished(settings)
    try require(settings.message == "云端内容已被其他设备更新，请刷新后重新确认。")
    // A server without the whole theme group still takes the other settings; the theme is left out as a group and the message says so, rather than the whole upload failing.
    cloud.themeSchema = false
    localSettings["platform.macos.global_theme"] = .string("night")
    localSettings["platform.macos.candidate_font_size"] = .integer(22)
    settings.download(); try await finished(settings)
    settings.upload(); try await finished(settings)
    try require(settings.message == "本机设置已上传，其他平台的云端设置已保留。云端暂不支持主题设置，主题没有上传。")
    let themeless = cloud.document
    try require(themeless.settings["platform.macos.candidate_font_size"] == .integer(22) && themeless.settings["platform.macos.global_theme"] == .string("shuishan"))
    cloud.themeSchema = true
    // 服务端字段表只缺深色槽位：只去掉这一项，主题组其余字段照常上传，提示也不说主题没上传。
    cloud.darkSkinSchema = false
    localSettings["platform.macos.global_theme"] = .string("ink")
    localSettings["platform.macos.custom_candidate_skin_dark"] = .string("dusk-card")
    settings.download(); try await finished(settings)
    settings.upload(); try await finished(settings)
    try require(settings.message == "本机设置已上传，其他平台的云端设置已保留。")
    let darkless = cloud.document
    try require(darkless.settings["platform.macos.global_theme"] == .string("ink") && darkless.settings["platform.macos.custom_candidate_skin_dark"] == .string("night-card"))
    cloud.darkSkinSchema = true
    // 云端还没有深色槽位：下载时保留本机的值，不因少一项拒绝整份快照。
    cloud.omitted = "platform.macos.custom_candidate_skin_dark"
    settings.download(); try await finished(settings)
    try require(settings.preview?["platform.macos.custom_candidate_skin_dark"] == .string("dusk-card"))
    cloud.omitted = nil
    settings.close(); try require(settings.preview == nil && settings.cloud == nil)
    // 打开窗口时的账号已经不是登录的那个：不发设置请求。
    let loads = FakeRust.calls("preferences_load").count
    let wrongAccount = MacSettingsModel(accountID: "previous-account", rust: FakeRust.account, local: .init(snapshot: { localSettings }, validate: { _ in }, apply: { _ in }))
    wrongAccount.download(); try await finished(wrongAccount)
    try require(wrongAccount.preview == nil && FakeRust.calls("preferences_load").count == loads)
  }
  /// 候选释义经 Rust 的 `translate`（#8201 迁移计划 4e）：回答按问之前登录的账号打标记，问完换了号就丢掉；两个账号都没有时补注册一次匿名账号再问，600 秒内只补一次；输入法同步读的标记来自 `status` 的缓存。
  @MainActor static func rustCandidateGloss() async throws {
    final class Script: @unchecked Sendable {
      var users: [Any] = []
      var signedIn: Any = NSNull()
      var unauthorized = false
      func respond(_ call: FakeRust.Call) -> String {
        switch call.operation {
        case "status":
          if !users.isEmpty { return FakeRust.ok(["user": users.removeFirst()]) }
          return FakeRust.ok(["user": signedIn])
        case "translate":
          if unauthorized { return FakeRust.failure("account_unauthorized") }
          let texts = call.action["texts"] as? [String] ?? []
          return FakeRust.ok(["translations": texts.map { $0 == "测试" ? "test" as Any : NSNull() }])
        case "ensure_anonymous": unauthorized = false; return FakeRust.ok(true)
        default: return FakeRust.failure("account_invalid")
        }
      }
    }
    let script = Script()
    FakeRust.answer(script.respond)
    final class Posted: @unchecked Sendable { var items: [[AnyHashable: Any]] = [] }
    let box = Posted()
    let observer = NotificationCenter.default.addObserver(forName: BackendCandidateGloss.notification, object: nil, queue: nil) { box.items.append($0.userInfo ?? [:]) }
    defer { NotificationCenter.default.removeObserver(observer) }
    var posted: [[AnyHashable: Any]] { box.items }
    let request = BackendCandidateGloss.Request(words: ["测试", "东京"], primary: "en", secondary: "", generation: 9)
    script.signedIn = FakeRust.user("synthetic-user")
    await BackendCandidateGloss.translate(request, rust: FakeRust.account)
    try await Task.sleep(nanoseconds: 20_000_000)
    let signedInMarker = backendAccountTranslationMarker(userID: "synthetic-user")
    let sent = FakeRust.calls("translate").last?.action
    try require(posted.count == 1 && posted[0]["marker"] as? UInt64 == signedInMarker && posted[0]["target"] as? String == "en"
                && posted[0]["translations"] as? [String: String] == ["测试": "test", "东京": ""]
                && sent?["texts"] as? [String] == ["测试", "东京"] && sent?["source_lang"] as? String == "ZH" && sent?["target_lang"] as? String == "en")
    try require(msimeBackendAccountTranslationMarker() == signedInMarker)
    // 问完再看一次登录状态，换了号就不交给输入法。
    script.users = [FakeRust.user("synthetic-user"), FakeRust.user("another-user")]
    await BackendCandidateGloss.translate(request, rust: FakeRust.account)
    try await Task.sleep(nanoseconds: 20_000_000)
    try require(posted.count == 1)
    // 没有登录、也没有匿名会话：补注册一次匿名账号再问，回答的标记是 0。
    script.signedIn = NSNull(); script.unauthorized = true
    await BackendCandidateGloss.translate(request, rust: FakeRust.account)
    try await Task.sleep(nanoseconds: 20_000_000)
    try require(posted.count == 2 && posted[1]["marker"] as? UInt64 == 0 && FakeRust.calls("ensure_anonymous").count == 1
                && msimeBackendAccountTranslationMarker() == 0)
    // 600 秒内不再补注册：这一页没有释义，下一页再问。
    script.unauthorized = true
    await BackendCandidateGloss.translate(request, rust: FakeRust.account)
    try await Task.sleep(nanoseconds: 20_000_000)
    try require(posted.count == 2 && FakeRust.calls("ensure_anonymous").count == 1)
  }
  /// 与 iOS 共用的「词包与回复模板」「完整词库目录」「云端候选与排序」经 Rust（#8201 迁移计划 4c）：请求带 `session_store`、前后都核对打开时的账号；目录页把 `catalog_entries` 读成词条；候选页的查询类 action 带同样的五项。
  @MainActor static func rustSharedCloudPages() async throws {
    final class Script: @unchecked Sendable { var user: Any = FakeRust.user("synthetic-user") }
    let script = Script()
    FakeRust.answer { call in
      switch (call.entry, call.operation) {
      case ("account", "status"): return FakeRust.ok(["user": script.user])
      case ("community", "resource_list"): return FakeRust.ok(["items": [], "has_more": false])
      case ("cloud_dictionary", "catalog"):
        return FakeRust.ok(["catalog_entries": [["kind": "pinyin", "code": "shi", "word": "是", "weight": 9]], "has_more": false,
                            "offset": 0, "revision": 4, "normalized": "shi"])
      default: return FakeRust.failure("account_invalid")
      }
    }
    let account = CommunityResourceAccount(accountID: "synthetic-user", rust: FakeRust.account)
    let page = try await account.decode(BackendAccountClient.ResourcePage.self, ["operation": "resource_list", "kind": "dictionary",
                                                                                  "scope": "", "search": "", "offset": 0])
    try require(page.items.isEmpty && FakeRust.calls("resource_list").first?.request["session_store"] as? Int == 7)
    script.user = FakeRust.user("another-user")
    do { _ = try await account.community(["operation": "resource_list"]); throw Failure() } catch is CancellationError { }
    try require(FakeRust.calls("resource_list").count == 1)
    script.user = FakeRust.user("synthetic-user")
    let catalog = try CloudDictionaryCatalogView.catalog(BackendRustAccount.object(try await account.cloudDictionary(["operation": "catalog"])))
    try require(catalog.entries.first?.word == "是" && catalog.revision == 4 && catalog.normalized == "shi")
    let query = BackendAccountClient.CandidateQuery(text: "shi", kind: "pinyin", scheme: "shuangpin", profile: "xiaohe", limit: 100)
    let action = CloudCandidatesView.queryAction("rank", query)
    try require(action["operation"] as? String == "rank" && action["text"] as? String == "shi" && action["kind"] as? String == "pinyin"
                && action["scheme"] as? String == "shuangpin" && action["profile"] as? String == "xiaohe" && action["limit"] as? Int == 100)
  }
  @MainActor static func main() async throws {
    // 云词库和快照窗口的中转目录在 $TMPDIR 下按进程号建，测试结束时删掉。
    defer { if let transfer = try? MacCloudTransferDirectory.url() { try? FileManager.default.removeItem(at: transfer) } }
    try windowAccountIsolation()
    try candidateGlossPayload()
    try await candidateGlossSingleFlight()
    try await anonymousAccountFallback()
    try await rustAccountModel()
    try await rustClipboardModel()
    try await rustDictionaryModel()
    try await rustSnapshotModel()
    try await rustSettingsModel()
    try await rustCandidateGloss()
    try await rustSharedCloudPages()
    print("PASS: native account model through Rust, local account card, settings sync and cloud clipboard")
  }

  @MainActor static func windowAccountIsolation() throws {
    final class Window {
      var visible = true
      var closed = false
    }
    let cache = BackendAccountWindowCache<Window>()
    var creations = 0
    func open(_ surface: String, _ account: String) -> Window {
      cache.window(for: surface, accountID: account, reusable: { $0.visible },
        close: { $0.closed = true; $0.visible = false },
        create: { creations += 1; return Window() })
    }
    let dictionary = open("dictionary", "synthetic-a")
    try require(open("dictionary", "synthetic-a") === dictionary && creations == 1)
    let clipboard = open("clipboard", "synthetic-a")
    try require(!dictionary.closed && !clipboard.closed && creations == 2)
    let otherAccount = open("dictionary", "synthetic-b")
    try require(otherAccount !== dictionary && dictionary.closed && clipboard.closed)
    try require(creations == 3)
    otherAccount.visible = false
    let reopened = open("dictionary", "synthetic-b")
    try require(reopened !== otherAccount && otherAccount.closed && creations == 4)
    let returned = open("dictionary", "synthetic-a")
    try require(returned !== dictionary && reopened.closed && creations == 5)
    var closedCount = 0
    cache.closeAll { $0.closed = true; closedCount += 1 }
    try require(returned.closed && closedCount == 1)
    cache.closeAll { _ in closedCount += 1 }
    try require(closedCount == 1)
    try require(open("dictionary", "synthetic-a") !== returned && creations == 6)
  }
}

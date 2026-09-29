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
  static var omittedPreferenceKey: String?
  static var themeSchema = true
  private static var preferenceRevision = 1
  private static var preferences: [String: Any] = ["platform.macos.candidate_font_size": 18, "platform.macos.candidate_learning": true, "platform.ios.nine_key": true]
  private static var clipboardEnabled = false
  private static var clipboardText: String?
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let body: String
    var status = 200
    var payload = request.httpBody ?? Data()
    if let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var buffer = [UInt8](repeating: 0, count: 4096)
      while true { let count = stream.read(&buffer, maxLength: buffer.count); if count <= 0 { break }; payload.append(contentsOf: buffer.prefix(count)) }
    }
    let values = (try? JSONSerialization.jsonObject(with: payload)) as? [String: Any]
    let itemID = String(repeating: "c", count: 64)
    func item(_ text: String) -> [String: String] { ["id": itemID, "text": text, "updated_at": "2026-09-08"] }
    func json(_ object: Any) -> String { String(data: try! JSONSerialization.data(withJSONObject: object), encoding: .utf8)! }
    switch (request.httpMethod!, request.url!.path) {
    case ("GET", "/v1/users/me/preferences/schema"):
      var fields: [String: Any] = ["platform.macos.global_theme": ["type":"string", "maxLength":64], "platform.macos.candidate_font_size": ["type":"integer"], "platform.macos.candidate_learning": ["type":"boolean"], "platform.macos.shuangpin_preedit_uses_raw": ["type":"boolean"], "platform.ios.nine_key": ["type":"boolean"]]
      // A server that has registered only part of the theme group, as one that predates the custom theme would.
      if Self.themeSchema { fields["platform.macos.custom_theme_base"] = ["type":"string", "maxLength":64]; fields["platform.macos.custom_candidate_skin"] = ["type":"string", "maxLength":128] }
      body = json(["fields": fields, "maximum_bytes": 1048576, "update_mode": "replace", "revision_required": true])
    case ("GET", "/v1/users/me/preferences"):
      body = json(["revision":Self.preferenceRevision, "settings":Self.preferences.filter { $0.key != Self.omittedPreferenceKey }])
    case ("PUT", "/v1/users/me/preferences"):
      if values?["revision"] as? Int == Self.preferenceRevision, let settings = values?["settings"] as? [String: Any] {
        Self.preferenceRevision += 1; Self.preferences = settings
        body = json(["revision":Self.preferenceRevision, "settings":Self.preferences])
      } else { body = "{}"; status = 409 }
    case ("PUT", "/v1/users/me/clipboard/settings"):
      Self.clipboardEnabled = values?["enabled"] as? Bool ?? false
      if !Self.clipboardEnabled { Self.clipboardText = nil }
      body = ""; status = 204
    case ("GET", "/v1/users/me/clipboard"):
      body = json(["enabled": Self.clipboardEnabled, "items": Self.clipboardText.map { [item($0)] } ?? []])
    case ("POST", "/v1/users/me/clipboard"):
      Self.clipboardText = values?["text"] as? String
      body = json(item(Self.clipboardText ?? ""))
    case ("DELETE", "/v1/users/me/clipboard"), ("DELETE", "/v1/users/me/clipboard/" + itemID):
      Self.clipboardText = nil; body = ""; status = 204
    case (_, "/v1/auth/providers"): body = #"{"providers":{"email":true,"phone":false,"apple":false}}"#
    case (_, "/v1/auth/challenges"): body = #"{"challenge_id":"synthetic","expires_in":300}"#
    case (_, "/v1/auth/login"):
      let token = String(repeating: "a", count: 64), refresh = String(repeating: "b", count: 64)
      body = "{\"access_token\":\"\(token)\",\"refresh_token\":\"\(refresh)\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"synthetic-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08\"}}"
    case (_, "/v1/auth/logout"): body = ""; status = Self.failLogout ? 503 : 204
    case ("PATCH", "/v1/users/me"): body = ""; status = 204
    case ("GET", "/v1/users/me"): body = #"{"user":{"id":"synthetic-user","display_name":"新昵称","created_at":"2026-09-08"},"identities":[]}"#
    case ("DELETE", "/v1/users/me"):
      body = Self.allowDelete ? "" : #"{"error":{"code":"recent_login_required"}}"#
      status = Self.allowDelete ? 204 : 403
    default: body = "{}"; status = 404
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type":"application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8)); client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
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
  @MainActor static func finished(_ model: MacSettingsModel) async throws {
    let deadline = Date().addingTimeInterval(5)
    while model.busy && Date() < deadline { try await Task.sleep(nanoseconds: 5_000_000) }
    try require(!model.busy)
  }
  @MainActor static func fileTransfer() async throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    let source = directory.appendingPathComponent("backup.ndjson")
    let destination = directory.appendingPathComponent("saved.ndjson")
    let original = Data("existing backup".utf8), replacement = Data("new backup".utf8)
    try replacement.write(to: source); try original.write(to: destination)
    var authorizations = 0
    var rejected = false
    do {
      try await MacCloudFileTransfer.save(source, to: destination) {
        authorizations += 1
        if authorizations == 2 { throw CancellationError() }
      }
    } catch is CancellationError { rejected = true }
    try require(rejected && authorizations == 2)
    try require(try Data(contentsOf: destination) == original)
    try require(try Set(FileManager.default.contentsOfDirectory(atPath: directory.path)) == ["backup.ndjson", "saved.ndjson"])
    // A missing download must not destroy an existing user backup either.
    rejected = false
    do { try await MacCloudFileTransfer.save(directory.appendingPathComponent("missing"), to: destination) {} }
    catch { rejected = true }
    try require(rejected && (try Data(contentsOf: destination)) == original)
    try await MacCloudFileTransfer.save(source, to: destination) {}
    try require(try Data(contentsOf: destination) == replacement)
    try require(try Set(FileManager.default.contentsOfDirectory(atPath: directory.path)) == ["backup.ndjson", "saved.ndjson"])
  }
  @MainActor static func anonymousAccountFallback() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AccountFixture.self]
    let client = BackendAccountClient(configuration: configuration)
    let realStorage = MemoryCredentials()
    let anonymousStorage = MemoryCredentials()
    let anonymousSession = BackendAccountSession(api: client, storage: anonymousStorage)
    try await anonymousSession.signIn(challenge: "synthetic", credential: "123456")
    var discarded = 0
    let model = MacAccountModel(client: client, account: BackendAccountSession(api: client, storage: realStorage),
                                anonymousAccount: anonymousSession,
                                closeAccountWindows: {}, discardAnonymous: { discarded += 1 })
    model.load(); try await finished(model)
    try require(model.user?.id == "synthetic-user" && model.anonymous)
    model.name = "匿名昵称"; model.rename(); try await finished(model)
    try require(model.user?.display_name == "新昵称" && anonymousStorage.load()?.tokens.user.display_name == "新昵称")
    model.name = "有效\n中文"; model.rename(); try await finished(model)
    let storedName = try anonymousStorage.load()?.tokens.user.display_name
    try require(model.message != nil && storedName == "新昵称")
    let realSession = BackendAccountSession(api: client, storage: realStorage)
    try await realSession.signIn(challenge: "synthetic", credential: "123456")
    let priorityModel = MacAccountModel(client: client, account: realSession, anonymousAccount: anonymousSession)
    priorityModel.load(); try await finished(priorityModel)
    try require(priorityModel.user?.id == "synthetic-user" && !priorityModel.anonymous)
    try await realSession.forget()
    AccountFixture.allowDelete = true
    defer { AccountFixture.allowDelete = false }
    model.logout(delete: true); try await finished(model)
    try require(model.user == nil && !model.anonymous && discarded == 1 && anonymousStorage.load() == nil)
  }
  @MainActor static func main() async throws {
    try windowAccountIsolation()
    try await fileTransfer()
    try await anonymousAccountFallback()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AccountFixture.self]
    let client = BackendAccountClient(configuration: configuration)
    let storage = MemoryCredentials()
    let session = BackendAccountSession(api: client, storage: storage)
    var clipboardAccount: String?
    var signInRequests = 0
    await BackendClipboardEntry.open(account: session, present: { clipboardAccount = $0 }, signIn: { signInRequests += 1 })
    try require(clipboardAccount == nil && signInRequests == 1)
    var windowClosures = 0
    let model = MacAccountModel(client: client, account: session, closeAccountWindows: { windowClosures += 1 })
    model.load(); try await finished(model)
    try require(model.providers["email"] == true && model.user == nil)
    model.channel = "phone"; model.target = "+10000000000"
    model.requestCode(); try await finished(model)
    try require(model.challenge == nil && model.message != nil)
    model.channel = "email"; model.target = "synthetic@example.invalid"
    model.requestCode(); try await finished(model)
    try require(model.challenge != nil && model.resendAt > Date())
    model.code = "123456"; model.codeLogin(); try await finished(model)
    try require(model.user?.id == "synthetic-user" && model.code.isEmpty && model.target.isEmpty)
    await BackendClipboardEntry.open(account: session, present: { clipboardAccount = $0 }, signIn: { signInRequests += 1 })
    try require(clipboardAccount == "synthetic-user" && signInRequests == 1)
    model.name = "新昵称"; model.rename(); try await finished(model)
    try require(model.user?.display_name == "新昵称" && storage.load()?.tokens.user.display_name == "新昵称")
    model.logout(delete: true); try await finished(model)
    try require(model.user != nil && model.message != nil && storage.load() != nil)
    try require(windowClosures == 0)
    var localSettings: MacSettingsAccess.Values = ["platform.macos.global_theme": .string("shuishan"), "platform.macos.custom_theme_base": .string("system"), "platform.macos.custom_candidate_skin": .string(""), "platform.macos.candidate_font_size": .integer(16), "platform.macos.candidate_learning": .boolean(false), "platform.macos.shuangpin_preedit_uses_raw": .boolean(false)]
    let settings = MacSettingsModel(accountID: "synthetic-user", client: client, account: session, local: .init(snapshot: { localSettings }, validate: { values in
      guard values.count == 6 else { throw Failure() }
    }, apply: { localSettings = $0 }))
    settings.download(); try await finished(settings)
    try require(settings.preview?["platform.macos.candidate_font_size"] == .integer(18))
    try require(settings.preview?["platform.macos.global_theme"] == .string("shuishan"))
    try require(settings.preview?["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(false))
    AccountFixture.omittedPreferenceKey = "platform.macos.candidate_font_size"
    settings.download(); try await finished(settings)
    try require(settings.preview == nil && settings.message != nil)
    AccountFixture.omittedPreferenceKey = nil
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
    let credentials = try await session.credentials()
    let savedPreferences = try await client.preferences(token: credentials.token)
    try require(savedPreferences.settings["platform.ios.nine_key"] == .boolean(true))
    try require(savedPreferences.settings["platform.macos.global_theme"] == .string("shuishan"))
    try require(savedPreferences.settings["platform.macos.custom_theme_base"] == .string("system") && savedPreferences.settings["platform.macos.custom_candidate_skin"] == .string(""))
    try require(settings.message == "本机设置已上传，其他平台的云端设置已保留。")
    try require(savedPreferences.settings["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(true))
    localSettings["platform.macos.shuangpin_preedit_uses_raw"] = .boolean(false)
    settings.download(); try await finished(settings)
    try require(settings.preview?["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(true))
    settings.apply(); try await finished(settings)
    try require(localSettings["platform.macos.shuangpin_preedit_uses_raw"] == .boolean(true))
    _ = try await client.putPreferences(savedPreferences, token: credentials.token)
    settings.upload(); try await finished(settings)
    try require(settings.message != nil)
    // A server without the whole theme group still takes the other settings; the theme is left out as a group and the message says so, rather than the whole upload failing.
    AccountFixture.themeSchema = false
    localSettings["platform.macos.global_theme"] = .string("night")
    localSettings["platform.macos.candidate_font_size"] = .integer(22)
    settings.download(); try await finished(settings)
    settings.upload(); try await finished(settings)
    try require(settings.message == "本机设置已上传，其他平台的云端设置已保留。云端暂不支持主题设置，主题没有上传。")
    let themeless = try await client.preferences(token: credentials.token)
    try require(themeless.settings["platform.macos.candidate_font_size"] == .integer(22) && themeless.settings["platform.macos.global_theme"] == .string("shuishan"))
    AccountFixture.themeSchema = true
    settings.close(); try require(settings.preview == nil && settings.cloud == nil)
    let clipboard = MacClipboardModel(accountID: "synthetic-user", client: client, account: session)
    clipboard.refresh(); try await finished(clipboard)
    try require(clipboard.loaded && !clipboard.enabled && clipboard.items.isEmpty)
    clipboard.setEnabled(true); try await finished(clipboard)
    clipboard.text = "合成剪贴板内容"; clipboard.upload(); try await finished(clipboard)
    try require(clipboard.items.first?.text == "合成剪贴板内容" && clipboard.text.isEmpty)
    clipboard.delete(id: clipboard.items.first!.id); try await finished(clipboard)
    try require(clipboard.items.isEmpty)
    clipboard.text = "合成清理内容"; clipboard.upload(); try await finished(clipboard)
    clipboard.setEnabled(false); try await finished(clipboard)
    try require(!clipboard.enabled && clipboard.items.isEmpty)
    let wrongAccount = MacClipboardModel(accountID: "previous-account", client: client, account: session)
    wrongAccount.refresh(); try await finished(wrongAccount)
    try require(!wrongAccount.loaded && wrongAccount.items.isEmpty)
    clipboard.text = "未上传的草稿"; clipboard.close()
    try require(clipboard.text.isEmpty && clipboard.items.isEmpty)
    model.logout(all: true); try await finished(model)
    try require(model.user == nil && storage.load() == nil)
    try require(windowClosures == 1)
    clipboardAccount = nil
    await BackendClipboardEntry.open(account: session, present: { clipboardAccount = $0 }, signIn: { signInRequests += 1 })
    try require(clipboardAccount == nil && signInRequests == 2)
    try await session.signIn(challenge: "synthetic", credential: "123456")
    model.load(); try await finished(model)
    AccountFixture.failLogout = true
    model.logout(); try await finished(model)
    AccountFixture.failLogout = false
    try require(windowClosures == 2 && model.user == nil && storage.load() == nil && model.message != nil)
    model.code = "123456"; model.target = "synthetic@example.invalid"; model.close()
    try require(model.code.isEmpty && model.target.isEmpty && model.challenge == nil)
    print("PASS: native account model login, disabled provider, rename, failed deletion, logout and credential cleanup")
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

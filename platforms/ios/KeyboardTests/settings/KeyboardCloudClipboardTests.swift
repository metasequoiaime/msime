import UIKit
import XCTest

private final class ClipboardAccountStorage: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var saved: BackendSavedSession?
  init(_ saved: BackendSavedSession) { self.saved = saved }
  func load() throws -> BackendSavedSession? { lock.withLock { saved } }
  func save(_ value: BackendSavedSession) throws { lock.withLock { saved = value } }
  func clear() throws { lock.withLock { saved = nil } }
}

private struct ClipboardSharedRefreshLock: BackendRefreshLock {
  var sharedAcrossProcesses: Bool { true }
  func run<T: Sendable>(_ body: @Sendable () async throws -> T) async throws -> T { try await body() }
}

private final class ClipboardAccountProtocol: URLProtocol, @unchecked Sendable {
  static let oldToken = String(repeating: "a", count: 64)
  static let newToken = String(repeating: "b", count: 64)
  private static let lock = NSLock()
  private static var recorded: [String] = []
  private static var recordedPaths: [String] = []
  private static var onRejection: (() -> Void)?
  static var authorizations: [String] { lock.withLock { recorded } }
  static var paths: [String] { lock.withLock { recordedPaths } }
  static func reset(onRejection: @escaping () -> Void) {
    lock.withLock { recorded = []; recordedPaths = []; Self.onRejection = onRejection }
  }
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let authorization = request.value(forHTTPHeaderField: "Authorization") ?? ""
    let onRejection = Self.lock.withLock { () -> (() -> Void)? in
      Self.recorded.append(authorization)
      Self.recordedPaths.append(request.url!.path + (request.url!.query.map { "?" + $0 } ?? ""))
      defer { Self.onRejection = nil }
      return Self.onRejection
    }
    let rejected = authorization == "Bearer \(Self.oldToken)"
    if rejected { onRejection?() }
    let response = HTTPURLResponse(url: request.url!, statusCode: rejected ? 401 : 201,
                                   httpVersion: nil, headerFields: ["Content-Type":"application/json"])!
    let body = rejected ? #"{"error":{"code":"invalid_credentials"}}"# :
      "{\"id\":\"\(String(repeating: "c", count: 64))\",\"text\":\"合成剪贴板文本\",\"updated_at\":\"synthetic-time\"}"
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class MemoryClipboardKeystore: BackendClipboardKeystoreStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: String?
  var stored: String? { lock.withLock { value } }
  func load() throws -> String? { lock.withLock { value } }
  func save(_ value: String) throws { lock.withLock { self.value = value } }
  func clear() throws { lock.withLock { value = nil } }
}

/// 不加密的驱动替身：上传把服务端替身的明文正文当作结果，非 2xx 变成驱动错误。只用来检查令牌和账号绑定，不涉及加密。
private func passthroughClipboardE2E() -> BackendClipboardE2E {
  BackendClipboardE2E(drive: { data in
    let input = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    var value: [String: Any] = ["keystore": "synthetic-keystore"]
    if let start = input["start"] as? [String: Any] {
      let body = String(data: try! JSONSerialization.data(withJSONObject: ["text": start["text"] ?? ""]), encoding: .utf8)!
      value["request"] = ["method": "POST", "path": "/v1/users/me/clipboard", "body": body]
      value["state"] = ["op": start["op"] ?? ""]
    } else if let response = input["response"] as? [String: Any], let status = response["status"] as? Int,
              let body = response["body"] as? String {
      if (200..<300).contains(status) { value["done"] = (try? JSONSerialization.jsonObject(with: Data(body.utf8))) ?? [:] }
      else { value["error"] = ["status": status, "code": BackendAccountClient.errorCode(Data(body.utf8)) ?? "http_\(status)"] }
    }
    return try? JSONSerialization.data(withJSONObject: ["ok": true, "value": value])
  }, keystore: MemoryClipboardKeystore(), deviceName: { "合成设备" })
}

/// A stand-in for the account's cloud clipboard: no request leaves the test process.
private final class FakeCloudClipboard: KeyboardCloudClipboardService, @unchecked Sendable {
  private let lock = NSLock()
  private var signedInValue: Bool
  private var accountSessionID = UUID()
  private var enabledValue: Bool
  /// 设了时模拟本机等待批准：列表为空，带这个核对码。
  private var pendingCode: String?
  private var stored: [BackendAccountClient.ClipboardItem]
  private var failure: Error?
  private var gate: CheckedContinuation<Void, Never>?
  private var holds = false
  private var holdPageAfterAdd = false
  private var uploadLog: [String] = []
  private var pages = 0
  var uploads: [String] { lock.withLock { uploadLog } }
  var pageCount: Int { lock.withLock { pages } }

  init(signedIn: Bool = true, enabled: Bool = true, items: [String] = []) {
    signedInValue = signedIn
    enabledValue = enabled
    stored = items.enumerated().map { Self.item($0.element, index: $0.offset) }
  }
  static func item(_ text: String, index: Int) -> BackendAccountClient.ClipboardItem {
    let id = String(repeating: String(index % 10), count: 64)
    let data = try! JSONSerialization.data(withJSONObject: ["id": id, "text": text, "updated_at": "2026-09-30T08:00:00Z"])
    return try! JSONDecoder().decode(BackendAccountClient.ClipboardItem.self, from: data)
  }
  func fail(with error: Error?) { lock.withLock { failure = error } }
  func setEnabled(_ enabled: Bool) { lock.withLock { enabledValue = enabled } }
  func setPending(_ code: String?) { lock.withLock { pendingCode = code } }
  /// Hold the next page request until `release()`, to model a reply that arrives late.
  func holdNextPage() { lock.withLock { holds = true } }
  func release() { lock.withLock { () -> CheckedContinuation<Void, Never>? in defer { gate = nil }; return gate }?.resume() }
  var isHolding: Bool { lock.withLock { gate != nil } }
  func holdPageAfterNextAdd() { lock.withLock { holdPageAfterAdd = true } }
  func replace(_ texts: [String]) { lock.withLock { stored = texts.enumerated().map { Self.item($0.element, index: $0.offset + 20) } } }
  func switchAccount(items: [String]) {
    lock.withLock {
      accountSessionID = UUID()
      stored = items.enumerated().map { Self.item($0.element, index: $0.offset + 40) }
    }
  }

  func binding() async throws -> KeyboardCloudClipboardBinding? {
    lock.withLock { signedInValue ? .init(userID: "synthetic-user", sessionID: accountSessionID) : nil }
  }
  func matches(_ binding: KeyboardCloudClipboardBinding) async -> Bool {
    lock.withLock { signedInValue && binding.userID == "synthetic-user" && binding.sessionID == accountSessionID }
  }
  func page(matching binding: KeyboardCloudClipboardBinding) async throws -> BackendAccountClient.ClipboardPage {
    let shouldHold = lock.withLock { () -> Bool in
      pages += 1
      defer { holds = false }
      return holds
    }
    let snapshot = lock.withLock { () -> (Error?, Bool, String?, [[String: Any]]) in
      (self.failure, enabledValue, pendingCode, stored.map { ["id": $0.id, "text": $0.text, "updated_at": $0.updated_at] })
    }
    if shouldHold { await withCheckedContinuation { continuation in lock.withLock { gate = continuation } } }
    let (failure, enabled, pending, rows) = snapshot
    if let failure { throw failure }
    let e2e: [String: Any] = ["status": pending == nil ? "ready" : "pending", "code": pending ?? "0000 0000 0000 0000",
                              "pending_devices": 0, "undecryptable": 0]
    let data = try JSONSerialization.data(withJSONObject: ["enabled": enabled, "items": pending == nil ? rows : [], "e2e": e2e])
    return try JSONDecoder().decode(BackendAccountClient.ClipboardPage.self, from: data)
  }
  func add(_ text: String, matching binding: KeyboardCloudClipboardBinding) async throws {
    try lock.withLock {
      guard signedInValue && binding.userID == "synthetic-user" && binding.sessionID == accountSessionID else {
        throw CancellationError()
      }
      if let failure { throw failure }
      uploadLog.append(text)
      stored.insert(Self.item(text, index: stored.count + 1), at: 0)
      if holdPageAfterAdd {
        holds = true
        holdPageAfterAdd = false
      }
    }
  }
}

@MainActor
final class KeyboardCloudClipboardTests: XCTestCase {
  func testNativeServiceBindsEveryDriverEntryToItsLogin() async throws {
    for operation in ["page", "add", "upload"] {
      let tokens = BackendAccountClient.Tokens(access_token: ClipboardAccountProtocol.oldToken,
        refresh_token: String(repeating: "f", count: 64), token_type: "Bearer", expires_in: 900,
        user: .init(id: "synthetic-user", display_name: "合成用户", created_at: "2026-10-16"))
      let storage = ClipboardAccountStorage(try BackendSavedSession.forTokens(tokens))
      let session = BackendAccountSession(storage: storage, refreshLock: ClipboardSharedRefreshLock())
      let keystore = MemoryClipboardKeystore()
      let e2e = BackendClipboardE2E(drive: passthroughClipboardE2E().drive, keystore: keystore, deviceName: {
        // 名称查询是驱动开始前的挂起点，模拟另一端恰好退出登录。
        try? await session.forget()
        return "合成设备"
      })
      let configuration = URLSessionConfiguration.ephemeral
      configuration.protocolClasses = [ClipboardAccountProtocol.self]
      ClipboardAccountProtocol.reset(onRejection: {})
      let service = BackendKeyboardCloudClipboardService(session: session,
        client: BackendAccountClient(configuration: configuration), e2e: e2e)
      let captured = try await service.binding()
      let binding = try XCTUnwrap(captured)
      do {
        switch operation {
        case "page": _ = try await service.page(matching: binding)
        case "add": try await service.add("合成文本", matching: binding)
        default: _ = try await service.uploadIfEnabled("合成文本", matching: binding)
        }
        XCTFail("旧登录应取消")
      } catch is CancellationError { }
      XCTAssertNil(keystore.stored, operation)
      XCTAssertTrue(ClipboardAccountProtocol.paths.isEmpty, operation)
    }
  }

  func testUploadNeverUsesTheReplacementAccountsToken() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ClipboardAccountProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let original = BackendAccountClient.Tokens(access_token: ClipboardAccountProtocol.oldToken,
      refresh_token: String(repeating: "d", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-user", display_name: "示例", created_at: "2026-09-08"))
    let replacement = BackendAccountClient.Tokens(access_token: ClipboardAccountProtocol.newToken,
      refresh_token: String(repeating: "e", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "other-synthetic-user", display_name: "另一个账号", created_at: "2026-09-08"))
    let storage = ClipboardAccountStorage(try BackendSavedSession.forTokens(original))
    let session = BackendAccountSession(api: client, storage: storage, refreshLock: ClipboardSharedRefreshLock())
    let service = BackendKeyboardCloudClipboardService(session: session, client: client, e2e: passthroughClipboardE2E())
    let captured = try await service.binding()
    let binding = try XCTUnwrap(captured)
    try storage.save(BackendSavedSession.forTokens(replacement))
    ClipboardAccountProtocol.reset(onRejection: {})

    do {
      try await service.add("合成剪贴板文本", matching: binding)
      XCTFail("the old panel must not upload through the replacement login")
    } catch is CancellationError { }
    XCTAssertTrue(ClipboardAccountProtocol.authorizations.isEmpty)
  }

  func testRejectedUploadDoesNotRetryAsAnotherAccount() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ClipboardAccountProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let original = BackendAccountClient.Tokens(access_token: ClipboardAccountProtocol.oldToken,
      refresh_token: String(repeating: "d", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-user", display_name: "示例", created_at: "2026-09-08"))
    let storage = ClipboardAccountStorage(try BackendSavedSession.forTokens(original))
    let session = BackendAccountSession(api: client, storage: storage,
                                        refreshLock: BackendProcessRefreshLock())
    let replacement = BackendAccountClient.Tokens(access_token: ClipboardAccountProtocol.newToken,
      refresh_token: String(repeating: "e", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "other-synthetic-user", display_name: "另一个账号", created_at: "2026-09-08"))
    ClipboardAccountProtocol.reset(onRejection: {
      try? storage.save(try BackendSavedSession.forTokens(replacement))
    })
    let service = BackendKeyboardCloudClipboardService(session: session, client: client, e2e: passthroughClipboardE2E())

    do {
      let captured = try await service.binding()
      let binding = try XCTUnwrap(captured)
      try await service.add("合成剪贴板文本", matching: binding)
      XCTFail("old account clipboard text must not be retried as the new account")
    } catch is CancellationError { }
    XCTAssertEqual(ClipboardAccountProtocol.authorizations,
                   ["Bearer \(ClipboardAccountProtocol.oldToken)"])
  }

  private func temporaryStore() throws -> ClipboardHistoryStore {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    return ClipboardHistoryStore(directory: directory)
  }

  private func settle(_ cloud: KeyboardCloudClipboard, until condition: @escaping () -> Bool) async {
    for _ in 0..<200 where !condition() { try? await Task.sleep(nanoseconds: 5_000_000) }
  }

  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }

  private func control<T: UIView>(_ identifier: String, in root: UIView, as type: T.Type = UIControl.self) throws -> T {
    try XCTUnwrap(descendants(root).first { $0.accessibilityIdentifier == identifier } as? T, identifier)
  }

  func testPanelFetchesOnOpenAndInsertsTheTappedCloudItem() async throws {
    let service = FakeCloudClipboard(items: ["合成云端文本一", "synthetic cloud two"])
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    var inserted: [String] = []
    let panel = KeyboardClipboardView(hasFullAccess: true, store: try temporaryStore(), cloud: cloud,
                                      onInsert: { inserted.append($0) }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 320, height: 260)
    panel.layoutIfNeeded()
    await settle(cloud) { !cloud.items.isEmpty }
    XCTAssertEqual(service.pageCount, 1, "opening the panel fetches once")
    XCTAssertTrue(cloud.signedIn)

    let source = try control("clipboardSource", in: panel, as: UISegmentedControl.self)
    XCTAssertFalse(source.isHidden)
    XCTAssertEqual(source.titleForSegment(at: 1), "云端")
    source.selectedSegmentIndex = 1
    source.sendActions(for: .valueChanged)
    panel.layoutIfNeeded()
    XCTAssertTrue(panel.showsCloud)
    XCTAssertTrue(try control("captureClipboard", in: panel).isHidden)
    XCTAssertTrue(try control("clipboardSearch", in: panel).isHidden)
    XCTAssertFalse(try control("refreshCloudClipboard", in: panel).isHidden)
    let table = try XCTUnwrap(panel.subviews.compactMap { $0 as? UITableView }.first)
    XCTAssertEqual(table.numberOfRows(inSection: 0), 2)
    panel.tableView(table, didSelectRowAt: IndexPath(row: 1, section: 0))
    await settle(cloud) { inserted.count == 1 }
    XCTAssertEqual(inserted, ["synthetic cloud two"])

    try control("refreshCloudClipboard", in: panel).sendActions(for: .primaryActionTriggered)
    await settle(cloud) { service.pageCount == 2 && !cloud.items.isEmpty }
    XCTAssertEqual(service.pageCount, 2, "refresh is the only other fetch")

    source.selectedSegmentIndex = 0
    source.sendActions(for: .valueChanged)
    XCTAssertFalse(panel.showsCloud)
    XCTAssertFalse(try control("captureClipboard", in: panel).isHidden)
  }

  func testAccountSwitchCannotInsertACachedCloudRow() async throws {
    let service = FakeCloudClipboard(items: ["账号 A 的合成文本"])
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    var inserted: [String] = []
    let panel = KeyboardClipboardView(hasFullAccess: true, store: try temporaryStore(), cloud: cloud,
                                      onInsert: { inserted.append($0) }, onClose: {})
    await settle(cloud) { cloud.items.count == 1 }
    let source = try control("clipboardSource", in: panel, as: UISegmentedControl.self)
    source.selectedSegmentIndex = 1
    source.sendActions(for: .valueChanged)
    let table = try XCTUnwrap(panel.subviews.compactMap { $0 as? UITableView }.first)
    service.switchAccount(items: ["账号 B 的合成文本"])
    panel.tableView(table, didSelectRowAt: IndexPath(row: 0, section: 0))
    await settle(cloud) { cloud.items.first?.text == "账号 B 的合成文本" }
    XCTAssertTrue(inserted.isEmpty, "a cached row from the previous account must not enter the editor")
  }

  func testAccountSwitchCannotUploadFromAnOldPanel() async {
    let service = FakeCloudClipboard(items: [])
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    cloud.refresh()
    await settle(cloud) { cloud.canUpload }
    service.switchAccount(items: [])
    cloud.upload("账号 A 面板中的合成文本")
    await settle(cloud) { !cloud.uploading && service.pageCount >= 2 }
    XCTAssertTrue(service.uploads.isEmpty, "an old panel must not send to the new account")
  }

  func testAccountSwitchDropsALatePageFromTheOldAccount() async {
    let service = FakeCloudClipboard(items: ["账号 A 的迟到文本"])
    service.holdNextPage()
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    cloud.refresh()
    await settle(cloud) { service.isHolding }
    service.switchAccount(items: ["账号 B 的文本"])
    service.release()
    await settle(cloud) { cloud.items.first?.text == "账号 B 的文本" }
    XCTAssertEqual(cloud.items.map(\.text), ["账号 B 的文本"])
  }

  func testSignedOutDisabledAndNoFullAccessStates() async throws {
    let signedOut = KeyboardCloudClipboard(hasFullAccess: true, service: FakeCloudClipboard(signedIn: false, items: ["x"]))
    signedOut.refresh()
    await settle(signedOut) { signedOut.message == KeyboardCloudClipboard.signedOutMessage }
    XCTAssertEqual(signedOut.message, "登录水杉账号后可在设备间同步剪贴板")
    XCTAssertFalse(signedOut.canUpload)
    XCTAssertTrue(signedOut.items.isEmpty)

    let disabled = KeyboardCloudClipboard(hasFullAccess: true, service: FakeCloudClipboard(enabled: false, items: ["隐藏"]))
    disabled.refresh()
    await settle(disabled) { disabled.isDisabled }
    XCTAssertEqual(disabled.message, "云剪贴板未开启")
    XCTAssertTrue(disabled.items.isEmpty, "a disabled cloud clipboard lists nothing")
    XCTAssertFalse(disabled.canUpload)

    let service = FakeCloudClipboard(items: ["不应读取"])
    let locked = KeyboardCloudClipboard(hasFullAccess: false, service: service)
    locked.refresh()
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertEqual(service.pageCount, 0, "no request without full access")
    XCTAssertEqual(locked.message, KeyboardCloudClipboard.needsFullAccessMessage)

    let failing = FakeCloudClipboard()
    failing.fail(with: BackendAccountClient.Failure(status: 503))
    let failed = KeyboardCloudClipboard(hasFullAccess: true, service: failing)
    failed.refresh()
    await settle(failed) { failed.message == BackendAccountClient.Failure(status: 503).localizedDescription }
    XCTAssertEqual(failed.message, "此服务暂不可用，请稍后再试。")
    XCTAssertFalse(failed.canUpload, "a failed cloud fetch must not enable uploads")

    let expired = FakeCloudClipboard()
    expired.fail(with: BackendAccountClient.Failure(status: 401))
    let rejected = KeyboardCloudClipboard(hasFullAccess: true, service: expired)
    rejected.refresh()
    await settle(rejected) { rejected.message == KeyboardCloudClipboard.signedOutMessage }
    XCTAssertFalse(rejected.signedIn, "a rejected session is treated as signed out")
  }

  /// 本机等待批准时只显示一行提示（带核对码），不显示条目，也不能上传。
  func testPendingDeviceShowsItsCodeAndUploadsNothing() async throws {
    let service = FakeCloudClipboard(items: ["不应显示"])
    service.setPending("0421 9873 5530 1176")
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    cloud.refresh()
    await settle(cloud) { cloud.signedIn }
    XCTAssertEqual(cloud.message, KeyboardCloudClipboard.pendingMessage("0421 9873 5530 1176"))
    XCTAssertTrue(cloud.message.contains("0421 9873 5530 1176"))
    XCTAssertTrue(cloud.items.isEmpty)
    XCTAssertFalse(cloud.canUpload)
    cloud.upload("不应上传")
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertTrue(service.uploads.isEmpty)
    // 另一台设备批准之后，刷新即可读取和上传。
    service.setPending(nil)
    cloud.refresh()
    await settle(cloud) { !cloud.items.isEmpty }
    XCTAssertEqual(cloud.items.map(\.text), ["不应显示"])
    XCTAssertTrue(cloud.canUpload)
  }

  /// 真实的 Rust 驱动经 C ABI 接上：第一步是带本机设备编号取密钥状态，请求带账号凭据，新生成的密钥串先保存；服务端的 401 交回驱动后变成带状态码的错误，调用方据此刷新令牌。
  func testRustDriverIsLinkedAndStartsWithAKeyRequest() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ClipboardAccountProtocol.self]
    ClipboardAccountProtocol.reset(onRejection: {})
    let client = BackendAccountClient(configuration: configuration)
    let keystore = MemoryClipboardKeystore()
    let e2e = BackendClipboardE2E(drive: IOSCloudClipboardDriver.e2e.drive, keystore: keystore, deviceName: { "合成设备" })
    do {
      _ = try await client.clipboard(token: ClipboardAccountProtocol.oldToken, e2e: e2e)
      XCTFail("a rejected token must fail")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 401)
      XCTAssertEqual(failure.code, "invalid_credentials")
    }
    XCTAssertEqual(ClipboardAccountProtocol.authorizations, ["Bearer \(ClipboardAccountProtocol.oldToken)"])
    let path = try XCTUnwrap(ClipboardAccountProtocol.paths.first)
    XCTAssertTrue(path.hasPrefix("/v1/users/me/clipboard/keys?device_id="), path)
    XCTAssertEqual(path.count, "/v1/users/me/clipboard/keys?device_id=".count + 32)
    XCTAssertNotNil(keystore.stored, "the new device identity is saved before the first request")
  }

  /// 密钥串与账号会话在同一个钥匙串服务名和访问组下，退出登录清除会话时一起删除。
  func testClipboardKeystoreIsClearedWithTheAccountSession() throws {
    let service = "app.msime.backend.account.test-\(UUID().uuidString)"
    let keystore = BackendKeychainClipboardKeystore(service: service)
    let keychain = BackendKeychain(service: service)
    do { try keystore.save("v1.synthetic-keystore") }
    catch { throw XCTSkip("the test host has no App Group keychain entitlement in this build") }
    defer { try? keystore.clear() }
    XCTAssertEqual(try keystore.load(), "v1.synthetic-keystore")
    try keystore.save("v1.replaced")
    XCTAssertEqual(try keystore.load(), "v1.replaced")
    XCTAssertThrowsError(try keystore.save("not a keystore"))
    try keychain.clear()
    XCTAssertNil(try keystore.load())
  }

  func testResultsArrivingAfterThePanelClosedAreDropped() async throws {
    let service = FakeCloudClipboard(items: ["迟到的合成内容"])
    service.holdNextPage()
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    let panel = KeyboardClipboardView(hasFullAccess: true, store: try temporaryStore(), cloud: cloud,
                                      onInsert: { _ in XCTFail("nothing is inserted") }, onClose: {})
    let host = UIView()
    host.addSubview(panel)
    await settle(cloud) { service.isHolding }
    XCTAssertTrue(service.isHolding)
    panel.removeFromSuperview()
    service.release()
    try await Task.sleep(nanoseconds: 100_000_000)
    XCTAssertTrue(cloud.items.isEmpty)
    XCTAssertEqual(cloud.message, "正在读取云端内容…", "the late reply never reached the closed panel")
    cloud.refresh()
    XCTAssertEqual(service.pageCount, 1, "a closed panel does not fetch again")
  }

  func testUploadRequiresSignInAndTheServerSwitch() async throws {
    let service = FakeCloudClipboard(items: [])
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    XCTAssertFalse(cloud.canUpload, "unknown until the panel has looked at the account")
    cloud.upload("太早")
    XCTAssertTrue(service.uploads.isEmpty)
    cloud.refresh()
    await settle(cloud) { cloud.signedIn }
    XCTAssertTrue(cloud.canUpload)
    cloud.upload("合成的本机记录")
    await settle(cloud) { cloud.notice == "已发到云剪贴板" }
    XCTAssertEqual(service.uploads, ["合成的本机记录"])
    XCTAssertEqual(cloud.items.first?.text, "合成的本机记录", "the list is refreshed after the upload")

    cloud.upload(String(repeating: "长", count: 4001))
    XCTAssertEqual(service.uploads.count, 1)
    XCTAssertEqual(cloud.notice, "这条记录超过 4000 字，不能发到云剪贴板。")

    // The switch was turned off on another device after the panel opened: the upload reads it first and sends nothing.
    service.setEnabled(false)
    cloud.upload("不应上传")
    await settle(cloud) { cloud.notice == KeyboardCloudClipboard.disabledMessage }
    XCTAssertEqual(service.uploads, ["合成的本机记录"])
    XCTAssertTrue(cloud.isDisabled)
    XCTAssertFalse(cloud.canUpload)
  }

  func testUploadCannotOverwriteARefreshThatStartedWhileItsReplyWasPending() async throws {
    let service = FakeCloudClipboard(items: ["旧列表"])
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    cloud.refresh()
    await settle(cloud) { cloud.signedIn }

    service.holdPageAfterNextAdd()
    cloud.upload("上传中的记录")
    await settle(cloud) { service.isHolding }
    XCTAssertTrue(service.isHolding)

    service.replace(["刷新后的列表"])
    cloud.refresh()
    await settle(cloud) { cloud.items.first?.text == "刷新后的列表" }
    XCTAssertEqual(cloud.items.map(\.text), ["刷新后的列表"])

    service.release()
    await settle(cloud) { cloud.notice == "已发到云剪贴板" }
    XCTAssertEqual(cloud.items.map(\.text), ["刷新后的列表"], "旧上传回复不能覆盖更新的刷新结果")
  }

  func testLocalRowMenuOffersSendToCloudOnlyWhenSignedIn() async throws {
    let store = try temporaryStore()
    try store.add("合成历史")
    let service = FakeCloudClipboard(signedIn: false)
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    let panel = KeyboardClipboardView(hasFullAccess: true, store: store, cloud: cloud, onInsert: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 300)
    panel.layoutIfNeeded()
    await settle(cloud) { cloud.message == KeyboardCloudClipboard.signedOutMessage }
    let table = try XCTUnwrap(panel.subviews.compactMap { $0 as? UITableView }.first)
    let action = try sendAction(panel.tableView(table, cellForRowAt: IndexPath(row: 0, section: 0)))
    XCTAssertTrue(action.attributes.contains(.disabled))

    let signedIn = FakeCloudClipboard()
    let onlineCloud = KeyboardCloudClipboard(hasFullAccess: true, service: signedIn)
    let online = KeyboardClipboardView(hasFullAccess: true, store: store, cloud: onlineCloud, onInsert: { _ in }, onClose: {})
    await settle(onlineCloud) { onlineCloud.signedIn }
    let onlineTable = try XCTUnwrap(online.subviews.compactMap { $0 as? UITableView }.first)
    let enabled = try sendAction(online.tableView(onlineTable, cellForRowAt: IndexPath(row: 0, section: 0)))
    XCTAssertFalse(enabled.attributes.contains(.disabled))

    let local = KeyboardClipboardView(hasFullAccess: true, store: store, onInsert: { _ in }, onClose: {})
    let localTable = try XCTUnwrap(local.subviews.compactMap { $0 as? UITableView }.first)
    let menu = try XCTUnwrap(local.tableView(localTable, cellForRowAt: IndexPath(row: 0, section: 0)).accessoryView as? UIButton)?.menu
    XCTAssertFalse(menu?.children.contains { ($0 as? UIAction)?.title == "发到云剪贴板" } ?? true, "no cloud, no upload entry")
    XCTAssertTrue(try control("clipboardSource", in: local, as: UISegmentedControl.self).isHidden)
  }

  func testCloudItemsAreWithheldOnceTheFieldTurnsIntoACredentialField() async throws {
    let service = FakeCloudClipboard(items: ["合成云端"])
    let cloud = KeyboardCloudClipboard(hasFullAccess: true, service: service)
    var credential = false
    cloud.fieldAllowsCloud = { !credential }
    let panel = KeyboardClipboardView(hasFullAccess: true, store: try temporaryStore(), cloud: cloud,
                                      onInsert: { _ in XCTFail("nothing goes into a credential field") }, onClose: {})
    await settle(cloud) { !cloud.items.isEmpty }
    let source = try control("clipboardSource", in: panel, as: UISegmentedControl.self)
    source.selectedSegmentIndex = 1
    source.sendActions(for: .valueChanged)
    let table = try XCTUnwrap(panel.subviews.compactMap { $0 as? UITableView }.first)
    XCTAssertEqual(table.numberOfRows(inSection: 0), 1)
    credential = true
    table.reloadData()
    XCTAssertEqual(table.numberOfRows(inSection: 0), 0)
    panel.tableView(table, didSelectRowAt: IndexPath(row: 0, section: 0))
  }

  func testCredentialFieldPolicyMatchesKeyCounting() {
    XCTAssertTrue(KeyboardViewController.isCredentialField(secure: true, contentType: nil))
    XCTAssertTrue(KeyboardViewController.isCredentialField(secure: false, contentType: .password))
    XCTAssertTrue(KeyboardViewController.isCredentialField(secure: nil, contentType: .oneTimeCode))
    XCTAssertTrue(KeyboardViewController.isCredentialField(secure: false, contentType: .newPassword))
    XCTAssertFalse(KeyboardViewController.isCredentialField(secure: false, contentType: .emailAddress))
    XCTAssertFalse(KeyboardViewController.isCredentialField(secure: nil, contentType: nil))
  }

  /// The keyboard can only read the signed-in session if the app stores it in the App Group's keychain access group, which both already hold as an entitlement.
  func testAccountSessionUsesTheAppGroupKeychainGroupOnIOS() throws {
    XCTAssertEqual(BackendKeychain.defaultAccessGroup, "group.app.msime.ios")
    let keychain = BackendKeychain(service: "app.msime.backend.account.test-\(UUID().uuidString)")
    let tokens = BackendAccountClient.Tokens(access_token: String(repeating: "a", count: 64),
      refresh_token: String(repeating: "f", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-user", display_name: "测试", created_at: "2026-09-08"))
    let session = try BackendSavedSession.forTokens(tokens)
    do { try keychain.save(session) }
    catch { throw XCTSkip("the test host has no App Group keychain entitlement in this build") }
    defer { try? keychain.clear() }
    XCTAssertEqual(try keychain.load()?.tokens.user.id, "synthetic-user")
    try keychain.clear()
    XCTAssertNil(try keychain.load())
  }

  private func sendAction(_ cell: UITableViewCell) throws -> UIAction {
    let menu = try XCTUnwrap((cell.accessoryView as? UIButton)?.menu)
    return try XCTUnwrap(menu.children.compactMap { $0 as? UIAction }.first { $0.title == "发到云剪贴板" })
  }
}

/// 「是否记录」的统一判断（`KeyboardPrivacyGate`），对应 Android 的 `ImePrivacyGate`。
@MainActor
final class KeyboardPrivacyGateTests: XCTestCase {
  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }

  /// 隐私模式或凭据输入框里，每一类记录都不允许；两者都不是时都允许。
  func testEveryRecordIsSuppressedInPrivacyModeAndInACredentialField() {
    for incognito in [false, true] {
      for credential in [false, true] {
        let gate = KeyboardPrivacyGate(incognito: incognito, credentialField: credential)
        XCTAssertEqual(gate.suppressed, incognito || credential, "\(incognito) \(credential)")
        for record in KeyboardPrivacyGate.Record.allCases {
          XCTAssertEqual(gate.allows(record), !(incognito || credential), "\(record) \(incognito) \(credential)")
        }
      }
    }
  }

  /// 隐私会话与 Android 的 `learningSuppressed` 同口径：只看隐私模式，凭据输入框不算。
  func testOnlyPrivacyModeMarksThePrivateSession() {
    XCTAssertTrue(KeyboardPrivacyGate(incognito: true, credentialField: false).privateSession)
    XCTAssertTrue(KeyboardPrivacyGate(incognito: true, credentialField: true).privateSession)
    XCTAssertFalse(KeyboardPrivacyGate(incognito: false, credentialField: true).privateSession)
    XCTAssertFalse(KeyboardPrivacyGate(incognito: false, credentialField: false).privateSession)
  }

  /// 闸门不许保存时，点「保存当前剪贴板」不写历史，只说明原因，与 Android 的 `captureClipboard(true)` 一样。
  func testCaptureSavesNothingWhileTheGateSaysNo() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    let store = ClipboardHistoryStore(directory: directory)
    let panel = KeyboardClipboardView(hasFullAccess: true, store: store, capturesHistory: { false },
                                      onInsert: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 320, height: 260)
    panel.layoutIfNeeded()
    let capture = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "captureClipboard" } as? UIButton)
    XCTAssertEqual(capture.accessibilityLabel, "保存当前剪贴板", "no new-copy prompt for a copy that cannot be saved")
    capture.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try store.load().isEmpty)
    XCTAssertTrue(descendants(panel).contains { ($0 as? UILabel)?.text == KeyboardClipboardView.privacyMessage })
  }
}

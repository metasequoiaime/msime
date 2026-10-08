import UIKit
import XCTest

/// A stand-in for the account's cloud clipboard: no request leaves the test process.
private final class FakeCloudClipboard: KeyboardCloudClipboardService, @unchecked Sendable {
  private let lock = NSLock()
  private var signedInValue: Bool
  private var enabledValue: Bool
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
  /// Hold the next page request until `release()`, to model a reply that arrives late.
  func holdNextPage() { lock.withLock { holds = true } }
  func release() { lock.withLock { () -> CheckedContinuation<Void, Never>? in defer { gate = nil }; return gate }?.resume() }
  var isHolding: Bool { lock.withLock { gate != nil } }
  func holdPageAfterNextAdd() { lock.withLock { holdPageAfterAdd = true } }
  func replace(_ texts: [String]) { lock.withLock { stored = texts.enumerated().map { Self.item($0.element, index: $0.offset + 20) } } }

  func isSignedIn() async -> Bool { lock.withLock { signedInValue } }
  func page() async throws -> BackendAccountClient.ClipboardPage {
    let shouldHold = lock.withLock { () -> Bool in
      pages += 1
      defer { holds = false }
      return holds
    }
    let snapshot = lock.withLock { () -> (Error?, Bool, [[String: Any]]) in
      (self.failure, enabledValue, stored.map { ["id": $0.id, "text": $0.text, "updated_at": $0.updated_at] })
    }
    if shouldHold { await withCheckedContinuation { continuation in lock.withLock { gate = continuation } } }
    let (failure, enabled, rows) = snapshot
    if let failure { throw failure }
    let data = try JSONSerialization.data(withJSONObject: ["enabled": enabled, "items": rows])
    return try JSONDecoder().decode(BackendAccountClient.ClipboardPage.self, from: data)
  }
  func add(_ text: String) async throws {
    try lock.withLock {
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
    XCTAssertEqual(inserted, ["synthetic cloud two"])

    try control("refreshCloudClipboard", in: panel).sendActions(for: .primaryActionTriggered)
    await settle(cloud) { service.pageCount == 2 && !cloud.items.isEmpty }
    XCTAssertEqual(service.pageCount, 2, "refresh is the only other fetch")

    source.selectedSegmentIndex = 0
    source.sendActions(for: .valueChanged)
    XCTAssertFalse(panel.showsCloud)
    XCTAssertFalse(try control("captureClipboard", in: panel).isHidden)
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

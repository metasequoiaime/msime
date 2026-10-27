import Foundation
import XCTest

/// iOS 的词库同步（`docs/dictionary-webdav-sync.md`）：偏好怎么读、说法、设备去重、键盘什么时候自己记录，以及对着真实 Rust（`msime_client_dictionary_sync`）的请求形状、本机目录和错误码。
///
/// 末尾两项要一个本机的 WebDAV 服务端，地址由环境变量 `MSIME_TEST_WEBDAV_URL`（xcodebuild 下写成 `TEST_RUNNER_MSIME_TEST_WEBDAV_URL`）给，账号和应用密码是 `MSIME_TEST_WEBDAV_USERNAME`、`MSIME_TEST_WEBDAV_PASSWORD`；没有时跳过。每次在服务端上用一个新的子目录，测完不清理服务端。
final class DictionarySyncTests: XCTestCase {
  private var directory: URL!

  override func setUpWithError() throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent("dictionary-sync-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: directory)
  }

  private static let password = "synthetic-app-password"

  // MARK: - 偏好

  func testConfigurationReadsTheDictionaryKeyOnly() throws {
    XCTAssertNil(IOSDictionarySyncPolicy.Configuration(preferences: nil))
    XCTAssertNil(IOSDictionarySyncPolicy.Configuration(preferences: [:]))
    // 云剪贴板的网盘是另一组，不算词库同步。
    XCTAssertNil(IOSDictionarySyncPolicy.Configuration(preferences: [
      "cloud_clipboard_webdav": ["url": "https://dav.example.com/dav/", "username": "someone@example.com", "password": Self.password],
    ]))
    XCTAssertNil(IOSDictionarySyncPolicy.Configuration(preferences: ["dictionary_sync_webdav": ["url": "", "username": "", "password": ""]]))
    let configuration = try XCTUnwrap(IOSDictionarySyncPolicy.Configuration(preferences: [
      "dictionary_sync_webdav": ["url": "https://dav.example.com/dav/", "username": "someone@example.com", "password": Self.password],
    ]))
    XCTAssertEqual(configuration.url, "https://dav.example.com/dav/")
    XCTAssertEqual(configuration.username, "someone@example.com")
    XCTAssertTrue(configuration.hasPassword)
    // 设备身份绑定地址和账号，换应用密码不换；绑定里没有应用密码。
    let rotated = try XCTUnwrap(IOSDictionarySyncPolicy.Configuration(preferences: [
      "dictionary_sync_webdav": ["url": "https://dav.example.com/dav/", "username": "someone@example.com", "password": "another-password"],
    ]))
    XCTAssertEqual(configuration.binding, rotated.binding)
    XCTAssertFalse(configuration.binding.contains(Self.password))
  }

  /// 词库同步的偏好由 Rust 校验和保存，与云剪贴板的网盘各配各的，可以同时配置。
  func testPreferenceIsValidatedBySharedLayer() throws {
    let state = directory.appendingPathComponent("state", isDirectory: true)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences["cloud_clipboard_webdav"] = ["url": "https://dav.example.com/dav/", "username": "someone@example.com", "password": Self.password]
      preferences[IOSDictionarySyncPolicy.preference] = ["url": "https://dav.example.com/dav/", "username": "someone@example.com", "password": Self.password]
    })
    XCTAssertNotNil(IOSDictionarySyncPolicy.Configuration(preferences: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)))
    // 账号带冒号的值存不进去。
    XCTAssertFalse(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences[IOSDictionarySyncPolicy.preference] = ["url": "https://dav.example.com/dav/", "username": "a:b", "password": Self.password]
    })
  }

  // MARK: - 键盘扩展

  func testKeyboardRecordsOnlyAfterTheAppHasJoinedAndWithinTheMemoryBudget() {
    let limit = IOSDictionarySyncPolicy.keyboardStateLimit
    XCTAssertTrue(IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: true, stateBytes: 4096, keystorePresent: true))
    XCTAssertTrue(IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: true, stateBytes: limit, keystorePresent: true))
    XCTAssertFalse(IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: true, stateBytes: limit + 1, keystorePresent: true))
    XCTAssertFalse(IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: false, stateBytes: 4096, keystorePresent: true))
    // 还没有密钥串（App 还没登记过）时键盘不调，从不由它生成设备身份。
    XCTAssertFalse(IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: true, stateBytes: 4096, keystorePresent: false))
    XCTAssertFalse(IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: true, stateBytes: nil, keystorePresent: true))
    XCTAssertFalse(IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: true, stateBytes: 0, keystorePresent: true))
  }

  func testKeyboardChecksTheFilesTheAppLeft() throws {
    XCTAssertFalse(KeyboardDictionarySync.mayRecord(directory: directory))
    try Data("{}".utf8).write(to: directory.appendingPathComponent(IOSDictionarySync.stateFile))
    XCTAssertFalse(KeyboardDictionarySync.mayRecord(directory: directory), "no key ring yet")
    try Data("{}".utf8).write(to: directory.appendingPathComponent(IOSDictionarySync.keystoreFile))
    XCTAssertTrue(KeyboardDictionarySync.mayRecord(directory: directory))
    try Data(count: IOSDictionarySyncPolicy.keyboardStateLimit + 1).write(to: directory.appendingPathComponent(IOSDictionarySync.stateFile))
    XCTAssertFalse(KeyboardDictionarySync.mayRecord(directory: directory), "too large for the extension")
  }

  // MARK: - 设备和说法

  func testDevicesAreListedOncePerDevice() throws {
    let json = """
    {"enabled":true,"status":"ready","device_id":"\(String(repeating: "a", count: 32))","code":"123 456","devices":[
      {"device_id":"\(String(repeating: "a", count: 32))","name":"本机","created_at":"2026-10-27T00:00:00Z","approved":true,"current":true,"code":"123 456"},
      {"device_id":"\(String(repeating: "b", count: 32))","name":"另一台","created_at":"2026-10-27T00:00:00Z","approved":false,"current":false,"code":"654 321"},
      {"device_id":"\(String(repeating: "b", count: 32))","name":"另一台","created_at":"2026-10-27T00:00:01Z","approved":false,"current":false,"code":"654 321"}
    ]}
    """
    let devices = try JSONDecoder().decode(BackendAccountClient.ClipboardDevices.self, from: Data(json.utf8))
    let unique = IOSDictionarySyncPolicy.uniqueDevices(devices.devices)
    XCTAssertEqual(unique.map(\.device_id), [String(repeating: "a", count: 32), String(repeating: "b", count: 32)])
    XCTAssertEqual(IOSDictionarySyncPolicy.awaitingApproval(devices).map(\.device_id), [String(repeating: "b", count: 32)])
    // 同一台设备的几条登记合成一条：获准、本机任一条是就算是，名称取最后一条非空的。
    let repeated = [
      BackendAccountClient.ClipboardDevice(device_id: "c", name: "旧名", created_at: "", approved: false, current: false, code: "1"),
      BackendAccountClient.ClipboardDevice(device_id: "c", name: "", created_at: "", approved: true, current: false, code: "1"),
      BackendAccountClient.ClipboardDevice(device_id: "c", name: "新名", created_at: "", approved: false, current: true, code: "1"),
    ]
    let merged = try XCTUnwrap(IOSDictionarySyncPolicy.uniqueDevices(repeated).first)
    XCTAssertEqual(IOSDictionarySyncPolicy.uniqueDevices(repeated).count, 1)
    XCTAssertEqual(merged.name, "新名")
    XCTAssertTrue(merged.approved)
    XCTAssertTrue(merged.current)
  }

  /// 头文件列出的每个错误码都有自己的说法，不落到「暂不可用」。
  func testEveryDocumentedCodeHasItsOwnWording() {
    let fallback = IOSDictionarySyncPolicy.failureMessage("something_new")
    XCTAssertEqual(fallback, "同步失败，请稍后再试。")
    for code in ["dictionary_sync_not_configured", "invalid_dictionary_sync", "dictionary_sync_unauthorized",
                 "dictionary_sync_unavailable", "dictionary_sync_rate_limited", "dictionary_sync_too_large",
                 "dictionary_sync_rollback", "dictionary_sync_unsupported", "dictionary_sync_corrupt",
                 "dictionary_sync_storage", "clipboard_device_pending", "clipboard_key_conflict", "not_found"] {
      XCTAssertNotEqual(IOSDictionarySyncPolicy.failureMessage(code), fallback, code)
    }
    XCTAssertEqual(IOSDictionarySyncPolicy.probeMessage("dictionary_sync_unauthorized"), "网盘拒绝了这个账号或应用密码。")
    // 同一个码在测试连接和同步里说法不同。
    XCTAssertNotEqual(IOSDictionarySyncPolicy.probeMessage("dictionary_sync_unsupported"),
                      IOSDictionarySyncPolicy.failureMessage("dictionary_sync_unsupported"))
    XCTAssertEqual(IOSDictionarySyncPolicy.probeMessage("other"), "测试连接失败，请稍后再试。")
    XCTAssertEqual(IOSDictionarySyncPolicy.lastSuccess(nil), "尚未同步")
    XCTAssertTrue(IOSDictionarySyncPolicy.heldDeletionsMessage(60).contains("60"))
  }

  // MARK: - 本机目录

  /// 目录只有本应用能进、不进设备备份；保护类别在真机上是「首次解锁后」（模拟器不报告保护类别）。
  func testDirectoryIsPrivateAndExcludedFromBackup() throws {
    let target = directory.appendingPathComponent(IOSDictionarySync.directoryName, isDirectory: true)
    try IOSDictionarySync.prepare(target)
    let attributes = try FileManager.default.attributesOfItem(atPath: target.path)
    XCTAssertEqual((attributes[.posixPermissions] as? NSNumber)?.intValue, 0o700)
    if let protection = attributes[.protectionKey] as? FileProtectionType {
      XCTAssertEqual(protection, .completeUntilFirstUserAuthentication)
    }
    XCTAssertEqual(try target.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, true)
    XCTAssertTrue(IOSDictionarySync.directory.path.hasSuffix("/dictionary-sync"))
  }

  // MARK: - 对着真实 Rust

  private func preparedSync(state: URL, directory: URL, device: String) throws -> IOSDictionarySync {
    let options = try MetasequoiaInputSessionBridge.preparedHostOptions(stateRoot: state)
    return IOSDictionarySync(options: options, directory: directory, deviceName: device)
  }

  /// 没有配置时 `dictionary_sync_not_configured`；配置了而连不上时同步失败写在结果里（不抛出），状态照样给出；测试连接的草稿不合规或连不上时以对应的码抛出。
  func testRustAnswersWithDocumentedCodes() async throws {
    let state = directory.appendingPathComponent("state", isDirectory: true)
    let local = directory.appendingPathComponent(IOSDictionarySync.directoryName, isDirectory: true)
    let sync = try preparedSync(state: state, directory: local, device: "合成设备")
    do {
      _ = try await sync.status()
      XCTFail("no configuration")
    } catch let failure as IOSDictionarySyncFailure {
      XCTAssertEqual(failure.code, "dictionary_sync_not_configured")
    }
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences[IOSDictionarySyncPolicy.preference] = ["url": "http://127.0.0.1:9/dav/", "username": "someone@example.com", "password": Self.password]
    })
    let status = try await sync.status()
    XCTAssertNil(status.device_id)
    XCTAssertFalse(status.approved)
    // 刚改了词：只记录、不联网，没有密钥串时什么也不做。
    let recorded = try await sync.run(.localChange)
    XCTAssertFalse(recorded.network)
    XCTAssertNil(recorded.error)
    let run = try await sync.run(.manual)
    XCTAssertTrue(run.network)
    XCTAssertEqual(run.error, "dictionary_sync_unavailable")
    XCTAssertNil(run.report)
    XCTAssertGreaterThan(run.next_check_ms, 0)
    do {
      try await sync.probe(url: "https://dav.example.com/dav/", username: "a:b", password: Self.password)
      XCTFail("an invalid draft must be refused")
    } catch let failure as IOSDictionarySyncFailure {
      XCTAssertEqual(failure.code, "dictionary_sync_not_configured")
    }
    do {
      try await sync.probe(url: "http://127.0.0.1:9/dav/", username: "someone@example.com", password: Self.password)
      XCTFail("an unreachable server must fail the probe")
    } catch let failure as IOSDictionarySyncFailure {
      XCTAssertEqual(failure.code, "dictionary_sync_unavailable")
    }
    do {
      _ = try await sync.approve("not-a-device")
      XCTFail("an invalid device id must be refused")
    } catch let failure as IOSDictionarySyncFailure {
      XCTAssertEqual(failure.code, "invalid_dictionary_sync")
    }
    // 本机目录里没有凭据。
    for name in (try? FileManager.default.contentsOfDirectory(atPath: local.path)) ?? [] {
      let data = (try? Data(contentsOf: local.appendingPathComponent(name))) ?? Data()
      XCTAssertNil(data.range(of: Data(Self.password.utf8)), name)
    }
  }

  private func server() throws -> (url: String, username: String, password: String) {
    let environment = ProcessInfo.processInfo.environment
    guard let base = environment["MSIME_TEST_WEBDAV_URL"], !base.isEmpty else {
      throw XCTSkip("set MSIME_TEST_WEBDAV_URL to a local WebDAV server")
    }
    let url = (base.hasSuffix("/") ? base : base + "/") + "dictionary-\(UUID().uuidString.lowercased())/"
    return (url, environment["MSIME_TEST_WEBDAV_USERNAME"] ?? "", environment["MSIME_TEST_WEBDAV_PASSWORD"] ?? "")
  }

  /// 同一个安装的 App 和键盘扩展给同一个目录：两个同时第一次同步，网盘的词库密钥串里只登记一台设备，之后反复同步也不再多。
  func testAppAndKeyboardShareOneIdentity() async throws {
    let server = try server()
    let state = directory.appendingPathComponent("state", isDirectory: true)
    let local = directory.appendingPathComponent(IOSDictionarySync.directoryName, isDirectory: true)
    try await createCollection(server)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences[IOSDictionarySyncPolicy.preference] = ["url": server.url, "username": server.username, "password": server.password]
    })
    let app = try preparedSync(state: state, directory: local, device: "合成手机")
    let keyboard = try preparedSync(state: state, directory: local, device: "合成手机")
    async let first = app.run(.manual)
    async let second = keyboard.run(.manual)
    let (a, b) = try await (first, second)
    XCTAssertNil(a.error, a.error ?? "")
    XCTAssertNil(b.error, b.error ?? "")
    for _ in 0..<3 { _ = try await keyboard.run(.manual) }
    let devices = try await app.devices()
    XCTAssertEqual(devices.devices.count, 1)
    XCTAssertEqual(devices.devices.first?.current, true)
    XCTAssertEqual(devices.devices.first?.approved, true, "the first device generates the key")
    XCTAssertEqual(a.status.device_id, b.status.device_id)
    XCTAssertTrue(KeyboardDictionarySync.mayRecord(directory: local), "the keyboard may record once the app has joined")
  }

  /// 第二个安装登记后等待批准，两边的核对码相同；第一台批准后，它在第一台加的词出现在第二台的「我的词条」里。
  func testSecondInstallIsApprovedAndReceivesWords() async throws {
    let server = try server()
    try await createCollection(server)
    let stateA = directory.appendingPathComponent("a", isDirectory: true)
    let stateB = directory.appendingPathComponent("b", isDirectory: true)
    for state in [stateA, stateB] {
      XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
        preferences[IOSDictionarySyncPolicy.preference] = ["url": server.url, "username": server.username, "password": server.password]
      })
    }
    let a = try preparedSync(state: stateA, directory: stateA.appendingPathComponent("sync"), device: "合成手机")
    let b = try preparedSync(state: stateB, directory: stateB.appendingPathComponent("sync"), device: "合成平板")
    // 第一台先加一个词：经会话写进 Engine 的个人词库，然后放开会话的词库租约。
    let word = PersonalWord(kind: .pinyin, key: "shui'shan'ce'shi", value: "水杉测试")
    do {
      let session = MetasequoiaInputSessionBridge(stateRoot: stateA)
      try session.applyPersonalPrevious(nil, replacement: word.bridgeValue, requestID: UUID().uuidString)
      _ = session.suspendDictionarySession()
    }
    let joined = try await a.run(.manual)
    XCTAssertNil(joined.error, joined.error ?? "")
    let pending = try await b.run(.manual)
    XCTAssertEqual(pending.status.isPending, true)
    let waiting = try await a.devices()
    let other = try XCTUnwrap(IOSDictionarySyncPolicy.uniqueDevices(waiting.awaitingApproval).first)
    XCTAssertEqual(other.code, pending.status.code, "both sides show the same verification code")
    _ = try await a.approve(other.device_id)
    let received = try await b.run(.manual)
    XCTAssertNil(received.error, received.error ?? "")
    XCTAssertTrue(received.status.approved)
    let session = MetasequoiaInputSessionBridge(stateRoot: stateB)
    let entries = try session.personalEntries(atOffset: 0)["entries"] as? [[String: Any]] ?? []
    XCTAssertTrue(entries.contains { $0["value"] as? String == "水杉测试" }, "the word reached the second install")
  }

  /// 每次用一个新的子目录：网盘不需要预先建好它，但父目录要在。
  private func createCollection(_ server: (url: String, username: String, password: String)) async throws {
    var request = URLRequest(url: try XCTUnwrap(URL(string: server.url)))
    request.httpMethod = "MKCOL"
    if !server.username.isEmpty {
      request.setValue("Basic " + Data("\(server.username):\(server.password)".utf8).base64EncodedString(), forHTTPHeaderField: "Authorization")
    }
    let (_, response) = try await URLSession.shared.data(for: request)
    XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 201)
  }
}

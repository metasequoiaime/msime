import Foundation
import Security
import XCTest

/// 内存里的钥匙串：几个 `IOSCloudClipboardRelayKeystore` 共用它，就像 App 和键盘扩展共用 App Group 访问组里的同一项。记下每次查询，核对服务名、账号名、访问组和可读性。
private final class MemoryKeychain: @unchecked Sendable {
  private let lock = NSLock()
  private var items: [String: Data] = [:]
  private var recorded: [[String: Any]] = []
  var failReads: OSStatus?
  var queries: [[String: Any]] { lock.withLock { recorded } }
  var count: Int { lock.withLock { items.count } }

  private func key(_ query: [String: Any]) -> String {
    [query[kSecAttrService as String], query[kSecAttrAccount as String], query[kSecAttrAccessGroup as String]]
      .map { "\($0 ?? "-")" }.joined(separator: "|")
  }

  var keychain: IOSCloudClipboardRelayKeystore.Keychain {
    IOSCloudClipboardRelayKeystore.Keychain(
      copyMatching: { query in
        self.lock.withLock {
          self.recorded.append(query)
          if let failure = self.failReads { return (failure, nil) }
          guard let data = self.items[self.key(query)] else { return (errSecItemNotFound, nil) }
          return (errSecSuccess, data)
        }
      },
      update: { query, attributes in
        self.lock.withLock {
          self.recorded.append(query.merging(attributes) { _, new in new })
          guard self.items[self.key(query)] != nil else { return errSecItemNotFound }
          self.items[self.key(query)] = attributes[kSecValueData as String] as? Data
          return errSecSuccess
        }
      },
      add: { attributes in
        self.lock.withLock {
          self.recorded.append(attributes)
          self.items[self.key(attributes)] = attributes[kSecValueData as String] as? Data
          return errSecSuccess
        }
      },
      delete: { query in
        self.lock.withLock {
          guard self.items.removeValue(forKey: self.key(query)) != nil else { return errSecItemNotFound }
          return errSecSuccess
        }
      })
  }
}

/// 假的中继：记下设备登记（`PUT /devices/{id}`），`GET /keys` 答已经登记的设备和一把本机没有的密钥，于是每台设备都停在「等待批准」。只用来数这台手机在中继上登记了几台设备。
private final class FakeRelay: @unchecked Sendable {
  private let lock = NSLock()
  private var devices: [[String: Any]] = []
  private var registrations: [String] = []
  var registered: [String] { lock.withLock { registrations } }

  func send(_ method: String, _ path: String, _ body: Data?) -> (status: Int, body: Data) {
    lock.withLock {
      let base = "/v1/users/me/clipboard"
      if method == "GET", path.hasPrefix(base + "/keys") {
        let keys: [String: Any] = ["enabled": true, "key_id": String(repeating: "0", count: 32), "wrap": NSNull(), "devices": devices]
        return (200, try! JSONSerialization.data(withJSONObject: keys))
      }
      if method == "PUT", path.hasPrefix(base + "/devices/"), let body,
         let object = try? JSONSerialization.jsonObject(with: body) as? [String: Any] {
        let id = String(path.dropFirst((base + "/devices/").count))
        registrations.append(id)
        if !devices.contains(where: { $0["device_id"] as? String == id }) {
          devices.append(["device_id": id, "public_key": object["public_key"] ?? "", "name": object["name"] ?? "",
                          "created_at": "2026-10-27T00:00:00Z", "approved": false])
        }
        return (200, Data("{}".utf8))
      }
      return (404, Data(#"{"error":{"code":"not_found","message":"x"}}"#.utf8))
    }
  }
}

final class CloudClipboardRelayTests: XCTestCase {
  private var directory: URL!

  override func setUpWithError() throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent("relay-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: directory)
  }

  private func keystore(_ keychain: MemoryKeychain, binding: String = "webdav:synthetic") -> IOSCloudClipboardRelayKeystore {
    IOSCloudClipboardRelayKeystore(binding: binding, lockURL: directory.appendingPathComponent(IOSCloudClipboardRelayKeystore.lockName),
                                   keychain: keychain.keychain)
  }

  private static let secret = String(repeating: "A", count: 43)
  private static let otherSecret = String(repeating: "B", count: 43)

  // MARK: - 当场提示，与 Rust 和共享设置页的用例对应

  func testURLRulesMatchTheSharedSettingsPage() {
    let relay = IOSCloudClipboardRelayPolicy.relayURLIssue
    let webdav = IOSCloudClipboardRelayPolicy.webdavURLIssue
    XCTAssertNil(relay("https://relay.example.com/msime"))
    XCTAssertNil(relay("https://relay.example.com:8443"))
    XCTAssertNil(relay("http://127.0.0.1:8787"))
    XCTAssertNil(relay("http://localhost/msime"))
    XCTAssertNil(relay("http://[::1]:8787"))
    XCTAssertNil(webdav("https://dav.jianguoyun.com/dav/"))
    XCTAssertEqual(relay(""), "请填写中继地址。")
    XCTAssertEqual(webdav(""), "请填写服务器地址。")
    XCTAssertEqual(relay("http://relay.example.com"), "中继必须使用 HTTPS；HTTP 只支持本机回环地址（127.0.0.1、localhost）。")
    XCTAssertEqual(webdav("http://192.168.1.2/dav"), "WebDAV 服务器必须使用 HTTPS；HTTP 只支持本机回环地址（127.0.0.1、localhost）。")
    XCTAssertEqual(relay("https://user:pass@relay.example.com"), "中继地址不能包含用户名或密码，令牌请填在下面。")
    XCTAssertEqual(webdav("https://me@dav.example.com"), "服务器地址不能包含用户名或密码，账号和应用密码请填在下面。")
    XCTAssertEqual(relay("https://relay.example.com/?"), "中继地址不能带查询串或片段。")
    XCTAssertEqual(relay("https://relay.example.com/#x"), "中继地址不能带查询串或片段。")
    XCTAssertEqual(relay("relay.example.com"), "请填写以 https:// 开头的完整中继地址。")
    XCTAssertEqual(relay("ftp://relay.example.com"), "请填写以 https:// 开头的完整中继地址。")
    XCTAssertEqual(relay("https://relay.example.com:99999"), "请填写以 https:// 开头的完整中继地址。")
    XCTAssertEqual(relay("https:///path"), "请填写以 https:// 开头的完整中继地址。")
    XCTAssertEqual(relay("https://relay.example.com/\u{7}"), "中继地址不能包含控制字符。")
    XCTAssertEqual(relay("https://" + String(repeating: "a", count: 2048)), "中继地址过长。")
  }

  func testCredentialRulesMatchRust() {
    XCTAssertNil(IOSCloudClipboardRelayPolicy.tokenIssue(String(repeating: "t", count: 16)))
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.tokenIssue(""), "请填写中继令牌。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.tokenIssue("short-token"), "中继令牌应为 16 到 256 个字符。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.tokenIssue(String(repeating: "t", count: 257)), "中继令牌应为 16 到 256 个字符。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.tokenIssue("has a space in the token"), "中继令牌只能包含可打印的 ASCII 字符，不能有空格。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.tokenIssue("令牌令牌令牌令牌令牌令牌令牌令牌"), "中继令牌只能包含可打印的 ASCII 字符，不能有空格。")
    XCTAssertNil(IOSCloudClipboardRelayPolicy.usernameIssue("someone@example.com"))
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.usernameIssue(""), "请填写网盘账号。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.usernameIssue("a:b"), "网盘账号不能包含冒号。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.usernameIssue("a\u{85}b"), "网盘账号不能包含控制字符。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.usernameIssue(String(repeating: "账", count: 86)), "网盘账号过长。")
    XCTAssertNil(IOSCloudClipboardRelayPolicy.passwordIssue(" spaced app password "))
    XCTAssertNil(IOSCloudClipboardRelayPolicy.passwordIssue("a:b"))
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.passwordIssue(""), "请填写应用密码。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.passwordIssue("a\tb"), "应用密码不能包含控制字符。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.passwordIssue(String(repeating: "p", count: 257)), "应用密码过长。")
  }

  /// 绑定不含凭据；网盘的绑定不随应用密码变（换应用密码不用重新批准），中继的随令牌变。
  func testBindingsIdentifyTheSpaceWithoutCredentials() throws {
    let url = "https://dav.example.com/dav"
    let webdav = IOSCloudClipboardRelayPolicy.webdavBinding(url: url, username: "someone@example.com")
    XCTAssertTrue(webdav.hasPrefix("webdav:"))
    XCTAssertEqual(webdav.count, "webdav:".count + 32)
    XCTAssertEqual(webdav, IOSCloudClipboardRelayPolicy.webdavBinding(url: url, username: "someone@example.com"))
    XCTAssertNotEqual(webdav, IOSCloudClipboardRelayPolicy.webdavBinding(url: url, username: "other@example.com"))
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.webdavBinding(url: "", username: "x"), "")
    let first = try XCTUnwrap(IOSCloudClipboardRelay.Configuration(preferences: [
      "cloud_clipboard_webdav": ["url": url, "username": "someone@example.com", "password": "first-app-password"]]))
    let second = try XCTUnwrap(IOSCloudClipboardRelay.Configuration(preferences: [
      "cloud_clipboard_webdav": ["url": url, "username": "someone@example.com", "password": "second-app-password"]]))
    XCTAssertEqual(first.binding, second.binding)
    XCTAssertFalse(first.binding.contains("password"))
    let relay = IOSCloudClipboardRelayPolicy.relayBinding(url: "https://relay.example.com", token: String(repeating: "t", count: 16))
    XCTAssertTrue(relay.hasPrefix("relay:"))
    XCTAssertNotEqual(relay, IOSCloudClipboardRelayPolicy.relayBinding(url: "https://relay.example.com", token: String(repeating: "u", count: 16)))
    // 网盘优先，与共享层选路相同；都没配置时是水杉账号。
    XCTAssertEqual(IOSCloudClipboardRelay.Configuration(preferences: [
      "cloud_clipboard_relay": ["url": "https://relay.example.com", "token": String(repeating: "t", count: 16)],
      "cloud_clipboard_webdav": ["url": url, "username": "u", "password": "p"]])?.backend, .webdav)
    XCTAssertNil(IOSCloudClipboardRelay.Configuration(preferences: [:]))
  }

  /// 中继和网盘的错误码各有说法，令牌被拒不说成没登录；测试连接的说法与共享设置页相同。
  func testFailureWording() {
    XCTAssertFalse(IOSCloudClipboardRelayPolicy.failureMessage("relay_unauthorized").contains("登录"))
    XCTAssertTrue(IOSCloudClipboardRelayPolicy.failureMessage("webdav_rollback").contains("重新测试连接并保存"))
    XCTAssertTrue(IOSCloudClipboardRelayPolicy.failureMessage("webdav_rate_limited").contains("600"))
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.failureMessage("clipboard_device_pending"),
                   BackendAccountClient.Failure.clipboardMessages["clipboard_device_pending"])
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.probeMessage("relay_unauthorized"), "网盘拒绝了这个账号或应用密码。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.probeMessage("webdav_unsupported"),
                   "这个网盘不支持云剪贴板需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.probeMessage("something_else"), "测试连接失败，请稍后再试。")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.driverFailureCode(status: 0, code: "clipboard_device_pending"), "clipboard_device_pending")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.driverFailureCode(status: 400, code: "http_400"), "invalid_cloud_clipboard")
    XCTAssertEqual(IOSCloudClipboardRelayPolicy.driverFailureCode(status: 502, code: nil), "relay_unavailable")
  }

  // MARK: - 密钥串：一个安装、一个中继只有一个设备身份

  /// App 和键盘读写的是同一项：服务名、账号名、App Group 访问组都相同，不随备份或迁移去别的设备；与账号那份是不同的两项，退出水杉账号不删它。
  func testKeystoreIsOneSharedKeychainItemThatStaysOnThisDevice() throws {
    let keychain = MemoryKeychain()
    try keystore(keychain).save("v1.\(Self.secret)")
    let write = try XCTUnwrap(keychain.queries.last)
    XCTAssertEqual(write[kSecAttrService as String] as? String, BackendKeychain.defaultService)
    XCTAssertEqual(write[kSecAttrAccount as String] as? String, "cloud-clipboard-relay-keystore")
    XCTAssertNotEqual(write[kSecAttrAccount as String] as? String, BackendKeychainClipboardKeystore.account)
    XCTAssertEqual(write[kSecAttrAccessGroup as String] as? String, BackendKeychain.defaultAccessGroup)
    XCTAssertEqual(BackendKeychain.defaultAccessGroup, MSIMEAppEdition.appGroupIdentifier, "App 和键盘扩展都持有 App Group 访问组")
    XCTAssertEqual(write[kSecAttrAccessible as String] as? String, kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String)
    // 另一个进程（键盘）用自己的实例读到的是同一份身份。
    XCTAssertEqual(try keystore(keychain).load(), "v1.\(Self.secret)")
    XCTAssertEqual(keychain.count, 1)
  }

  /// 已经存着一个身份时，另一个身份写不进去（App 和键盘第一次同时用时只留先到的那个）；同一个身份带上新的剪贴板密钥照常写。换了中继或网盘账号时旧的读作没有，写入直接覆盖。
  func testSecondIdentityIsRefusedAndAnotherRelayStartsFresh() throws {
    let keychain = MemoryKeychain()
    try keystore(keychain).save("v1.\(Self.secret)")
    XCTAssertThrowsError(try keystore(keychain).save("v1.\(Self.otherSecret)")) { error in
      XCTAssertEqual((error as? BackendClipboardKeystoreConflict)?.stored, "v1.\(Self.secret)")
    }
    let withKey = "v1.\(Self.secret).\(String(repeating: "0", count: 32)).\(String(repeating: "C", count: 43))"
    try keystore(keychain).save(withKey)
    XCTAssertEqual(try keystore(keychain).load(), withKey)
    XCTAssertNil(try keystore(keychain, binding: "webdav:another").load())
    try keystore(keychain, binding: "webdav:another").save("v1.\(Self.otherSecret)")
    XCTAssertEqual(try keystore(keychain, binding: "webdav:another").load(), "v1.\(Self.otherSecret)")
    XCTAssertNil(try keystore(keychain).load())
    XCTAssertThrowsError(try keystore(keychain).save("not a keystore"))
  }

  /// 钥匙串读不出来（设备锁着、缺权限）是失败，不当作没有：否则会生成新的设备身份，这台设备就得重新批准，中继上多出一台待批准的设备。写之前换了中继也不写。
  func testUnreadableKeychainNeverMintsANewIdentity() throws {
    let keychain = MemoryKeychain()
    try keystore(keychain).save("v1.\(Self.secret)")
    keychain.failReads = errSecInteractionNotAllowed
    XCTAssertThrowsError(try keystore(keychain).load()) { error in
      XCTAssertEqual((error as? IOSCloudClipboardRelayFailure)?.code, "relay_storage")
    }
    XCTAssertThrowsError(try keystore(keychain).save("v1.\(Self.otherSecret)"))
    keychain.failReads = nil
    var changed = keystore(keychain, binding: "webdav:changed")
    changed.ensureCurrent = { throw IOSCloudClipboardRelayFailure(code: "relay_changed") }
    XCTAssertThrowsError(try changed.save("v1.\(Self.otherSecret)"))
    XCTAssertEqual(try keystore(keychain).load(), "v1.\(Self.secret)")
  }

  /// 真实的 Rust 驱动：App 和键盘（两个密钥串实例共用一项钥匙串和一把锁）同时第一次打开云剪贴板，中继上只登记一台设备；之后每次刷新、重试都不再发登记请求。
  func testAppAndKeyboardRegisterOneDeviceAndRetriesDoNotRepost() async throws {
    let keychain = MemoryKeychain()
    let relay = FakeRelay()
    let drive = IOSCloudClipboardDriver.e2e.drive
    func list(_ name: String) async throws -> BackendAccountClient.ClipboardPage {
      let e2e = BackendClipboardE2E(drive: drive, keystore: keystore(keychain), deviceName: { name })
      let data = try await BackendAccountClient.clipboardE2E(.list(search: ""), e2e: e2e) { method, path, body in
        relay.send(method, path, body)
      }
      return try BackendAccountClient.clipboardPage(data)
    }
    async let app = list("App")
    async let keyboard = list("键盘")
    let pages = try await [app, keyboard]
    XCTAssertTrue(pages.allSatisfy(\.isPending))
    XCTAssertEqual(Set(relay.registered).count, 1, "one installation is one device: \(relay.registered)")
    XCTAssertEqual(pages[0].e2e?.code, pages[1].e2e?.code, "both show the same verification code")
    let registrations = relay.registered.count
    for _ in 0..<3 { _ = try await list("App") }
    XCTAssertEqual(relay.registered.count, registrations, "a pending device is not registered again on refresh")
  }

  // MARK: - 选路和 C ABI

  /// 偏好里配置了网盘或中继时共享层答 `relay`，iOS 读出同一份配置；去掉后回到水杉账号。
  func testSourceFollowsTheSharedPreferences() async throws {
    let state = directory.appendingPathComponent("state", isDirectory: true)
    let selected1 = try await IOSCloudClipboardRelay.selected(preferencesDirectory: state.path)
    XCTAssertFalse(selected1)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences["cloud_clipboard_webdav"] = ["url": "http://127.0.0.1:9/dav", "username": "someone@example.com", "password": "synthetic-app-password"]
    })
    let selected2 = try await IOSCloudClipboardRelay.selected(preferencesDirectory: state.path)
    XCTAssertTrue(selected2)
    XCTAssertEqual(try IOSCloudClipboardRelay.currentConfiguration(preferencesDirectory: state.path)?.backend, .webdav)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences.removeValue(forKey: "cloud_clipboard_webdav")
      preferences["cloud_clipboard_relay"] = ["url": "http://127.0.0.1:9", "token": String(repeating: "t", count: 16)]
    })
    XCTAssertEqual(try IOSCloudClipboardRelay.currentConfiguration(preferencesDirectory: state.path)?.backend, .relay)
    // 中继和网盘同时配置的偏好存不进去（Rust 校验）。
    XCTAssertFalse(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences["cloud_clipboard_webdav"] = ["url": "https://dav.example.com", "username": "u", "password": "p"]
    })
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences.removeValue(forKey: "cloud_clipboard_relay")
    })
    let selected3 = try await IOSCloudClipboardRelay.selected(preferencesDirectory: state.path)
    XCTAssertFalse(selected3)
  }

  /// 配置了网盘就只经网盘：连不上时报 `relay_unavailable`，一个账号请求都不发；测试连接的草稿不合规时不发请求。
  func testRelayRouteNeverFallsBackToTheAccount() async throws {
    let state = directory.appendingPathComponent("state", isDirectory: true)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { preferences in
      preferences["cloud_clipboard_webdav"] = ["url": "http://127.0.0.1:9/dav", "username": "someone@example.com", "password": "synthetic-app-password"]
    })
    FakeRust.answer { call in
      if call.entry == "account" && call.operation == "status" { return FakeRust.ok(["user": FakeRust.user("user-1")]) }
      return FakeRust.failure("rust_client_unavailable")
    }
    var clipboard = IOSCloudClipboard(account: FakeRust.account, deviceName: "合成设备")
    clipboard.preferencesDirectory = state.path
    clipboard.relayDirectory = directory.appendingPathComponent("relay", isDirectory: true)
    // 驱动在第一个请求之前把新身份存进钥匙串；测完删掉，不留在测试宿主的钥匙串里。
    defer { try? IOSCloudClipboardRelayKeystore(binding: "", lockURL: directory.appendingPathComponent("lock")).clear() }
    do {
      _ = try await clipboard.refresh(boundTo: nil, search: "") { _ in nil }
      XCTFail("an unreachable WebDAV server must fail")
    } catch let failure as IOSCloudClipboardRelayFailure where failure.code == "relay_storage" {
      throw XCTSkip("the test host has no App Group keychain entitlement in this build")
    } catch let failure as IOSCloudClipboardRelayFailure {
      XCTAssertEqual(failure.code, "relay_unavailable")
    }
    XCTAssertTrue(FakeRust.calls.filter { $0.entry != "account" }.isEmpty, "no account cloud clipboard request")
    do {
      try await IOSCloudClipboardRelay.probeWebDAV(url: "https://dav.example.com", username: "a:b", password: "p",
                                                    directory: directory.appendingPathComponent("relay", isDirectory: true))
      XCTFail("an invalid draft must be refused")
    } catch let failure as IOSCloudClipboardRelayFailure {
      XCTAssertEqual(failure.code, "relay_not_configured")
    }
    do {
      try await IOSCloudClipboardRelay.probeWebDAV(url: "http://127.0.0.1:9/dav", username: "someone@example.com", password: "p",
                                                    directory: directory.appendingPathComponent("relay", isDirectory: true))
      XCTFail("an unreachable server must fail the probe")
    } catch let failure as IOSCloudClipboardRelayFailure {
      XCTAssertEqual(failure.code, "relay_unavailable")
    }
  }
}

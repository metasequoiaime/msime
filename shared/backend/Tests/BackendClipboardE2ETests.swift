import Foundation
import XCTest
@testable import MSIMEBackend

/// 测试用的密钥串存放处，把每次保存记进共享的事件表，用来核对「先保存、再发请求」的顺序。
final class MemoryClipboardKeystore: BackendClipboardKeystoreStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: String?
  private var failure: Error?
  let events: ClipboardEventLog
  init(_ value: String? = nil, events: ClipboardEventLog = .init(), failing failure: Error? = nil) {
    self.value = value; self.events = events; self.failure = failure
  }
  var stored: String? { lock.lock(); defer { lock.unlock() }; return value }
  func load() throws -> String? {
    lock.lock(); defer { lock.unlock() }
    if let failure { throw failure }
    return value
  }
  func save(_ value: String) throws {
    lock.lock(); self.value = value; lock.unlock()
    events.append("save:" + value)
  }
  func clear() throws { lock.lock(); value = nil; lock.unlock() }
}

final class ClipboardEventLog: @unchecked Sendable {
  private let lock = NSLock()
  private var values: [String] = []
  var all: [String] { lock.lock(); defer { lock.unlock() }; return values }
  func append(_ value: String) { lock.lock(); values.append(value); lock.unlock() }
}

/// 不做任何加密的驱动替身：`list` 和 `add` 把服务端的正文当作明文结果交回（`list` 在本地按子串搜索），其余操作取 `GET …/keys` 并原样交回正文。服务端的非 2xx 响应变成驱动错误，错误码取正文的 `error.code`。只用来让依赖驱动的上层代码在没有 Rust 库的测试进程里照常跑。
final class PassthroughClipboardDriver: @unchecked Sendable {
  private let lock = NSLock()
  private var recorded: [[String: Any]] = []
  var inputs: [[String: Any]] { lock.lock(); defer { lock.unlock() }; return recorded }

  func e2e(keystore: any BackendClipboardKeystoreStorage = MemoryClipboardKeystore()) -> BackendClipboardE2E {
    BackendClipboardE2E(drive: { [self] in call($0) }, keystore: keystore, deviceName: { "合成设备" })
  }

  func call(_ data: Data) -> Data? {
    guard let input = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
    lock.lock(); recorded.append(input); lock.unlock()
    let value: [String: Any]
    if let start = input["start"] as? [String: Any], let op = start["op"] as? String {
      switch op {
      case "list":
        value = request("GET", "/v1/users/me/clipboard", body: nil, state: ["op": "list", "search": start["search"] ?? ""])
      case "add":
        let body = String(data: try! JSONSerialization.data(withJSONObject: ["text": start["text"] ?? ""]), encoding: .utf8)!
        value = request("POST", "/v1/users/me/clipboard", body: body, state: ["op": "add"])
      default:
        value = request("GET", "/v1/users/me/clipboard/keys?device_id=" + String(repeating: "0", count: 32), body: nil, state: ["op": op])
      }
    } else if let state = input["resume"] as? [String: Any], let response = input["response"] as? [String: Any],
              let status = response["status"] as? Int, let body = response["body"] as? String {
      if !(200..<300).contains(status) {
        let code = BackendAccountClient.errorCode(Data(body.utf8)) ?? "http_\(status)"
        value = ["error": ["status": status, "code": code], "keystore": "synthetic-keystore"]
      } else if var object = try? JSONSerialization.jsonObject(with: Data(body.utf8)) as? [String: Any] {
        if state["op"] as? String == "list" {
          let search = (state["search"] as? String ?? "").lowercased()
          let items = (object["items"] as? [[String: Any]] ?? []).filter { item in
            search.isEmpty || (item["text"] as? String)?.lowercased().contains(search) == true
          }
          object["items"] = items
          let enabled = object["enabled"] as? Bool ?? false
          object["e2e"] = ["status": enabled ? "ready" : "disabled", "code": "0000 0000 0000 0000",
                           "pending_devices": 0, "undecryptable": 0]
        }
        value = ["done": object, "keystore": "synthetic-keystore"]
      } else {
        value = ["error": ["status": 0, "code": "invalid_response"], "keystore": "synthetic-keystore"]
      }
    } else { return nil }
    return try? JSONSerialization.data(withJSONObject: ["ok": true, "value": value])
  }

  private func request(_ method: String, _ path: String, body: String?, state: [String: Any]) -> [String: Any] {
    ["request": ["method": method, "path": path, "body": body.map { $0 as Any } ?? NSNull()], "state": state, "keystore": "synthetic-keystore"]
  }
}

/// 按顺序交出预先写好的每一步，并记下驱动收到的输入。
private final class ScriptedClipboardDriver: @unchecked Sendable {
  private let lock = NSLock()
  private var steps: [[String: Any]]
  private var recorded: [[String: Any]] = []
  init(_ steps: [[String: Any]]) { self.steps = steps }
  var inputs: [[String: Any]] { lock.lock(); defer { lock.unlock() }; return recorded }
  func call(_ data: Data) -> Data? {
    lock.lock(); defer { lock.unlock() }
    recorded.append(try! JSONSerialization.jsonObject(with: data) as! [String: Any])
    guard !steps.isEmpty else { return Data(#"{"ok":false,"error":"no more steps"}"#.utf8) }
    return try! JSONSerialization.data(withJSONObject: ["ok": true, "value": steps.removeFirst()])
  }
}

/// 按顺序回放预先写好的响应，并把每个请求记进事件表。`nil` 状态表示传输失败。
private final class ClipboardE2EProtocol: URLProtocol {
  struct Recorded { let method: String; let path: String; let authorization: String?; let body: String? }
  private static let lock = NSLock()
  private static var responses: [(status: Int?, body: String)] = []
  private static var recorded: [Recorded] = []
  static var events = ClipboardEventLog()
  private static var beforeResponse: (@Sendable () async -> Void)?
  static var requests: [Recorded] { lock.lock(); defer { lock.unlock() }; return recorded }
  static func reset(_ responses: [(status: Int?, body: String)], events: ClipboardEventLog,
                    beforeResponse: (@Sendable () async -> Void)? = nil) {
    lock.lock(); Self.responses = responses; recorded = []; Self.events = events; Self.beforeResponse = beforeResponse; lock.unlock()
  }
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var target = request.url!.path
    if let query = request.url!.query { target += "?" + query }
    var payload = request.httpBody
    if payload == nil, let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var data = Data(); var buffer = [UInt8](repeating: 0, count: 4096)
      while true { let count = stream.read(&buffer, maxLength: buffer.count); if count <= 0 { break }; data.append(contentsOf: buffer.prefix(count)) }
      payload = data
    }
    Self.lock.lock()
    Self.recorded.append(.init(method: request.httpMethod!, path: target,
                               authorization: request.value(forHTTPHeaderField: "Authorization"),
                               body: payload.flatMap { String(data: $0, encoding: .utf8) }))
    let next: (status: Int?, body: String) = Self.responses.isEmpty ? (500, "") : Self.responses.removeFirst()
    let pause = Self.beforeResponse
    Self.beforeResponse = nil
    Self.lock.unlock()
    Self.events.append(request.httpMethod! + " " + target)
    Task {
      await pause?()
      self.respond(next)
    }
  }
  private func respond(_ next: (status: Int?, body: String)) {
    guard let status = next.status else {
      client?.urlProtocol(self, didFailWithError: URLError(.notConnectedToInternet))
      return
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil,
                                   headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    if !next.body.isEmpty { client?.urlProtocol(self, didLoad: Data(next.body.utf8)) }
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private actor ClipboardResponseGate {
  private var waiting: CheckedContinuation<Void, Never>?
  private var observer: CheckedContinuation<Void, Never>?
  func pause() async {
    await withCheckedContinuation { waiting = $0; observer?.resume(); observer = nil }
  }
  func waitUntilPaused() async {
    if waiting != nil { return }
    await withCheckedContinuation { observer = $0 }
  }
  func resume() { waiting?.resume(); waiting = nil }
}

private final class ClipboardSessionStore: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?
  let keystore: MemoryClipboardKeystore
  init(_ value: BackendSavedSession, keystore: MemoryClipboardKeystore) {
    self.value = value; self.keystore = keystore
  }
  func load() throws -> BackendSavedSession? { lock.withLock { value } }
  func save(_ value: BackendSavedSession) throws { lock.withLock { self.value = value } }
  func clear() throws { lock.withLock { value = nil }; try keystore.clear() }
}

final class BackendClipboardE2ETests: XCTestCase {
  private let deviceID = String(repeating: "a", count: 32)
  private let itemID = String(repeating: "c", count: 64)

  private func client() -> BackendAccountClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ClipboardE2EProtocol.self]
    return BackendAccountClient(configuration: configuration)
  }
  private func json(_ object: Any) -> String { String(data: try! JSONSerialization.data(withJSONObject: object), encoding: .utf8)! }
  private func request(_ method: String, _ path: String, body: String? = nil, state: Int, keystore: String) -> [String: Any] {
    ["request": ["method": method, "path": path, "body": body.map { $0 as Any } ?? NSNull()], "state": ["opaque": state], "keystore": keystore]
  }


  func testLateClipboardResponseCannotRestoreKeystoreAfterLogout() async throws {
    try await checkLateClipboardResponse(relogin: false)
  }

  func testLateClipboardResponseCannotOverwriteReloginKeystore() async throws {
    try await checkLateClipboardResponse(relogin: true)
  }

  func testOtherProcessLogoutStopsLateClipboardResponse() async throws {
    try await checkLateClipboardResponse(relogin: false, otherProcess: true)
  }

  func testOtherProcessReloginStopsLateClipboardResponse() async throws {
    try await checkLateClipboardResponse(relogin: true, otherProcess: true)
  }

  private func checkLateClipboardResponse(relogin: Bool, otherProcess: Bool = false) async throws {
    let tokens = BackendAccountClient.Tokens(access_token: String(repeating: "a", count: 64),
      refresh_token: String(repeating: "f", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-user", display_name: "合成用户", created_at: "2026-10-16"))
    let keystore = MemoryClipboardKeystore("old-keystore")
    let storage = ClipboardSessionStore(try BackendSavedSession.forTokens(tokens), keystore: keystore)
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let sharedLock = BackendFileRefreshLock(url: directory.appendingPathComponent("account-refresh.lock"))
    let session = BackendAccountSession(storage: storage, refreshLock: sharedLock)
    let gate = ClipboardResponseGate()
    let page: [String: Any] = ["enabled": true, "items": []]
    let driver = ScriptedClipboardDriver([
      request("GET", "/v1/users/me/clipboard", state: 1, keystore: "old-keystore"),
      request("POST", "/v1/users/me/clipboard", state: 2, keystore: "late-keystore"),
      ["done": page, "keystore": "late-keystore"],
    ])
    ClipboardE2EProtocol.reset([(200, "{}"), (200, "{}")], events: .init(), beforeResponse: { await gate.pause() })
    let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: keystore, deviceName: { "合成设备" })
    let api = client()
    // 与键盘扩展（`BackendKeyboardCloudClipboardService`）相同：操作绑定发起时的登录，令牌经会话取。
    let identity = try await session.credentials()
    let bound = e2e.bound(to: session, userID: identity.userID, sessionID: identity.sessionID)
    let pending = Task {
      try await session.authenticated(matchingUserID: identity.userID, matchingSessionID: identity.sessionID) { token in
        try await api.clipboard(token: token, e2e: bound)
      }.value
    }
    await gate.waitUntilPaused()
    let signingOut = otherProcess ? BackendAccountSession(storage: storage, refreshLock: sharedLock) : session
    try await signingOut.forget()
    if relogin {
      // 同一账号重新登录也必须隔开旧操作。
      try storage.save(BackendSavedSession.forTokens(tokens))
      try keystore.save("replacement-keystore")
    }
    await gate.resume()
    do { _ = try await pending.value; XCTFail("旧会话应取消") }
    catch is CancellationError { }
    XCTAssertEqual(keystore.stored, relogin ? "replacement-keystore" : nil)
    XCTAssertEqual(ClipboardE2EProtocol.requests.count, 1)
  }

  /// 每个请求都带账号凭据发出，任何状态码的响应都原样交回驱动；密钥串一变就在下一个请求之前保存，没变就不写。
  func testDriverLoopSendsEveryRequestAndSavesTheKeystoreFirst() async throws {
    let events = ClipboardEventLog()
    let keystore = MemoryClipboardKeystore("old-keystore", events: events)
    let keysBody = #"{"enabled":true,"key_id":null,"wrap":null,"devices":[]}"#
    let conflict = #"{"error":{"code":"clipboard_key_conflict","message":"x"}}"#
    let listBody = #"{"enabled":true,"retention_days":7,"key_id":"k","items":[]}"#
    let page: [String: Any] = ["enabled": true, "retention_days": 7,
      "items": [["id": itemID, "text": "合成剪贴板文本", "updated_at": "2026-10-11T08:00:00Z", "pinned": true, "device": "合成设备"]],
      "e2e": ["status": "ready", "code": "0421 9873 5530 1176", "pending_devices": 2, "undecryptable": 1]]
    let driver = ScriptedClipboardDriver([
      request("GET", "/v1/users/me/clipboard/keys?device_id=\(deviceID)", state: 1, keystore: "new-keystore-1"),
      request("PUT", "/v1/users/me/clipboard/devices/\(deviceID)", body: #"{"public_key":"synthetic","name":"合成设备"}"#, state: 2, keystore: "new-keystore-1"),
      request("GET", "/v1/users/me/clipboard", state: 3, keystore: "new-keystore-2"),
      ["done": page, "keystore": "new-keystore-2"],
    ])
    ClipboardE2EProtocol.reset([(200, keysBody), (409, conflict), (200, listBody)], events: events)
    let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: keystore, deviceName: { "合成设备" })

    let result = try await client().clipboard(token: "synthetic-token", search: "合成", e2e: e2e)

    XCTAssertEqual(events.all, ["save:new-keystore-1",
                                "GET /v1/users/me/clipboard/keys?device_id=\(deviceID)",
                                "PUT /v1/users/me/clipboard/devices/\(deviceID)",
                                "save:new-keystore-2",
                                "GET /v1/users/me/clipboard"])
    XCTAssertEqual(keystore.stored, "new-keystore-2")
    XCTAssertTrue(ClipboardE2EProtocol.requests.allSatisfy { $0.authorization == "Bearer synthetic-token" })
    XCTAssertEqual(ClipboardE2EProtocol.requests[1].body, #"{"public_key":"synthetic","name":"合成设备"}"#)
    let inputs = driver.inputs
    XCTAssertEqual(inputs.count, 4)
    let start = try XCTUnwrap(inputs[0]["start"] as? [String: Any])
    XCTAssertEqual(start["op"] as? String, "list")
    XCTAssertEqual(start["search"] as? String, "合成")
    XCTAssertEqual(inputs[0]["keystore"] as? String, "old-keystore")
    XCTAssertEqual(inputs[0]["device_name"] as? String, "合成设备")
    for (index, (status, body)) in [(200, keysBody), (409, conflict), (200, listBody)].enumerated() {
      let input = inputs[index + 1]
      XCTAssertEqual((input["resume"] as? [String: Any])?["opaque"] as? Int, index + 1)
      let response = try XCTUnwrap(input["response"] as? [String: Any])
      XCTAssertEqual(response["status"] as? Int, status)
      XCTAssertEqual(response["body"] as? String, body)
    }
    XCTAssertEqual(result.retention_days, 7)
    XCTAssertEqual(result.items.first?.text, "合成剪贴板文本")
    XCTAssertEqual(result.items.first?.pinned, true)
    XCTAssertEqual(result.items.first?.device, "合成设备")
    XCTAssertEqual(result.e2e, .init(status: "ready", code: "0421 9873 5530 1176", pending_devices: 2, undecryptable: 1))
    XCTAssertFalse(result.isPending)
  }

  /// 第一次使用时没有密钥串，驱动收到 null。
  func testFirstUseStartsWithoutAKeystore() async throws {
    let keystore = MemoryClipboardKeystore()
    let page: [String: Any] = ["enabled": true, "items": [],
                               "e2e": ["status": "pending", "code": "0421 9873 5530 1176", "pending_devices": 0, "undecryptable": 0]]
    let driver = ScriptedClipboardDriver([["done": page, "keystore": "created"]])
    ClipboardE2EProtocol.reset([], events: .init())
    let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: keystore, deviceName: { "合成设备" })
    let result = try await client().clipboard(token: "synthetic-token", e2e: e2e)
    XCTAssertTrue(result.isPending)
    XCTAssertTrue(driver.inputs.first?["keystore"] is NSNull)
    XCTAssertEqual(keystore.stored, "created")
    XCTAssertTrue(ClipboardE2EProtocol.requests.isEmpty)
  }

  /// 驱动的错误保留 `status` 和 `code`，提示按错误码给出。
  func testDriverErrorsKeepStatusAndCode() async throws {
    for (status, code, message) in [(0, "clipboard_device_pending", BackendAccountClient.Failure.clipboardMessages["clipboard_device_pending"]!),
                                     (403, "clipboard_sync_disabled", "云剪贴板未开启"),
                                     (502, "http_502", "请求未完成，请稍后重试。")] {
      let keystore = MemoryClipboardKeystore("kept")
      let driver = ScriptedClipboardDriver([["error": ["status": status, "code": code], "keystore": "kept"]])
      let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: keystore, deviceName: { "合成设备" })
      do {
        _ = try await client().addClipboard("合成剪贴板文本", token: "synthetic-token", e2e: e2e)
        XCTFail("a driver error must fail the operation")
      } catch let failure as BackendAccountClient.Failure {
        XCTAssertEqual(failure.status, status)
        XCTAssertEqual(failure.code, code)
        XCTAssertEqual(failure.localizedDescription, message)
      }
      let start = try XCTUnwrap(driver.inputs.first?["start"] as? [String: Any])
      XCTAssertEqual(start["op"] as? String, "add")
      XCTAssertEqual(start["text"] as? String, "合成剪贴板文本")
    }
  }

  /// 传输失败照常抛错，不交给驱动。
  func testTransportFailureIsNotHandedToTheDriver() async throws {
    let driver = ScriptedClipboardDriver([request("GET", "/v1/users/me/clipboard/keys?device_id=\(deviceID)", state: 1, keystore: "k")])
    ClipboardE2EProtocol.reset([(nil, "")], events: .init())
    let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: MemoryClipboardKeystore(), deviceName: { "合成设备" })
    do {
      _ = try await client().clipboard(token: "synthetic-token", e2e: e2e)
      XCTFail("a transport failure must throw")
    } catch is BackendAccountClient.Failure {
      XCTFail("a transport failure is not a server answer")
    } catch { }
    XCTAssertEqual(driver.inputs.count, 1)
  }

  /// 驱动只能访问云剪贴板自己的接口，也只能用这四种方法。
  func testDriverCannotSendTheTokenElsewhere() async throws {
    for (method, path) in [("GET", "/v1/users/me"), ("GET", "/v1/users/me/clipboardx"), ("PATCH", "/v1/users/me/clipboard")] {
      let driver = ScriptedClipboardDriver([request(method, path, state: 1, keystore: "k")])
      ClipboardE2EProtocol.reset([(200, "{}")], events: .init())
      let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: MemoryClipboardKeystore(), deviceName: { "合成设备" })
      do {
        _ = try await client().clipboard(token: "synthetic-token", e2e: e2e)
        XCTFail("request outside the clipboard API was sent: \(method) \(path)")
      } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }
      XCTAssertTrue(ClipboardE2EProtocol.requests.isEmpty)
    }
  }

  /// 没有驱动、密钥串读不出来、输入不合法时什么都不发，驱动也不会被调用。
  func testNothingIsSentWithoutADriverAReadableKeystoreOrValidInput() async throws {
    ClipboardE2EProtocol.reset([], events: .init())
    do { _ = try await client().clipboard(token: "synthetic-token", e2e: nil); XCTFail("no driver") }
    catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }

    let driver = ScriptedClipboardDriver([])
    let unreadable = BackendClipboardE2E(drive: { driver.call($0) },
      keystore: MemoryClipboardKeystore(failing: BackendAccountClient.Failure(status: 0)), deviceName: { "合成设备" })
    do { _ = try await client().clipboard(token: "synthetic-token", e2e: unreadable); XCTFail("unreadable keystore") }
    catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }

    let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: MemoryClipboardKeystore(), deviceName: { "合成设备" })
    for operation: BackendAccountClient.ClipboardE2EOperation in [
      .approve(deviceID: "../auth"), .reject(deviceID: String(repeating: "A", count: 32)), .remove(deviceID: ""),
    ] {
      do { _ = try await client().clipboardE2E(operation, token: "synthetic-token", e2e: e2e); XCTFail("invalid device id") }
      catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    }
    do { _ = try await client().addClipboard(String(repeating: "😀", count: 2001), token: "synthetic-token", e2e: e2e); XCTFail("too long") }
    catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 400)
      XCTAssertEqual(failure.code, "invalid_clipboard_text")
    }
    XCTAssertTrue(driver.inputs.isEmpty)
    XCTAssertTrue(ClipboardE2EProtocol.requests.isEmpty)
  }

  /// 本机等待批准时列表为空，`isPending` 为真，核对码随列表一起给出。
  func testPendingListCarriesTheVerificationCode() async throws {
    let page: [String: Any] = ["enabled": true, "retention_days": 0, "items": [],
                               "e2e": ["status": "pending", "code": "0421 9873 5530 1176", "pending_devices": 0, "undecryptable": 0]]
    let driver = ScriptedClipboardDriver([["done": page, "keystore": "k"]])
    let e2e = BackendClipboardE2E(drive: { driver.call($0) }, keystore: MemoryClipboardKeystore(), deviceName: { "合成设备" })
    let result = try await client().clipboard(token: "synthetic-token", e2e: e2e)
    XCTAssertTrue(result.isPending && result.items.isEmpty)
    XCTAssertEqual(result.e2e?.code, "0421 9873 5530 1176")
  }

  /// C ABI 包装：请求原样交给函数，回复按 C 字符串读出后释放一次。
  func testCABIWrapperFreesTheReplyOnce() {
    final class Box: @unchecked Sendable { var freed = 0; var seen = Data() }
    let box = Box()
    let reply = strdup(#"{"ok":true,"value":{}}"#)!
    let drive = BackendClipboardE2E.callingCABI({ pointer, length in
      box.seen = Data(bytes: pointer!, count: length)
      return reply
    }, free: { value in
      XCTAssertEqual(value, reply)
      box.freed += 1
      free(value)
    })
    XCTAssertEqual(drive(Data("{\"start\":1}".utf8)), Data(#"{"ok":true,"value":{}}"#.utf8))
    XCTAssertEqual(box.seen, Data("{\"start\":1}".utf8))
    XCTAssertEqual(box.freed, 1)
    XCTAssertNil(BackendClipboardE2E.callingCABI({ _, _ in nil }, free: { _ in XCTFail("nothing to free") })(Data("{}".utf8)))
  }

  #if os(macOS)
  /// macOS 的密钥串文件：与会话文件同目录、仅本人可读写、原子替换，退出登录时随会话一起删除。
  func testDesktopKeystoreFileIsOwnerOnlyAndClearedWithTheSession() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("clipboard-keystore-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let keystore = BackendDesktopClipboardKeystoreFile(directory: directory)
    XCTAssertNil(try keystore.load())
    try keystore.save("v1.synthetic-keystore")
    let path = directory.appendingPathComponent("cloud-clipboard-keystore").path
    let mode = try XCTUnwrap(FileManager.default.attributesOfItem(atPath: path)[.posixPermissions] as? Int)
    XCTAssertEqual(mode & 0o777, 0o600)
    XCTAssertEqual(try keystore.load(), "v1.synthetic-keystore")
    try keystore.save("v1.replaced")
    XCTAssertEqual(try keystore.load(), "v1.replaced")
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path), ["cloud-clipboard-keystore"])
    // 另一个进程写的文件末尾带换行也能读。
    try Data("v1.written-elsewhere\n".utf8).write(to: URL(fileURLWithPath: path))
    chmod(path, 0o600)
    XCTAssertEqual(try keystore.load(), "v1.written-elsewhere")
    XCTAssertThrowsError(try keystore.save("has space"))
    // 别人能读的文件不是我们写的，不用。
    chmod(path, 0o644)
    XCTAssertThrowsError(try keystore.load())
    chmod(path, 0o600)

    let session = BackendDesktopSessionFile(directory: directory)
    try session.save(.init(tokens: .init(access_token: String(repeating: "a", count: 64), refresh_token: String(repeating: "f", count: 64),
                                         token_type: "Bearer", expires_in: 900,
                                         user: .init(id: "synthetic-user", display_name: "测试", created_at: "2026-09-08")),
                           expiresAt: Date().addingTimeInterval(900)))
    try session.clear()
    XCTAssertNil(try keystore.load())
    XCTAssertNil(try session.load())
    try keystore.clear()
  }
  #endif
}

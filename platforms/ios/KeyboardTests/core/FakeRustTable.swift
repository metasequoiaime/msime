// 替身 Rust：iOS 的 Swift 测试经它代替 `msime_rust_client_linked()` 的函数表，不联网、不碰钥匙串和 App Group。C 回调不能捕获变量，所以每个入口都转到 `FakeRust` 的静态状态，测试按入口和 action 给出回答。测试进程里串行使用：每个用例开头调 `answer` 换掉回答、清掉记录。
import CMSIMEClient
import Foundation
import XCTest

enum FakeRust {
  /// 一次调用：入口名（`account`、`community`、`cloud_clipboard`、`cloud_dictionary`、`cloud_sync`、`ios_settings_export`、`ios_settings_apply`、`chat_stream`、`chat_cancel`、`ai_skin_generate`、`ai_skin_cancel`）和整个请求。
  struct Call {
    let entry: String
    let request: [String: Any]
    /// 请求里的 action；没有 action 的入口（`chat_stream`、`ios_settings_*`）是空的。
    var action: [String: Any] { request["action"] as? [String: Any] ?? [:] }
    var operation: String? { action["operation"] as? String }
  }

  /// 流式对话的回答：先交出 `deltas`，再以 `result` 结束。
  struct Stream {
    var deltas: [String]
    var result: String
  }

  private static let lock = NSLock()
  private static var recorded: [Call] = []
  private static var respond: (Call) -> String = { _ in failure("rust_client_unavailable") }
  private static var stream: (Call) -> Stream = { _ in Stream(deltas: [], result: failure("rust_client_unavailable")) }

  /// 换上新的回答方式，清掉已经记下的调用。
  static func answer(_ respond: @escaping (Call) -> String) {
    lock.lock(); defer { lock.unlock() }
    self.respond = respond
    recorded = []
  }

  /// 流式对话怎么回答。
  static func streaming(_ stream: @escaping (Call) -> Stream) {
    lock.lock(); defer { lock.unlock() }
    self.stream = stream
  }

  /// 到现在为止的调用，按到达顺序。
  static var calls: [Call] {
    lock.lock(); defer { lock.unlock() }
    return recorded
  }

  /// 某个 action 的调用。
  static func calls(_ operation: String) -> [Call] { calls.filter { $0.operation == operation } }

  static func ok(_ value: Any) -> String {
    let data = try! JSONSerialization.data(withJSONObject: ["ok": true, "value": value], options: [.fragmentsAllowed])
    return String(decoding: data, as: UTF8.self)
  }

  static func failure(_ code: String, retryAfter: Int? = nil) -> String {
    var object: [String: Any] = ["ok": false, "error": code]
    if let retryAfter { object["retry_after_seconds"] = retryAfter }
    let data = try! JSONSerialization.data(withJSONObject: object)
    return String(decoding: data, as: UTF8.self)
  }

  /// 账号页形状的 user。
  static func user(_ id: String, _ name: String = "测试", email: String? = nil) -> [String: Any] {
    var user: [String: Any] = ["id": id, "displayName": name, "createdAt": "2026-09-08"]
    if let email { user["email"] = email }
    return user
  }

  /// 按「入口.操作」给回答的常用写法；没有列出的是 `rust_client_unavailable`。`status` 默认按 `signedIn` 回答。
  static func replies(_ table: [String: String], signedIn: String? = "user-1") {
    answer { call in
      let key = "\(call.entry).\(call.operation ?? "")"
      if let reply = table[key] { return reply }
      if key == "account.status" { return ok(["user": signedIn.map { user($0) as Any } ?? NSNull()]) }
      return failure("rust_client_unavailable")
    }
  }

  private static func decode(_ bytes: UnsafePointer<UInt8>?, _ length: Int) -> [String: Any] {
    let data = bytes.map { Data(bytes: $0, count: length) } ?? Data()
    return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
  }

  fileprivate static func handle(_ entry: String, _ bytes: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>? {
    let call = Call(entry: entry, request: decode(bytes, length))
    lock.lock()
    recorded.append(call)
    let respond = self.respond
    lock.unlock()
    return strdup(respond(call))
  }

  fileprivate static func handleStream(_ bytes: UnsafePointer<UInt8>?, _ length: Int,
                                       _ onDelta: msime_client_account_chat_delta_callback?,
                                       _ context: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    let call = Call(entry: "chat_stream", request: decode(bytes, length))
    lock.lock()
    recorded.append(call)
    let stream = self.stream
    lock.unlock()
    let reply = stream(call)
    for delta in reply.deltas {
      let bytes = Array(delta.utf8)
      bytes.withUnsafeBufferPointer { onDelta?(context, $0.baseAddress, $0.count) }
    }
    return strdup(reply.result)
  }

  fileprivate static func handleSkin(_ bytes: UnsafePointer<UInt8>?, _ length: Int,
                                     _ progress: msime_client_ai_skin_progress_callback?,
                                     _ context: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    let call = Call(entry: "ai_skin_generate", request: decode(bytes, length))
    lock.lock()
    recorded.append(call)
    let respond = self.respond
    lock.unlock()
    for completed in 1...3 { progress?(context, UInt32(completed)) }
    return strdup(respond(call))
  }

  static func table() -> msime_rust_client {
    var table = msime_rust_client()
    table.version = MSIME_RUST_CLIENT_VERSION
    table.size = UInt32(MemoryLayout<msime_rust_client>.stride)
    table.string_free = { free($0) }
    table.session_store_register = { _ in strdup(#"{"ok":true,"value":{"session_store":7}}"#) }
    table.session_store_unregister = { _ in strdup(#"{"ok":true,"value":true}"#) }
    table.client_agent = { FakeRust.handle("client_agent", $0, $1) }
    table.account = { FakeRust.handle("account", $0, $1) }
    table.account_chat_stream = { FakeRust.handleStream($0, $1, $2, $3) }
    table.account_chat_cancel = { FakeRust.handle("chat_cancel", $0, $1) }
    table.account_gloss = { FakeRust.handle("account_gloss", $0, $1) }
    table.account_cloud_clipboard = { FakeRust.handle("cloud_clipboard", $0, $1) }
    table.community = { FakeRust.handle("community", $0, $1) }
    table.cloud_dictionary = { FakeRust.handle("cloud_dictionary", $0, $1) }
    table.ai_skin_generate = { FakeRust.handleSkin($0, $1, $2, $3) }
    table.ai_skin_cancel = { FakeRust.handle("ai_skin_cancel", $0, $1) }
    table.ai_skin_job = { FakeRust.handle("ai_skin_job", $0, $1) }
    table.cloud_sync = { FakeRust.handle("cloud_sync", $0, $1) }
    table.account_ios_settings_export = { FakeRust.handle("ios_settings_export", $0, $1) }
    table.account_ios_settings_apply = { FakeRust.handle("ios_settings_apply", $0, $1) }
    return table
  }

  /// 注册在 7 号存储上的账号，与替身注册给出的 id 相同。
  static var account: BackendRustAccount {
    BackendRustAccount(client: BackendRustClient(table: table())!, store: 7)
  }
}

// 替身 Rust：macOS 后端的 Swift 测试经它代替 `msime_rust_client_linked()` 的函数表，不联网、不碰钥匙串和状态目录。C 回调不能捕获变量，所以每个入口都转到 `FakeRust.handler`，测试按入口和 action 给出回答。只在一个测试进程里串行使用。
import CMSIMEClient
import Foundation

enum FakeRust {
  /// 一次调用：入口名（`account`、`cloud_clipboard`、`cloud_dictionary`、`community`、`client_agent`）和整个请求。
  struct Call {
    let entry: String
    let request: [String: Any]
    /// 请求里的 action；`client_agent` 没有 action，是空的。
    var action: [String: Any] { request["action"] as? [String: Any] ?? [:] }
    var operation: String? { action["operation"] as? String }
  }

  private static let lock = NSLock()
  private static var recorded: [Call] = []
  private static var respond: (Call) -> String = { _ in failure("rust_client_unavailable") }

  /// 换上新的回答方式，清掉已经记下的调用。
  static func answer(_ respond: @escaping (Call) -> String) {
    lock.lock(); defer { lock.unlock() }
    self.respond = respond
    recorded = []
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

  static func failure(_ code: String) -> String {
    let data = try! JSONSerialization.data(withJSONObject: ["ok": false, "error": code])
    return String(decoding: data, as: UTF8.self)
  }

  /// 账号页形状的 user。
  static func user(_ id: String, _ name: String = "测试") -> [String: Any] {
    ["id": id, "displayName": name, "createdAt": "2026-09-08"]
  }

  fileprivate static func handle(_ entry: String, _ bytes: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>? {
    let data = bytes.map { Data(bytes: $0, count: length) } ?? Data()
    let request = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    let call = Call(entry: entry, request: request)
    lock.lock()
    recorded.append(call)
    let respond = self.respond
    lock.unlock()
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
    table.account_cloud_clipboard = { FakeRust.handle("cloud_clipboard", $0, $1) }
    table.cloud_dictionary = { FakeRust.handle("cloud_dictionary", $0, $1) }
    table.community = { FakeRust.handle("community", $0, $1) }
    return table
  }

  /// 注册在 7 号存储上的账号，与替身注册给出的 id 相同。
  static var account: BackendRustAccount {
    BackendRustAccount(client: BackendRustClient(table: table())!, store: 7)
  }
}

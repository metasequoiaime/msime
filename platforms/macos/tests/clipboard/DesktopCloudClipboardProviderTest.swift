// Tauri 云剪贴板面板的 IPC provider 经 Rust（#8201 迁移计划 4e）：动作原样交给 `msime_client_account_cloud_clipboard`，带设备名、按打开面板时的账号绑定，错误码交给面板。Rust 是替身函数表（tests/core/FakeRustTable.swift）。
import Foundation

private func check(_ condition: @autoclosure () -> Bool, _ message: String, line: UInt = #line) {
  if !condition() {
    FileHandle.standardError.write(Data("DesktopCloudClipboardProviderTest:\(line): \(message)\n".utf8))
    exit(1)
  }
}

/// 替身 Rust 的回答：账号是 `user`（nil 是没有登录），云剪贴板的动作按 `failure` 失败或给出形状正确的结果。
private final class Cloud: @unchecked Sendable {
  var user: String? = "synthetic-user"
  var failure: String?
  static let devices: [String: Any] = ["enabled": true, "status": "ready", "device_id": String(repeating: "a", count: 32),
    "code": "0421 9873 5530 1176", "devices": [
      ["device_id": String(repeating: "a", count: 32), "name": "合成 Mac", "created_at": "2026-10-11T08:00:00Z", "approved": true, "current": true, "code": "0421 9873 5530 1176"],
      ["device_id": String(repeating: "b", count: 32), "name": "合成 iPhone", "created_at": "2026-10-11T09:00:00Z", "approved": false, "current": false, "code": "1111 2222 3333 4444"],
    ]]
  func respond(_ call: FakeRust.Call) -> String {
    if call.entry == "account" { return FakeRust.ok(["user": user.map { FakeRust.user($0) as Any } ?? NSNull()]) }
    if let failure { return FakeRust.failure(failure) }
    switch call.operation {
    case "list":
      return FakeRust.ok(["enabled": true, "retention_days": 7, "items": [["id": String(repeating: "a", count: 64), "text": "synthetic\n\t合成",
        "updated_at": "synthetic-time", "pinned": true, "device": "合成 iPhone"]],
        "e2e": ["status": "ready", "code": "0421 9873 5530 1176", "pending_devices": 1, "undecryptable": 0]])
    case "add": return FakeRust.ok(["id": String(repeating: "a", count: 64), "text": call.action["text"] ?? "", "updated_at": "synthetic-time"])
    case "delete": return FakeRust.ok([String: Any]())
    case "set_enabled": return FakeRust.ok(["enabled": call.action["enabled"] ?? false])
    default: return FakeRust.ok(Self.devices)
    }
  }
}

@MainActor private func reply(_ provider: BackendCloudClipboardProvider, _ request: NSDictionary, cancel: Bool = false) async -> NSDictionary {
  await withCheckedContinuation { continuation in
    let progress = provider.request(request, completion: { continuation.resume(returning: $0) })
    if cancel { progress.cancel() }
  }
}

@main enum DesktopCloudClipboardProviderTest {
  @MainActor static func main() async throws {
    let cloud = Cloud()
    FakeRust.answer(cloud.respond)
    func sent() -> [FakeRust.Call] { FakeRust.calls.filter { $0.entry == "cloud_clipboard" } }
    let provider = BackendCloudClipboardProvider(rust: FakeRust.account, accountID: "synthetic-user", deviceName: "合成 Mac")
    let page = try await provider.execute(["operation": "list", "search": "合成"])
    // 列表原样转发 Rust 的形状：条目带置顶和设备名，另有保留期和端到端加密状态。
    let listed = (page["items"] as? [[String: Any]])?.first
    check(listed?["pinned"] as? Bool == true && listed?["device"] as? String == "合成 iPhone", "list shape \(page)")
    check(page["retention_days"] as? Int == 7 && (page["e2e"] as? [String: Any])?["pending_devices"] as? Int == 1, "list e2e \(page)")
    // 动作原样交给 Rust，带注册给出的存储 id 和本机的设备名，不带目录；请求前后各核对一次账号。
    let first = sent().first
    check(first?.action["operation"] as? String == "list" && first?.action["search"] as? String == "合成"
          && first?.request["session_store"] as? Int == 7 && first?.request["device_name"] as? String == "合成 Mac"
          && first?.request["directory"] == nil, "forwarded \(String(describing: first?.request))")
    check(FakeRust.calls.map(\.entry) == ["account", "cloud_clipboard", "account"], "binding \(FakeRust.calls.map(\.entry))")
    let text = String(repeating: "界", count: 3997) + "\n\r\t"
    _ = try await provider.execute(["operation": "add", "text": text])
    check(sent().last?.action["text"] as? String == text, "add text")
    _ = try await provider.execute(["operation": "delete", "id": String(repeating: "a", count: 64)])
    _ = try await provider.execute(["operation": "set_enabled", "enabled": false])
    check(sent().last?.action["enabled"] as? Bool == false, "set_enabled")
    let other = String(repeating: "b", count: 32)
    for request: NSDictionary in [["operation": "devices"], ["operation": "approve", "device_id": other], ["operation": "reject", "device_id": other],
                                  ["operation": "remove", "device_id": other], ["operation": "reset"]] {
      let status = try await provider.execute(request)
      check(sent().last?.action["operation"] as? String == request["operation"] as? String
            && status["status"] as? String == "ready" && (status["devices"] as? [[String: Any]])?.count == 2, "devices \(request)")
    }
    check(sent().count == 9, "forwarded \(sent().count)")
    // 共享面板在 macOS 上没有置顶、保留期和清空的入口，宿主不放行；不认识的动作也一样。都不发请求。
    for request: NSDictionary in [["operation": "pin", "id": String(repeating: "a", count: 64), "pinned": true], ["operation": "set_retention", "days": 7],
                                  ["operation": "clear"], ["operation": "token"], [:]] {
      let rejected = await reply(provider, request)
      check(rejected["ok"] as? Bool == false && rejected["error"] as? String == "invalid", "\(request) accepted: \(rejected)")
    }
    check(sent().count == 9, "a refused action was sent")
    // Rust 和驱动给出的错误码原样交给面板，参数不合法是 invalid，本地造出的失败是 unavailable。
    cloud.failure = "clipboard_device_pending"
    let pending = await reply(provider, ["operation": "add", "text": "synthetic"])
    check(pending["ok"] as? Bool == false && pending["error"] as? String == "clipboard_device_pending", "pending \(pending)")
    cloud.failure = "invalid_cloud_clipboard"
    let invalid = await reply(provider, ["operation": "approve", "device_id": "bad"])
    check(invalid["error"] as? String == "invalid", "invalid \(invalid)")
    cloud.failure = nil
    let unavailable = BackendCloudClipboardProvider(rust: BackendRustAccount(client: BackendRustClient(table: {
      var table = FakeRust.table(); table.account_cloud_clipboard = nil; return table }())!, store: 7), accountID: "synthetic-user", deviceName: "合成 Mac")
    let missing = await reply(unavailable, ["operation": "list", "search": ""])
    check(missing["error"] as? String == "unavailable", "missing entry \(missing)")
    // 打开面板以后换了号或退出登录：不发请求，回答 account_unauthorized；请求在路上时换了号，回答不交给面板。
    let before = sent().count
    cloud.user = "another-user"
    let switched = await reply(provider, ["operation": "add", "text": "synthetic"])
    check(switched["error"] as? String == "account_unauthorized" && sent().count == before, "switched \(switched)")
    cloud.user = nil
    let signedOut = await reply(provider, ["operation": "list", "search": ""])
    check(signedOut["error"] as? String == "account_unauthorized" && sent().count == before, "signed out \(signedOut)")
    cloud.user = "synthetic-user"
    FakeRust.answer { call in
      // 列表发出之后、回答之前换了号。
      if call.entry == "cloud_clipboard" { cloud.user = "another-user" }
      return cloud.respond(call)
    }
    let raced = await reply(provider, ["operation": "list", "search": ""])
    check(raced["error"] as? String == "account_unauthorized", "an old account's list reached the panel: \(raced)")
    cloud.user = "synthetic-user"
    FakeRust.answer(cloud.respond)
    // 调用方在开始之前就取消：什么都不发。
    let cancelled = await reply(provider, ["operation": "add", "text": "synthetic"], cancel: true)
    check(cancelled["ok"] as? Bool == false && cancelled["error"] as? String == "unavailable" && sent().isEmpty, "cancelled \(cancelled)")

    // 表情面板「发到云剪贴板」：开关关着或本机还在等待批准时 Rust 不上传，宿主按码给出提示。
    let delivered = await provider.send("synthetic\n合成")
    check(delivered == .sent && sent().last?.action["text"] as? String == "synthetic\n合成", "send")
    cloud.failure = "clipboard_sync_disabled"
    let disabled = await provider.send("synthetic")
    check(disabled == .disabled, "disabled")
    cloud.failure = "clipboard_device_pending"
    let waiting = await provider.send("synthetic")
    check(waiting == .pending, "waiting")
    cloud.failure = "account_network"
    let failed = await provider.send("synthetic")
    check(failed == .failed, "failed")
    check(BackendCloudClipboardProvider.SendOutcome.signedOut.message == "登录水杉账号后可在设备间同步剪贴板", "signed out message")
    check(BackendCloudClipboardProvider.SendOutcome.pending.message.contains("等待批准"), "pending message")
    print("DesktopCloudClipboardProviderTest: ok")
  }
}

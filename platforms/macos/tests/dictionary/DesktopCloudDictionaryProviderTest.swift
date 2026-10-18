// Tauri 云词库面板的 IPC provider 经 Rust（#8201 迁移计划 4e）：账号云词库的动作原样交给 `msime_client_cloud_dictionary`，导出写进只有本人能读写的 `msime-export-<UUID>` 目录，整份快照的文件面板和预览令牌留在宿主里。Rust 是替身函数表（tests/core/FakeRustTable.swift）。
import Foundation

private func check(_ condition: @autoclosure () -> Bool, _ message: String, line: UInt = #line) {
  if !condition() {
    FileHandle.standardError.write(Data("DesktopCloudDictionaryProviderTest:\(line): \(message)\n".utf8))
    exit(1)
  }
}

private final class Cloud: @unchecked Sendable {
  var user: String? = "synthetic-user"
  var failure: String?
  var queued = "queued"
  static let snapshot: [String: Any] = ["cloudRevision": 5, "sha256": String(repeating: "e", count: 64), "bytes": 120, "records": 7,
                                        "entries": 3, "overlays": 2, "positions": 1, "selections": 4]
  func respond(_ call: FakeRust.Call) -> String {
    if call.entry == "account" { return FakeRust.ok(["user": user.map { FakeRust.user($0) as Any } ?? NSNull()]) }
    if let failure { return FakeRust.failure(failure) }
    let request: [String: Any] = ["id": "synthetic-request", "cloudRevision": 5, "fileSha256": String(repeating: "f", count: 64), "status": queued]
    switch call.operation {
    case "list": return FakeRust.ok(["entries": [], "has_more": false, "offset": call.action["offset"] ?? 0])
    case "catalog": return FakeRust.ok(["catalog_entries": [["kind": "pinyin", "code": "he'cheng", "word": "合成", "weight": 100]], "has_more": true,
                                        "offset": 0, "revision": 3, "normalized": "he'cheng"])
    case "rank": return FakeRust.ok(["revision": 4, "changed": true, "selection_count": 2])
    case "export_file":
      guard let path = call.action["path"] as? String else { return FakeRust.failure("invalid_cloud_dictionary") }
      // 与 Rust 相同：经私有的临时文件换进去，只有本人能读写。
      FileManager.default.createFile(atPath: path, contents: Data("合成\the'cheng\t100\n".utf8), attributes: [.posixPermissions: 0o600])
      return FakeRust.ok(["path": path, "filename": "dictionary-pinyin.tsv", "bytes": 19])
    case "snapshot_status":
      // 处理排队中的请求要等输入会话放开词库，与真的 Rust 一样占住调用线程一会儿。
      usleep(300_000)
      return FakeRust.ok(["localVersion": "local-1", "request": request])
    case "snapshot_cancel": return FakeRust.ok(["localVersion": "local-1", "request": NSNull()])
    case "snapshot_preview": return FakeRust.ok(["previewToken": "preview-1", "snapshot": Self.snapshot, "localVersion": "local-1"])
    case "snapshot_enqueue":
      usleep(300_000)
      return FakeRust.ok(["localVersion": "local-1", "request": request])
    case "snapshot_restore_preview_file": return FakeRust.ok(["snapshot": Self.snapshot, "expectedRevision": 5])
    case "snapshot_restore_file": return FakeRust.ok(["revision": 6, "reset": true])
    case "snapshot_export_file":
      guard let path = call.action["path"] as? String else { return FakeRust.failure("account_invalid") }
      FileManager.default.createFile(atPath: path, contents: Data("{}\n".utf8))
      return FakeRust.ok(["path": path, "filename": "msime-dictionary-snapshot.ndjson", "snapshot": Self.snapshot])
    default: return FakeRust.failure("invalid_cloud_dictionary")
    }
  }
}

@MainActor private func reply(_ provider: BackendCloudDictionaryProvider, _ request: NSDictionary) async -> NSDictionary {
  await withCheckedContinuation { continuation in
    _ = provider.request(request, completion: { continuation.resume(returning: $0) })
  }
}

private func permissions(_ path: String) -> Int {
  ((try? FileManager.default.attributesOfItem(atPath: path)[.posixPermissions]) as? NSNumber)?.intValue ?? -1
}

@main enum DesktopCloudDictionaryProviderTest {
  @MainActor static func main() async throws {
    let cloud = Cloud()
    FakeRust.answer(cloud.respond)
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-dictionary-provider-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    let transfer = root.appendingPathComponent("transfer", isDirectory: true)
    func sent() -> [FakeRust.Call] { FakeRust.calls.filter { $0.entry == "cloud_dictionary" } }

    let exports = root.appendingPathComponent("exports", isDirectory: true)
    try FileManager.default.createDirectory(at: exports, withIntermediateDirectories: false)
    var provider: BackendCloudDictionaryProvider? = BackendCloudDictionaryProvider(rust: FakeRust.account, accountID: "synthetic-user",
                                                                                  transfer: { transfer }, exports: exports)
    // 账号云词库的动作原样交给 Rust，带存储 id 和私有中转目录，不带目录模式的 directory；请求前后各核对一次账号。
    let catalog = try await provider!.execute(["operation": "catalog", "kind": "pinyin", "code": "heig", "offset": 0, "scheme": "shuangpin", "profile": "xiaohe"])
    let first = sent().first
    check(first?.action["code"] as? String == "heig" && first?.action["profile"] as? String == "xiaohe" && first?.request["session_store"] as? Int == 7
          && first?.request["snapshot_directory"] as? String == transfer.path && first?.request["directory"] == nil, "forwarded \(String(describing: first?.request))")
    check((catalog["catalog_entries"] as? [[String: Any]])?.first?["word"] as? String == "合成" && catalog["revision"] as? Int == 3, "catalog \(catalog)")
    check(FakeRust.calls.map(\.entry) == ["account", "cloud_dictionary", "account"], "binding \(FakeRust.calls.map(\.entry))")
    let ranked = try await provider!.execute(["operation": "rank", "text": "heig", "kind": "pinyin", "scheme": "shuangpin", "profile": "xiaohe", "limit": 10,
                                              "code": "he'cheng", "word": "合成", "revision": 3, "mode": "linear", "linear_step": 3, "trigger_count": 2, "force_top": true])
    check(ranked["selection_count"] as? Int == 2 && sent().last?.action["force_top"] as? Bool == true, "rank \(ranked)")
    // 只在 C 上有的文件动作和面板不发的动作不放行：路径只由宿主定。都不发请求。
    let count = sent().count
    for request: NSDictionary in [["operation": "export_file", "kind": "pinyin", "format": "standard", "path": "/tmp/x.tsv"], ["operation": "changes", "after": 0, "limit": 1],
                                  ["operation": "snapshot_export_file", "path": "/tmp/x"], ["operation": "export", "kind": "../x", "format": "standard"], [:]] {
      let rejected = await reply(provider!, request)
      check(rejected["ok"] as? Bool == false && rejected["error"] as? String == "unavailable", "\(request) accepted: \(rejected)")
    }
    check(sent().count == count, "a refused action was sent")

    // 导出：Rust 写进一个只有本人能读写的 msime-export-<UUID> 目录，面板只拿到路径和字节数；下一次导出删掉上一次的目录。
    let exported = try await provider!.execute(["operation": "export", "kind": "pinyin", "format": "standard"])
    let file = (exported["export_file"] as? [String: Any])?["path"] as? String ?? ""
    let directory = (file as NSString).deletingLastPathComponent
    let name = (directory as NSString).lastPathComponent
    check(exported["filename"] as? String == "dictionary-pinyin.tsv" && (exported["export_file"] as? [String: Any])?["bytes"] as? Int == 19
          && sent().last?.action["operation"] as? String == "export_file" && sent().last?.action["path"] as? String == file
          && sent().last?.action["format"] as? String == "standard", "export \(exported)")
    check(name.hasPrefix("msime-export-") && UUID(uuidString: String(name.dropFirst("msime-export-".count))) != nil
          && permissions(directory) == 0o700 && permissions(file) == 0o600, "export directory \(directory)")
    let again = try await provider!.execute(["operation": "export", "kind": "pinyin", "format": "windows"])
    let second = (((again["export_file"] as? [String: Any])?["path"] as? String ?? "") as NSString).deletingLastPathComponent
    check(!FileManager.default.fileExists(atPath: directory) && FileManager.default.fileExists(atPath: second), "previous export kept")
    // 导出失败时不留下目录。
    cloud.failure = "account_network"
    let failedExport = await reply(provider!, ["operation": "export", "kind": "pinyin", "format": "standard"])
    let leftovers = try FileManager.default.contentsOfDirectory(atPath: exports.path).filter { exports.appendingPathComponent($0).path != second }
    check(failedExport["error"] as? String == "unavailable" && leftovers.isEmpty, "failed export \(failedExport) left \(leftovers)")
    cloud.failure = nil
    provider = nil
    check(!FileManager.default.fileExists(atPath: second), "export survived the provider")

    // 冲突（云端或本机词库已经变了、快照正忙）交给面板是 conflict，其余是 unavailable。
    let errors = BackendCloudDictionaryProvider(rust: FakeRust.account, accountID: "synthetic-user", transfer: { transfer })
    for (code, expected) in [("account_conflict", "conflict"), ("snapshot_busy", "conflict"), ("snapshot_conflict", "conflict"),
                             ("account_unauthorized", "unavailable"), ("account_network", "unavailable")] {
      cloud.failure = code
      let failed = await reply(errors, ["operation": "list", "kind": "pinyin", "offset": 0, "search": ""])
      check(failed["error"] as? String == expected, "\(code) became \(failed)")
    }
    cloud.failure = nil
    // 打开面板以后换了号：不发请求。
    cloud.user = "another-user"
    let before = sent().count
    let switched = await reply(errors, ["operation": "list", "kind": "pinyin", "offset": 0, "search": ""])
    check(switched["ok"] as? Bool == false && sent().count == before, "switched account sent \(switched)")
    cloud.user = "synthetic-user"

    // 整份快照：状态和入列带本机的 HostOptions，处理期间叫醒本进程里握着词库的输入会话。
    var chosen: [URL?] = []
    let options: [String: Any] = ["user_data": "/synthetic/state/user"]
    let snapshots = BackendDesktopSnapshots(rust: FakeRust.account, accountID: "synthetic-user", transfer: { transfer },
                                            choose: { _ in chosen.isEmpty ? nil : chosen.removeFirst() }, hostOptions: { options })
    var wakes = 0
    let observer = NotificationCenter.default.addObserver(forName: Notification.Name("MSIMEDictionaryMaintenanceWillBeginNotification"),
                                                          object: nil, queue: nil) { _ in wakes += 1 }
    defer { NotificationCenter.default.removeObserver(observer) }
    let status = try await snapshots.execute(["operation": "snapshot_status"])
    check(status["nativeFiles"] as? Bool == true && status["localVersion"] as? String == "local-1"
          && (sent().last?.request["options"] as? [String: String])?["user_data"] == "/synthetic/state/user" && wakes > 0, "status \(status)")
    let preview = try await snapshots.execute(["operation": "snapshot_preview"])
    check(preview["previewToken"] as? String == "preview-1", "preview \(preview)")
    wakes = 0
    let queued = try await snapshots.execute(["operation": "snapshot_enqueue", "token": "preview-1"])
    check((queued["request"] as? [String: Any])?["status"] as? String == "queued" && sent().last?.action["token"] as? String == "preview-1" && wakes > 0, "enqueue \(queued)")
    // 恢复：宿主弹面板选文件，网页只拿到令牌；确认时按预览给出的摘要和修订号恢复，令牌只能用一次。
    let backup = root.appendingPathComponent("backup.ndjson")
    FileManager.default.createFile(atPath: backup.path, contents: Data("{}\n".utf8))
    let cancelledChoice = try await snapshots.execute(["operation": "snapshot_choose_restore"])
    check(cancelledChoice["saved"] as? Bool == false, "cancelled choice \(cancelledChoice)")
    chosen = [backup]
    let restorePreview = try await snapshots.execute(["operation": "snapshot_choose_restore"])
    let token = restorePreview["previewToken"] as? String ?? ""
    check(!token.isEmpty && restorePreview["expectedRevision"] as? Int == 5 && sent().last?.action["path"] as? String == backup.path, "restore preview \(restorePreview)")
    let restored = try await snapshots.execute(["operation": "snapshot_restore_native", "token": token])
    let restoreAction = sent().last?.action
    check(restored["revision"] as? Int == 6 && restoreAction?["path"] as? String == backup.path
          && restoreAction?["expected_sha256"] as? String == String(repeating: "e", count: 64) && restoreAction?["revision"] as? Int == 5, "restore \(restored)")
    do { _ = try await snapshots.execute(["operation": "snapshot_restore_native", "token": token]); check(false, "a used restore token was accepted") } catch { }
    chosen = [backup]
    let discarded = try await snapshots.execute(["operation": "snapshot_choose_restore"])
    _ = try await snapshots.execute(["operation": "snapshot_restore_cancel"])
    do { _ = try await snapshots.execute(["operation": "snapshot_restore_native", "token": discarded["previewToken"] as? String ?? ""]); check(false, "a cancelled restore was sent") } catch { }
    // 备份：宿主弹面板选位置，Rust 校验完换进去；取消时不发请求。
    let saves = sent().count
    let cancelledSave = try await snapshots.execute(["operation": "snapshot_export"])
    check(cancelledSave["cancelled"] as? Bool == true && sent().count == saves, "cancelled save \(cancelledSave)")
    let destination = root.appendingPathComponent("saved.ndjson")
    chosen = [destination]
    let saved = try await snapshots.execute(["operation": "snapshot_export"])
    check(saved["saved"] as? Bool == true && sent().last?.action["path"] as? String == destination.path, "save \(saved)")
    print("DesktopCloudDictionaryProviderTest: ok")
  }
}

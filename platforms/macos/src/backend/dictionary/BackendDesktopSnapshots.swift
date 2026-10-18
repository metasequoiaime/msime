import AppKit
import Foundation

@MainActor final class DesktopSnapshotFileDialogs {
  func choose(saving: Bool) async throws -> URL? {
    let panel: NSSavePanel = saving ? NSSavePanel() : NSOpenPanel()
    if let open = panel as? NSOpenPanel { open.allowsMultipleSelection = false; open.canChooseDirectories = false }
    panel.nameFieldStringValue = "msime-dictionary-snapshot.ndjson"
    return try await withTaskCancellationHandler(operation: {
      try Task.checkCancellation()
      let result: URL? = await withCheckedContinuation { continuation in
        panel.begin { response in continuation.resume(returning: response == .OK ? panel.url : nil) }
      }
      try Task.checkCancellation()
      return result
    }, onCancel: { Task { @MainActor in panel.cancel(nil) } })
  }
}

/// Tauri 云词库面板的整份快照（`snapshotNative`）：网页只看到元数据和一次性的预览令牌，快照内容、路径和令牌都留在宿主里。下载、备份、恢复和「应用到本机」都经 Rust 的 `msime_client_cloud_dictionary`（#8201 迁移计划 4e）：保存和选文件由宿主弹面板，路径交给 `snapshot_export_file`、`snapshot_restore_preview_file`、`snapshot_restore_file`；「应用到本机」排进 Rust 的快照队列，与账号窗口的「云词库同步与备份」相同（`MacSnapshotModel`）。
@MainActor final class BackendDesktopSnapshots {
  private let rust: BackendRustAccount
  private let accountID: String
  private let transfer: () throws -> URL
  private let choose: (Bool) async throws -> URL?
  private let hostOptions: @MainActor () throws -> [String: Any]
  /// 面板确认恢复之前留着的那份备份：路径、预览给出的摘要和云端修订号，网页只拿到令牌。
  private var restore: (token: String, file: URL, sha256: String, revision: Int64)?
  init(rust: BackendRustAccount, accountID: String, transfer: @escaping () throws -> URL = MacCloudTransferDirectory.url,
       choose: @escaping (Bool) async throws -> URL? = { try await DesktopSnapshotFileDialogs().choose(saving: $0) },
       hostOptions: @escaping @MainActor () throws -> [String: Any] = MacLocalDictionary.hostOptions) {
    self.rust = rust; self.accountID = accountID; self.transfer = transfer; self.choose = choose; self.hostOptions = hostOptions
  }

  private func requireAccount() async throws {
    guard try await rust.status()?.id == accountID else { throw BackendRustClient.Failure(code: "account_unauthorized") }
    try Task.checkCancellation()
  }
  private func request(_ action: [String: Any], options: [String: Any]? = nil) async throws -> [String: Any] {
    try BackendRustAccount.object(try await rust.cloudDictionary(action, snapshotDirectory: try transfer(), options: options))
  }
  /// 队列状态里只有页面要的 `localVersion` 和 `request`，另带 `nativeFiles` 说明备份与恢复走宿主的文件面板。
  private func queue(_ result: [String: Any]) -> [String: Any] {
    var value = result
    value["nativeFiles"] = true
    return value
  }

  func execute(_ request: NSDictionary) async throws -> [String: Any] {
    guard let operation = request["operation"] as? String else { throw BackendRustClient.Failure(code: "invalid_cloud_dictionary") }
    if operation == "snapshot_restore_cancel" {
      restore = nil
      return ["cancelled": true]
    }
    if operation == "snapshot_discard" {
      // 本机替换的预览由 Rust 保管，换号、下一次预览或入列时作废；这里只丢掉恢复的那份。
      if let token = request["token"] as? String, token == restore?.token { restore = nil }
      return ["discarded": true]
    }
    try await requireAccount()
    let result: [String: Any]
    switch operation {
    case "snapshot_status":
      // 面板每两秒问一次状态，Rust 借这次接着处理排队中的请求（最多每 60 秒一次），所以同样要叫醒本进程的输入会话。
      let options = try hostOptions()
      result = queue(try await MacLocalDictionary.wakingInputSessions {
        try await self.request(["operation": "snapshot_status"], options: options)
      })
    case "snapshot_cancel":
      result = queue(try await self.request(["operation": "snapshot_cancel"], options: try hostOptions()))
    case "snapshot_preview":
      result = try await self.request(["operation": "snapshot_preview"], options: try hostOptions())
    case "snapshot_enqueue":
      guard let token = request["token"] as? String else { throw BackendRustClient.Failure(code: "invalid_cloud_dictionary") }
      let options = try hostOptions()
      result = queue(try await MacLocalDictionary.wakingInputSessions {
        try await self.request(["operation": "snapshot_enqueue", "token": token], options: options)
      })
    case "snapshot_restore_preview", "snapshot_choose_restore":
      restore = nil
      guard let source = try await choose(false) else { return ["saved": false] }
      let scoped = source.startAccessingSecurityScopedResource()
      defer { if scoped { source.stopAccessingSecurityScopedResource() } }
      let preview = try await self.request(["operation": "snapshot_restore_preview_file", "path": source.path])
      guard let snapshot = preview["snapshot"] as? [String: Any], let sha256 = snapshot["sha256"] as? String,
            let revision = preview["expectedRevision"] as? NSNumber else { throw BackendRustClient.Failure.invalidResponse }
      let token = UUID().uuidString
      restore = (token, source, sha256, revision.int64Value)
      result = ["previewToken": token, "snapshot": snapshot, "expectedRevision": revision]
    case "snapshot_export", "snapshot_save":
      guard let destination = try await choose(true) else { return ["cancelled": true] }
      let scoped = destination.startAccessingSecurityScopedResource()
      defer { if scoped { destination.stopAccessingSecurityScopedResource() } }
      _ = try await self.request(["operation": "snapshot_export_file", "path": destination.path])
      result = ["saved": true]
    case "snapshot_restore_native", "snapshot_restore", "snapshot_restore_prepared":
      guard let selected = restore, request["token"] as? String == selected.token else { throw BackendRustClient.Failure(code: "invalid_cloud_dictionary") }
      // 发出去之前就作废：拿不准结果的恢复不会被再发一次。
      restore = nil
      let scoped = selected.file.startAccessingSecurityScopedResource()
      defer { if scoped { selected.file.stopAccessingSecurityScopedResource() } }
      let restored = try await self.request(["operation": "snapshot_restore_file", "path": selected.file.path,
                                             "expected_sha256": selected.sha256, "revision": selected.revision])
      result = ["revision": restored["revision"] ?? NSNull()]
    default: throw BackendRustClient.Failure(code: "invalid_cloud_dictionary")
    }
    try await requireAccount()
    return result
  }
}

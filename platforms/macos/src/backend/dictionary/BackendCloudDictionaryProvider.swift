import Foundation

/// 给 Tauri 云词库面板的 IPC 服务端（`DesktopCloudClipboard.mm` 的词库那一路）做事的一方：一个面板只能操作打开它时登录的那个账号。动作交给 Rust 的 `msime_client_cloud_dictionary`（#8201 迁移计划 4e），令牌不出 Rust；跨过 IPC 的只有词库内容和一份私有的导出文件描述。
@MainActor @objc(MSIMEBackendCloudDictionaryProvider)
final class BackendCloudDictionaryProvider: NSObject {
  /// 面板能发的账号云词库动作，与原来的 Swift provider 相同；`snapshot_*` 交给 `BackendDesktopSnapshots`。`changes` 和 Rust 只在 C 上有的 `export_file` 等文件动作不放行：路径只由宿主自己定。
  static let operations: Set<String> = ["catalog", "edit_catalog", "candidates", "rank", "remove_candidate", "fixed_positions",
                                        "set_fixed_position", "list", "add", "update", "delete", "import", "export"]
  private static let kinds: Set<String> = ["pinyin", "wubi", "wubi98", "quick", "english"]
  private let rust: BackendRustAccount
  private let accountID: String
  private let transfer: () throws -> URL
  /// 放导出目录的地方，`crates/host-macos` 的 `read_export` 只认它下面的 `msime-export-<UUID>`。
  private let exports: URL
  private lazy var snapshots = BackendDesktopSnapshots(rust: rust, accountID: accountID, transfer: transfer)
  /// 上一次导出的私有目录，面板读完以后下一次导出或 provider 释放时删掉。
  private var exported: URL?
  init(rust: BackendRustAccount, accountID: String, transfer: @escaping () throws -> URL = MacCloudTransferDirectory.url,
       exports: URL = FileManager.default.temporaryDirectory) {
    self.rust = rust; self.accountID = accountID; self.transfer = transfer; self.exports = exports
  }
  deinit { if let exported { try? FileManager.default.removeItem(at: exported) } }

  /// 为当前登录的账号准备一个 provider；宿主没有注册会话存储、没有登录或读不了登录状态时是 nil，输入法随后回退到原生窗口。
  @objc static func prepare(completion: @escaping (BackendCloudDictionaryProvider?) -> Void) {
    Task {
      guard let rust = BackendRustAccount.shared, let user = (try? await rust.status()) ?? nil else { completion(nil); return }
      completion(BackendCloudDictionaryProvider(rust: rust, accountID: user.id))
    }
  }

  /// 打开面板时的账号还登录着才继续。C 上拿不到会话代次，只按 user.id 绑定。
  private func requireAccount() async throws {
    guard try await rust.status()?.id == accountID else { throw BackendRustClient.Failure(code: "account_unauthorized") }
    try Task.checkCancellation()
  }

  func execute(_ request: NSDictionary) async throws -> [String: Any] {
    guard let operation = request["operation"] as? String else { throw BackendRustClient.Failure(code: "invalid_cloud_dictionary") }
    if operation.hasPrefix("snapshot_") { return try await snapshots.execute(request) }
    guard Self.operations.contains(operation), var action = request as? [String: Any], JSONSerialization.isValidJSONObject(action) else {
      throw BackendRustClient.Failure(code: "invalid_cloud_dictionary")
    }
    try await requireAccount()
    guard operation == "export" else {
      let result = try BackendRustAccount.object(try await rust.cloudDictionary(action, snapshotDirectory: try transfer()))
      try await requireAccount()
      return result
    }
    // 导出的正文最多 384 MiB，不进 IPC：Rust 写进一个只有本人能读写的 `msime-export-<UUID>` 目录（`export_file`），面板只拿到路径和字节数，由 Tauri 一侧按 `crates/host-macos` 的 `read_export` 核对目录、文件名、权限和大小后读出。
    guard let kind = action["kind"] as? String, Self.kinds.contains(kind) else { throw BackendRustClient.Failure(code: "invalid_cloud_dictionary") }
    let directory = exports.appendingPathComponent("msime-export-" + UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    var kept = false
    defer { if !kept { try? FileManager.default.removeItem(at: directory) } }
    let file = directory.appendingPathComponent("dictionary-" + kind + ".tsv", isDirectory: false)
    action["operation"] = "export_file"
    action["path"] = file.path
    let result = try BackendRustAccount.object(try await rust.cloudDictionary(action, snapshotDirectory: try transfer()))
    try await requireAccount()
    guard let bytes = result["bytes"] as? NSNumber, bytes.int64Value <= 384 * 1024 * 1024 else { throw BackendRustClient.Failure.invalidResponse }
    if let exported { try? FileManager.default.removeItem(at: exported) }
    exported = directory; kept = true
    return ["export_file": ["path": file.path, "bytes": bytes], "filename": file.lastPathComponent]
  }

  /// 失败时交给面板的错误：冲突（云端或本机词库已经变了、快照正忙）是 `conflict`，其余都是 `unavailable`，与原来的 Swift provider 相同。
  static func errorReply(_ error: Error) -> NSDictionary {
    let code = (error as? BackendRustClient.Failure)?.code
    return ["ok": false, "error": ["account_conflict", "snapshot_conflict", "snapshot_busy"].contains(code ?? "") ? "conflict" : "unavailable"]
  }

  @objc func request(_ request: NSDictionary, completion: @escaping (NSDictionary) -> Void) -> Progress {
    let progress = Progress(totalUnitCount: 1)
    let task = Task {
      do {
        // Progress.cancel() marks the progress synchronously but invokes its
        // cancellation handler asynchronously. Check that flag before entering
        // execute so a request cancelled before this task is scheduled never
        // fetches credentials or starts network I/O.
        guard !progress.isCancelled else { throw CancellationError() }
        completion(["ok": true, "value": try await execute(request)])
      }
      catch { completion(Self.errorReply(error)) }
      progress.completedUnitCount = 1; progress.cancellationHandler = nil
    }
    progress.cancellationHandler = { task.cancel() }
    return progress
  }
}

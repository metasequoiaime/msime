import Foundation

@MainActor
enum MacCloudFileTransfer {
  static func save(_ file: URL, to destination: URL, authorize: () async throws -> Void) async throws {
    guard file.isFileURL, destination.isFileURL else { throw BackendAccountClient.Failure(status: 400) }
    try await authorize()
    let scoped = destination.startAccessingSecurityScopedResource()
    defer { if scoped { destination.stopAccessingSecurityScopedResource() } }
    let temporary = destination.deletingLastPathComponent().appendingPathComponent("." + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: temporary) }
    let copy = Task.detached(priority: .utility) {
      try Task.checkCancellation()
      try FileManager.default.copyItem(at: file, to: temporary)
      try Task.checkCancellation()
    }
    try await withTaskCancellationHandler(operation: { try await copy.value }, onCancel: { copy.cancel() })
    try await authorize()
    if FileManager.default.fileExists(atPath: destination.path) { _ = try FileManager.default.replaceItemAt(destination, withItemAt: temporary) }
    else { try FileManager.default.moveItem(at: temporary, to: destination) }
  }
}

/// 本进程交给 Rust 云词库（`msime_client_cloud_dictionary`）的私有中转目录 `snapshot_directory`：放整份快照的下载、导出和恢复的中转文件。在 `$TMPDIR` 下按进程号分开，仅本人可读写：Rust 在本进程第一次用到它时清掉上次留下的中转文件，输入法进程和独立设置窗口进程同时运行时不能共用一个。
enum MacCloudTransferDirectory {
  static func url() throws -> URL {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-dictionary-\(getpid())", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    return url
  }
}

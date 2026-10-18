import Foundation

/// 云词库各页发给 Rust 的请求（`msime_client_cloud_dictionary`，#8201 迁移计划 4c）。每个页面只按打开时的 user.id 绑定：C 上拿不到会话代次，同一个账号的令牌轮换和重新登录不作废页面，请求前后都核对，退出或换了号就丢掉结果（`CancellationError`）。
struct IOSCloudDictionary: Sendable {
  let accountID: String
  /// 默认是 App 启动时注册的那一份，测试传入替身函数表上的账号。
  var rust: @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() }

  /// 发一个 action，回答是 `value` 的 JSON。`snapshot_*` 和 `export_file` 带本进程的中转目录。
  func request(_ action: [String: Any]) async throws -> Data {
    let account = try rust()
    try await requireAccount(account)
    let data = try await account.cloudDictionary(action, snapshotDirectory: try Self.transferDirectory())
    try await requireAccount(account)
    return data
  }

  /// 同上，回答是对象。
  func object(_ action: [String: Any]) async throws -> [String: Any] {
    try BackendRustAccount.object(try await request(action))
  }

  private func requireAccount(_ account: BackendRustAccount) async throws {
    let current = try await account.status()?.id
    try Task.checkCancellation()
    guard current == accountID else { throw CancellationError() }
  }

  /// Rust 的中转目录（`snapshot_directory`）：放预览下载和导出、恢复的中转文件，Rust 在本进程第一次用到时清掉上次留下的。iOS 上只有 App 一个进程用它，放在 App 自己的临时目录里。
  static func transferDirectory() throws -> URL {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-dictionary", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    return url
  }

  /// 一个只有本 App 能读写的新目录里的 `name`，给导出后交给分享面板、或复制一份用户选的备份。调用方用完删掉整个目录。
  static func privateFile(_ name: String) throws -> URL {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-export-" + UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    return directory.appendingPathComponent(name, isDirectory: false)
  }

  /// Rust 给出的快照元数据（`snapshot`）里页面显示和恢复要用的几项。
  struct Snapshot: Equatable, Sendable {
    let cloudRevision: Int64
    let records: Int64
    let entries: Int64
    let overlays: Int64
    let positions: Int64
    let sha256: String

    init(_ value: Any?) throws {
      guard let object = value as? [String: Any], let sha256 = object["sha256"] as? String, !sha256.isEmpty else {
        throw BackendRustClient.Failure.invalidResponse
      }
      func count(_ key: String) throws -> Int64 {
        guard let number = object[key] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.int64Value >= 0 else {
          throw BackendRustClient.Failure.invalidResponse
        }
        return number.int64Value
      }
      cloudRevision = try count("cloudRevision"); records = try count("records")
      entries = try count("entries"); overlays = try count("overlays"); positions = try count("positions")
      self.sha256 = sha256
    }
  }
}

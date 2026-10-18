import Foundation

// Each prepared object has a single owner; staging runs off the main thread,
// and only the finished result is handed back for main-thread publication.
final class MacPreparedLocalSnapshot: @unchecked Sendable {
  let context: NSDictionary
  let snapshot: BackendPreparedSnapshot
  let identifier = UUID().uuidString
  private var prepared: UInt64?
  private var stagingRoot: URL?
  init(context: NSDictionary, snapshot: BackendPreparedSnapshot) { self.context = context; self.snapshot = snapshot }
  static func strictHandle(_ raw: Any?) -> UInt64? {
    guard !(raw is Bool), let number = raw as? NSNumber else { return nil }
    switch String(cString: number.objCType) {
    case "C", "S", "I", "L", "Q": return number.uint64Value
    case "c", "s", "i", "l", "q":
      let value = number.int64Value
      return value >= 0 ? UInt64(value) : nil
    default: return nil
    }
  }
  static func invoke(_ selector: String, _ parameters: NSDictionary? = nil) throws -> NSDictionary {
    guard let type = NSClassFromString("MSIMEClientSession") as? NSObject.Type,
          let result = type.perform(NSSelectorFromString(selector), with: parameters)?.takeUnretainedValue() as? NSDictionary else {
      throw BackendAccountClient.Failure(status: 503)
    }
    if let error = result["error"] as? NSError { throw error }
    return result
  }
  func stage() throws {
    let stream = try BackendSnapshotRecordStream(snapshot: snapshot)
    let next: @convention(block) (AutoreleasingUnsafeMutablePointer<NSError?>?) -> NSDictionary? = { failure in
      do { return try stream.next().map { $0 as NSDictionary } }
      catch { failure?.pointee = error as NSError; return nil }
    }
    let versionResult = try Self.invoke("snapshotVersion:", context)
    guard let version = versionResult["version"] as? String else { throw BackendAccountClient.Failure(status: 409) }
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-stage-" + identifier, isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    stagingRoot = root
    let request: NSDictionary = ["options": context, "staging_root": root.path,
      "expected_version": version, "records": snapshot.envelope.records,
      "activation_id": identifier]
    let result = try Self.invoke("prepareSnapshot:", ["request": request, "nextRecord": next])
    guard let handle = Self.strictHandle(result["handle"]) else {
      throw BackendAccountClient.Failure(status: 500)
    }
    prepared = handle
  }
  @MainActor func activate() throws {
    guard let prepared else { throw BackendAccountClient.Failure(status: 400) }
    let versionResult = try Self.invoke("snapshotVersion:", context)
    guard let version = versionResult["version"] as? String else { throw BackendAccountClient.Failure(status: 409) }
    _ = try Self.invoke("applySnapshot:", ["handle": prepared, "expectedVersion": version])
  }
  deinit {
    if let prepared { _ = try? Self.invoke("discardSnapshot:", ["handle": prepared]) }
    if let stagingRoot { try? FileManager.default.removeItem(at: stagingRoot) }
  }
}

/// 本进程里的输入会话与本机词库，给 Rust 的快照队列（`snapshot_preview`、`snapshot_enqueue`、`snapshot_status`）用：账号窗口的「云词库同步与备份」和 Tauri 面板的「应用到本机」都经这里。
@MainActor
enum MacLocalDictionary {
  /// 活动输入会话的 HostOptions 文档（`MSIMEClientSession.activeHostOptions`）。独立设置窗口进程和还没打过字的输入法进程没有输入会话，是 `snapshot_unavailable`。
  static func hostOptions() throws -> [String: Any] {
    guard let options = try? MacPreparedLocalSnapshot.invoke("activeHostOptions") as? [String: Any] else {
      throw BackendRustClient.Failure(code: "snapshot_unavailable")
    }
    return options
  }

  /// 在 `body` 进行期间每 250 毫秒叫醒一次本进程里握着词库的输入会话（`InputController.mm` 的 `dictionaryMaintenanceWillBegin:`，它只在词库维护租约有效时放手）。Rust 处理队列时写租约、不发通知，而输入法没有获得焦点的输入框时每秒检查租约的计时器不跑，会话就一直握着词库，请求只能留在队列里。
  static func wakingInputSessions<T>(_ body: () async throws -> T) async throws -> T {
    let ticker = Task { @MainActor in
      while !Task.isCancelled {
        NotificationCenter.default.post(name: BackendEdition.notificationName("MSIMEDictionaryMaintenanceWillBeginNotification"), object: nil)
        try? await Task.sleep(nanoseconds: 250_000_000)
      }
    }
    defer { ticker.cancel() }
    return try await body()
  }
}

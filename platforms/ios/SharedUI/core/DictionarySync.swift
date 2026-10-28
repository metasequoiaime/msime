import Foundation
#if canImport(Darwin)
import Darwin
#endif

@_silgen_name("msime_client_dictionary_sync")
private func msimeDictionarySync(_ request: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeDictionarySyncStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 词库同步失败。`code` 是共享层给的码（`dictionary_sync_unauthorized`、`clipboard_device_pending` 等，见 `msime_client.h`），或宿主自己的 `invalid_response`、`unavailable`；说法见 `IOSDictionarySyncPolicy.failureMessage`。
struct IOSDictionarySyncFailure: Error, LocalizedError, Equatable, Sendable {
  let code: String
  var errorDescription: String? { IOSDictionarySyncPolicy.failureMessage(code) }
}

/// 经用户的 WebDAV 网盘同步「我的词条」（`docs/dictionary-webdav-sync.md`），App 的「词库同步」页、App 的定时同步和键盘扩展共用。协议、合并、额度、退避、密钥串和设备批准全在 Rust 的 `msime_client_dictionary_sync` 里，这里只拼请求、在专用线程上调用、拆信封。
///
/// 本机的东西都在 App Group 容器的 `dictionary-sync/` 里（[`directory`]）：同步状态（含词条明文）、词库自己的密钥串（设备私钥和词库密钥，Rust 写成只有本应用可读的文件）、密钥串的日志缓存和两把锁。App 和键盘给同一个目录，所以是同一台设备：Rust 在 `dictionary-sync.lock` 里「读不到密钥串才生成」，两个进程第一次同时用也只会有一个身份。目录建成 0700、文件保护为「首次解锁后」、排除在设备备份之外，与云剪贴板的 `cloud-clipboard-relay/` 相同。
///
/// 除 `status` 外每个动作都会阻塞在网络上，持有那两把文件锁；挂起时还持有 App Group 文件锁的进程会被系统结束（0xdead10cc），所以调用期间要保持后台执行（`activity`，App 用 `beginBackgroundTask`，键盘用 `performExpiringActivity`）。
struct IOSDictionarySync: Sendable {
  enum Trigger: String, Sendable {
    case localChange = "local_change", opened, periodic, manual
  }

  /// 本机目录名，在 App Group 容器里，App 和键盘用同一个。
  static let directoryName = "dictionary-sync"
  /// Rust 在目录里放的两个文件：同步状态和词库密钥串（`msime_client.h` 列出的文件名）。
  static let stateFile = "dictionary-sync-state"
  static let keystoreFile = "cloud-clipboard-relay-keystore"

  /// App Group 容器里的 `dictionary-sync/`。
  static var directory: URL {
    IOSSharedPaths.sharedContainerOrApplicationSupport(for: MSIMEAppEdition.appGroupIdentifier)
      .appendingPathComponent(directoryName, isDirectory: true)
  }

  /// 不联网的概况，`status` 动作和每次 `run` 都带回它。
  struct Status: Decodable, Equatable, Sendable {
    /// 本机在词库密钥串里的设备编号和核对码；还没用过网盘时为 nil。
    let device_id: String?
    let code: String?
    /// 本机持有当前密钥（已获准，或者是第一台设备）。
    let approved: Bool
    let joined: Bool
    /// 共享词库里的词条数。
    let words: Int
    let pending_upload: Int
    let pending_apply: Int
    /// 被挡下的大批删除：本机少了这么多词，等用户选择。
    let held_deletions: Int?
    let clock_skew: Bool
    let last_success_ms: UInt64?
    let last_error: String?
    /// 自动同步因请求额度在等时什么时候继续（Unix 毫秒）：本机额度不够一轮，或者被网盘限流之后；没在等时为 nil（旧版本的 Rust 不给这个字段，也是 nil）。「立即同步」不受它限制。
    let budget_wait_until: UInt64?

    /// 已经登记，但还在等另一台设备批准。
    var isPending: Bool { device_id != nil && !approved }
  }

  struct Report: Decodable, Equatable, Sendable {
    let captured: Int
    let received_events: Int
    let applied: Int
    /// 输入法正占着词库，远端改动这一轮没写进去，Rust 按 `next_check_ms` 安排不联网的重试。
    let deferred: Bool
    let published_ops: Int
    let pending_key: Bool
    let compacted: Bool
    let rejoined: Bool
  }

  /// `run` 的结果。同步失败时没有 `report`，`error` 是错误码，`status` 照样给出。
  struct Run: Decodable, Equatable, Sendable {
    let network: Bool
    /// 下一次该在多少毫秒后以 `periodic` 再调。
    let next_check_ms: UInt64
    let status: Status
    let report: Report?
    let error: String?
  }

  /// `remove` 和 `reset` 的结果：设备状态，加上随后那一次同步。
  struct DeviceChange: Decodable, Sendable {
    let devices: BackendAccountClient.ClipboardDevices
    let run: Run
  }

  /// 调用期间保持后台执行：参数是原因，返回值在调用结束时调用一次。
  typealias Activity = @Sendable (String) -> @Sendable () -> Void

  /// 宿主准备好的 HostOptions（`msime_client_prepare_host` 的结果）编成的 JSON，Rust 从里面找 Engine 的词库和偏好目录。
  let options: Data
  let directory: URL
  let deviceName: String
  var activity: Activity = { _ in {} }

  init(options: Data, directory: URL = IOSDictionarySync.directory, deviceName: String,
       activity: @escaping Activity = { _ in {} }) {
    self.options = options
    self.directory = directory
    self.deviceName = deviceName
    self.activity = activity
  }

  // MARK: - 动作

  func status() async throws -> Status {
    try decode(Status.self, try await request(["operation": "status"]))
  }

  /// 跑一次同步。`confirmDeletions` 和 `restoreDeletions` 是用户对被挡下的大批删除的选择。
  func run(_ trigger: Trigger, confirmDeletions: Bool = false, restoreDeletions: Bool = false) async throws -> Run {
    var action: [String: Any] = ["operation": "run", "trigger": trigger.rawValue]
    if confirmDeletions { action["confirm_deletions"] = true }
    if restoreDeletions { action["restore_deletions"] = true }
    return try decode(Run.self, try await request(action))
  }

  /// 词库密钥串里的设备；本机还没登记就登记，网盘上还没有密钥时由本机生成。
  func devices() async throws -> BackendAccountClient.ClipboardDevices {
    try decode(BackendAccountClient.ClipboardDevices.self, try await request(["operation": "devices"]))
  }

  func approve(_ deviceID: String) async throws -> BackendAccountClient.ClipboardDevices {
    try decode(BackendAccountClient.ClipboardDevices.self, try await request(["operation": "approve", "device_id": deviceID]))
  }

  func reject(_ deviceID: String) async throws -> BackendAccountClient.ClipboardDevices {
    try decode(BackendAccountClient.ClipboardDevices.self, try await request(["operation": "reject", "device_id": deviceID]))
  }

  /// 移除一台设备并换钥（其他设备要重新批准），随后同步一次。
  func remove(_ deviceID: String) async throws -> DeviceChange {
    try decode(DeviceChange.self, try await request(["operation": "remove", "device_id": deviceID]))
  }

  /// 换一把只给本机的新密钥，其他设备都要重新批准，随后同步一次。
  func reset() async throws -> DeviceChange {
    try decode(DeviceChange.self, try await request(["operation": "reset"]))
  }

  /// 以本机为准重新开始：持有旧密钥的设备都不在了时由用户选择，其他设备的词随后合并进来，不删词。
  func restart() async throws -> Status {
    try decode(Status.self, try await request(["operation": "restart"]))
  }

  /// 保存之前的「测试连接」：用填写页里还没保存的草稿，默认不改同步状态。只有用户单独确认恢复后才传 trustRemote。请求里有应用密码，不写日志。大约二十个请求。
  func probe(url: String, username: String, password: String, trustRemote: Bool = false) async throws {
    _ = try await request(["operation": "probe", "url": url, "username": username, "password": password, "trust_remote": trustRemote])
  }

  /// 一个动作，Value 原样交回。
  func request(_ action: [String: Any]) async throws -> Data {
    let data = try encode(action)
    let activity = self.activity
    let operation = action["operation"] as? String ?? ""
    return try await Self.onDedicatedThread {
      let end = activity("app.msime.dictionary-sync.\(operation)")
      defer { end() }
      return try Self.perform(data)
    }
  }

  /// 拼好整个请求：建好目录，带上 HostOptions、目录和设备名。
  func encode(_ action: [String: Any]) throws -> Data {
    try Self.prepare(directory)
    guard let options = (try? JSONSerialization.jsonObject(with: options)) as? [String: Any] else {
      throw IOSDictionarySyncFailure(code: "invalid_dictionary_sync")
    }
    let request: [String: Any] = ["options": options, "directory": directory.path, "device_name": deviceName, "action": action]
    guard JSONSerialization.isValidJSONObject(request), let data = try? JSONSerialization.data(withJSONObject: request),
          data.count <= 2 * 1024 * 1024 else { throw IOSDictionarySyncFailure(code: "invalid_dictionary_sync") }
    return data
  }

  private func decode<T: Decodable>(_ type: T.Type, _ data: Data) throws -> T {
    do { return try JSONDecoder().decode(type, from: data) }
    catch { throw IOSDictionarySyncFailure(code: "invalid_response") }
  }

  // MARK: - 调用

  /// 建好本机目录：只有本应用能进，文件保护为「首次解锁后」（键盘在锁屏下也可能被调出来，与其他 App Group 文件相同），不进设备备份（与 Android 放 `noBackupFilesDir` 同样考虑：里面有设备私钥和词条明文）。目录上的保护类别会被 Rust 之后建的文件继承。
  static func prepare(_ directory: URL) throws {
    let manager = FileManager.default
    do {
      try manager.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
      try manager.setAttributes([.posixPermissions: 0o700, .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
                                ofItemAtPath: directory.path)
    } catch {
      throw IOSDictionarySyncFailure(code: "dictionary_sync_storage")
    }
    IOSSharedPaths.excludeFromBackup(directory)
  }

  /// 同步地调用 C 入口并拆开 `{ok,value|error}` 信封；失败以共享层的码抛出。会阻塞几十秒，只在专用线程或后台线程上调用。回答只释放一次，原文不带进错误。
  static func perform(_ request: Data) throws -> Data {
    let raw = request.withUnsafeBytes { bytes in
      msimeDictionarySync(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
    }
    guard let raw else { throw IOSDictionarySyncFailure(code: "invalid_response") }
    let reply = Data(bytes: raw, count: strlen(raw))
    msimeDictionarySyncStringFree(raw)
    guard let envelope = (try? JSONSerialization.jsonObject(with: reply)) as? [String: Any],
          let ok = envelope["ok"] as? NSNumber, CFGetTypeID(ok) == CFBooleanGetTypeID() else {
      throw IOSDictionarySyncFailure(code: "invalid_response")
    }
    guard ok.boolValue else {
      let code = envelope["error"] as? String ?? ""
      throw IOSDictionarySyncFailure(code: code.isEmpty ? "invalid_response" : code)
    }
    guard let value = try? JSONSerialization.data(withJSONObject: envelope["value"] ?? NSNull(), options: [.fragmentsAllowed]) else {
      throw IOSDictionarySyncFailure(code: "invalid_response")
    }
    return value
  }

  /// Rust 入口会阻塞几十秒，不放进协作线程池；栈给 2 MiB，与 `IOSCloudClipboardRelay.call` 相同。
  private static func onDedicatedThread(_ body: @escaping @Sendable () throws -> Data) async throws -> Data {
    let result: Result<Data, Error> = await withCheckedContinuation { continuation in
      let thread = Thread { continuation.resume(returning: Result { try body() }) }
      thread.name = "app.msime.dictionary-sync"
      thread.stackSize = 2 << 20
      thread.start()
    }
    return try result.get()
  }
}

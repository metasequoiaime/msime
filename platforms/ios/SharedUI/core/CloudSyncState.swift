import Foundation

@_silgen_name("msime_client_cloud_sync")
private func msimeClientCloudSync(_ request: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_string_free")
private func msimeCloudSyncStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 云同步的分类。iOS 只同步设置和无编码常用语：服务端字段表里没有 iOS 的皮肤库，个人词库由键盘里的 Engine 持有、应用进程导不出来（见 `.agents/notes` 里的 iOS 自动云同步）。
enum CloudSyncSection: String, CaseIterable, Sendable {
  case settings, phrases
}

/// Rust 状态（`msime_client_cloud_sync` 的 S）里页面要读的几项。
struct CloudSyncStatus: Equatable, Sendable {
  var enabled = false
  /// 「我的」页同步行的一句话，措辞在 Rust。
  var statusLine = ""
  var accountID: String?
  /// 等「云端被清空」的选择，或者用户在那里选了「关闭同步」。
  var disabledOnCloudCleared = false
  /// 各分类有没有待上传的本机改动。
  var dirty: Set<CloudSyncSection> = []

  init() {}

  init(_ value: Any?) throws {
    guard let object = value as? [String: Any], let enabled = object["enabled"] as? Bool,
          let line = object["status_line"] as? String else { throw CloudSyncState.Failure.invalid }
    self.enabled = enabled
    statusLine = line
    accountID = object["account_id"] as? String
    disabledOnCloudCleared = object["disabled_on_cloud_cleared"] as? Bool ?? false
    let sections = object["sections"] as? [String: Any] ?? [:]
    dirty = Set(CloudSyncSection.allCases.filter { ((sections[$0.rawValue] as? [String: Any])?["dirty"] as? Bool) == true })
  }
}

/// 云同步的状态文件交给 Rust 管（`msime_client_cloud_sync`，#8201 迁移计划 4d）：App Group 根目录下的 `cloud-sync/state.json`，读改写都在 Rust 的 `state.lock` 里。这里只剩两件不需要会话的事：记一次本机改动（`mark_dirty`）和只读地看开关（`peek`），键盘扩展和 App 都会调；对账、开关、跑一轮和提交只由持有登录的 App 经 `CloudSync` 调。
///
/// 放在 App Group 而不是应用私有存储，是因为键盘扩展也要记改动：iOS 的键盘没有能调用应用的通道，只能写共享容器。没有 App Group 容器时（未签名的模拟器构建不带 entitlements）退回应用自己的 Application Support，与 `MetasequoiaInputSessionBridge` 存共享设置文档的退路相同，改动标记和它标记的文档始终在同一处。
final class CloudSyncState: @unchecked Sendable {
  enum Failure: Error, Equatable {
    case unavailable
    case invalid
    /// Rust 给出的失败码，例如 `cloud_sync_storage`。
    case rust(String)
  }

  static let defaultRoot: URL = IOSSharedPaths.sharedContainerOrApplicationSupport(
    for: MSIMEAppEdition.appGroupIdentifier)
  /// 生产代码都用这一份；测试换成临时目录上的一份，免得碰真正的同步进度。
  nonisolated(unsafe) static var shared = CloudSyncState(directory: defaultRoot)
  private static let suppressionKey = "app.msime.cloudSync.suppressMarks"
  /// 交给 Rust 的 `state_directory`。
  let directory: URL?

  init(directory: URL?) {
    self.directory = directory
  }

  /// iOS 第 1 版自己写的状态文件，迁移到 Rust 时导入一次（`importLegacy`）。
  var legacyFile: URL? { directory?.appendingPathComponent("CloudSync/state.json") }

  /// 记一次本机改动：开关打开时给这个分类记上待上传，下一轮就会上传；开关关着时 Rust 不记。同步自己写本机时（`suppressingMarks`）不记。记不下（容器不可用，例如键盘没有完全访问权限）时什么也不做：这时改动本身也写不进共享容器。
  func markDirty(_ section: CloudSyncSection) {
    guard !Self.suppressed else { return }
    _ = try? call(["operation": "mark_dirty", "section": section.rawValue])
  }

  /// 只读状态文件看开关和状态行，不读会话、不对账，键盘扩展也能调。
  func peek() throws -> CloudSyncStatus {
    try CloudSyncStatus(try call(["operation": "peek"]))
  }

  /// 把 iOS 第 1 版的状态导入 Rust，免得迁移以后又弹首次选择。只在还没有任何进度时导入，导入过一次、调过开关以后不再导入，所以每次启动都可以调；旧文件不存在或读不出来时什么也不做。返回是否导入了。
  @discardableResult
  func importLegacy() -> Bool {
    guard let file = legacyFile, let data = try? BoundedFileReader.read(from: file, maximumBytes: 256 * 1024),
          let record = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let value = try? call(["operation": "import_legacy", "ios_v1": record]) as? [String: Any] else { return false }
    return value["imported"] as? Bool == true
  }

  /// 在这段同步代码里写本机时不记改动，否则同步应用云端内容之后，下一轮又会把它当成本机改动上传。只对当前线程生效，闭包必须同步执行完。
  static func suppressingMarks<T>(_ body: () throws -> T) rethrows -> T {
    let dictionary = Thread.current.threadDictionary
    let previous = dictionary[suppressionKey]
    dictionary[suppressionKey] = true
    defer { dictionary[suppressionKey] = previous }
    return try body()
  }

  private static var suppressed: Bool {
    Thread.current.threadDictionary[suppressionKey] as? Bool == true
  }

  /// 不带会话来源的一个 action（`peek`、`mark_dirty`、`import_legacy`），同步调用：只读写状态文件，不联网。成功时是 `value`。
  private func call(_ action: [String: Any]) throws -> Any {
    guard let directory else { throw Failure.unavailable }
    let request: [String: Any] = ["state_directory": directory.path, "action": action]
    guard JSONSerialization.isValidJSONObject(request), let data = try? JSONSerialization.data(withJSONObject: request) else {
      throw Failure.invalid
    }
    guard let raw = data.withUnsafeBytes({ bytes in
      msimeClientCloudSync(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
    }) else { throw Failure.invalid }
    let answer = Data(bytes: raw, count: strlen(raw))
    msimeCloudSyncStringFree(raw)
    guard let envelope = try? JSONSerialization.jsonObject(with: answer) as? [String: Any],
          let ok = envelope["ok"] as? Bool else { throw Failure.invalid }
    guard ok else { throw Failure.rust(envelope["error"] as? String ?? "") }
    return envelope["value"] ?? NSNull()
  }
}

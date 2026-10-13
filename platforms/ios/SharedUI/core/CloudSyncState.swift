import Foundation
import Darwin

/// 云同步的分类。命名词库和个人词库不在其中：iOS 的服务端字段表没有皮肤库字段，个人词库由键盘里的 Engine 持有、应用进程导不出来（见 `.agents/notes` 里的 iOS 自动云同步）。
enum CloudSyncSection: String, Codable, CaseIterable, Sendable {
  case settings, phrases
}

/// 同步开关与进度，字段与 Android 的 `SyncSwitch` 加 `CloudSync` 的状态对应。
struct CloudSyncRecord: Codable, Equatable, Sendable {
  var version = 1
  var enabled = false
  /// 这份进度属于哪个账号的哪一次登录；没有真实账号时都为空。
  var accountID = ""
  var sessionID: UUID?
  /// 每次换账号、退出或重新登录都加一，用来作废换账号之前开始的那一轮同步的写入。
  var bindingGeneration: Int64 = 0
  /// 每个分类上次同步到的云端版本；空串表示还没同步过，下一轮按首次处理。
  var cursors: [String: String] = [:]
  /// 每个分类的本机改动代数，每记一次改动加一。
  var generations: [String: Int64] = [:]
  /// 每个分类上次清掉待上传标记时的代数；与 `generations` 不同即有改动还没上传。
  var cleans: [String: Int64] = [:]
  /// 上次应用云端常用语时本机收不下的正文（超出本机条数或长度上限、或写入失败）。
  var unheldPhrases: [String] = []
  var lastAttempt: Date?
  var lastSynced: Date?
  /// 上次弹出「云端已有同步数据」的时间。
  var promptedAt: Date?
  /// 最近一次失败展示给用户的原因；成功后清掉。
  var error: String?

  func cursor(_ section: CloudSyncSection) -> String { cursors[section.rawValue] ?? "" }
  func generation(_ section: CloudSyncSection) -> Int64 { generations[section.rawValue] ?? 0 }
  func dirty(_ section: CloudSyncSection) -> Bool { generation(section) != (cleans[section.rawValue] ?? 0) }
  var anyDirty: Bool { CloudSyncSection.allCases.contains(where: dirty) }
  var bound: Bool { !accountID.isEmpty && sessionID != nil }
}

/// 同步状态的存储：App Group 的 `CloudSync/state.json`，读改写都在同目录 `state.lock` 的 `flock` 里完成。
///
/// 放在 App Group 而不是应用私有存储，是因为键盘扩展也要给代数加一：Android 的输入法进程经 `AccountSessionProvider` 让主进程记改动，iOS 的键盘没有能调用应用的通道，只能写共享容器。改动由应用和键盘两个进程并发记录，所以不用 `UserDefaults`，它跨进程的读改写不是原子的。只有应用跑同步，其余写入都是应用里的同步控制器做的。
final class CloudSyncState: @unchecked Sendable {
  enum Failure: Error { case unavailable, invalid, notSignedIn }

  /// 共享状态的根目录。没有 App Group 容器时（未签名的模拟器构建不带 entitlements）退回应用自己的 Application Support，与 `MetasequoiaInputSessionBridge` 存共享设置文档的退路相同，改动标记和它标记的文档始终在同一处。签名的构建总有容器，不会走到退路。
  static let defaultRoot: URL = IOSSharedPaths.sharedContainerOrApplicationSupport(
    for: MSIMEAppEdition.appGroupIdentifier)
  static let shared = CloudSyncState(directory: defaultRoot)
  private static let maximumBytes = 256 * 1024
  private static let suppressionKey = "app.msime.cloudSync.suppressMarks"
  private static let processLock = NSLock()
  private let directory: URL?

  init(directory: URL? = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: MSIMEAppEdition.appGroupIdentifier)) {
    self.directory = directory?.appendingPathComponent("CloudSync", isDirectory: true)
  }

  /// 读当前状态；文件不存在时是全新的默认值。
  func read() throws -> CloudSyncRecord {
    try withLock { try load($0) }
  }

  /// 在锁内读出、修改、写回。闭包可以抛错放弃这次修改。
  @discardableResult
  func update<T>(_ change: (inout CloudSyncRecord) throws -> T) throws -> T {
    try withLock { root in
      var record = try load(root)
      let before = record
      let result = try change(&record)
      if record != before { try save(record, in: root) }
      return result
    }
  }

  /// 记一次本机改动：开关打开时给这个分类的代数加一，下一轮就会上传。同步关闭时不记，打开时本来就会整份比对。同步自己写本机时（`suppressingMarks`）不记。记不下（容器不可用，例如键盘没有完全访问权限）时什么也不做：这时改动本身也写不进共享容器。
  func markDirty(_ section: CloudSyncSection) {
    guard !Self.suppressed else { return }
    try? update { record in
      guard record.enabled else { return }
      record.generations[section.rawValue] = record.generation(section) + 1
    }
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

  /// 把进度和当前登录对齐。登录（账号 id 加这次登录的 `sessionID`）与记下的不同（换了账号、退出后重新登录、已经退出）时清空全部进度并关闭开关，`bindingGeneration` 加一，与 Android 的 `SyncSwitch.clear` 加 `bindAccount` 相同；相同时不动。返回对齐后的状态。
  @discardableResult
  func bind(accountID: String?, sessionID: UUID?) throws -> CloudSyncRecord {
    try update { record in
      let accountID = accountID ?? ""
      let sessionID = accountID.isEmpty ? nil : sessionID
      guard record.accountID != accountID || record.sessionID != sessionID else { return record }
      let generation = record.bindingGeneration + 1
      record = CloudSyncRecord()
      record.bindingGeneration = generation
      record.accountID = accountID
      record.sessionID = sessionID
      return record
    }
  }

  /// 打开或关闭同步；关闭时保留游标，重新打开后增量继续。没有真实账号时不能打开。打开时把全部分类标成待上传，与 Android 的开关相同。
  func setEnabled(_ enabled: Bool) throws {
    try update { record in
      guard !enabled || record.bound else { throw Failure.notSignedIn }
      record.enabled = enabled
      if enabled {
        for section in CloudSyncSection.allCases {
          record.generations[section.rawValue] = record.generation(section) + 1
        }
      }
    }
  }

  /// 只在开始这一轮的那次登录仍然是当前登录时才修改；换了账号或退出登录时返回 false，什么也不写。
  @discardableResult
  func updateIfCurrent(_ bindingGeneration: Int64, _ change: (inout CloudSyncRecord) -> Void) throws -> Bool {
    try update { record in
      guard record.bindingGeneration == bindingGeneration else { return false }
      change(&record)
      return true
    }
  }

  /// 代数仍是 `expected` 时清掉待上传标记；期间又有改动（代数更大）或换了账号时保留，下一轮会把那次改动传上去。
  @discardableResult
  func clearDirtyIfCurrent(_ section: CloudSyncSection, expected: Int64, bindingGeneration: Int64) throws -> Bool {
    try update { record in
      guard record.bindingGeneration == bindingGeneration, record.generation(section) == expected else { return false }
      record.cleans[section.rawValue] = expected
      return true
    }
  }

  private func withLock<T>(_ body: (URL) throws -> T) throws -> T {
    guard let directory else { throw Failure.unavailable }
    Self.processLock.lock()
    defer { Self.processLock.unlock() }
    guard !SafePath.hasRefusedSymbolicLink(directory) else { throw Failure.unavailable }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    let descriptor = open(directory.appendingPathComponent("state.lock").path,
                          O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw Failure.unavailable }
    defer { close(descriptor) }
    // 阻塞等锁：每次持锁只读写一个小文件，而键盘记改动时不能因为应用恰好在写就丢掉这一次。
    guard flock(descriptor, LOCK_EX) == 0 else { throw Failure.unavailable }
    defer { flock(descriptor, LOCK_UN) }
    return try body(directory)
  }

  private func load(_ root: URL) throws -> CloudSyncRecord {
    let file = root.appendingPathComponent("state.json")
    guard FileManager.default.fileExists(atPath: file.path) else { return CloudSyncRecord() }
    let data: Data
    do {
      data = try BoundedFileReader.read(from: file, maximumBytes: Self.maximumBytes)
    } catch BoundedFileReader.Failure.tooLarge {
      return CloudSyncRecord()
    } catch {
      throw Failure.unavailable
    }
    // 读不出的文件按全新状态处理：开关回到关闭、游标清空，下一次打开时整份比对，不会丢数据。
    guard let record = try? Self.decoder.decode(CloudSyncRecord.self, from: data),
          record.version == 1 else { return CloudSyncRecord() }
    return record
  }

  private func save(_ record: CloudSyncRecord, in root: URL) throws {
    let data = try Self.encoder.encode(record)
    guard data.count <= Self.maximumBytes else { throw Failure.invalid }
    try data.write(to: root.appendingPathComponent("state.json"),
                   options: IOSSharedPaths.protectedAtomicWriteOptions)
  }

  private static let encoder: JSONEncoder = {
    let encoder = JSONEncoder()
    encoder.dateEncodingStrategy = .iso8601
    return encoder
  }()
  private static let decoder: JSONDecoder = {
    let decoder = JSONDecoder()
    decoder.dateDecodingStrategy = .iso8601
    return decoder
  }()
}

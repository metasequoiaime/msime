import Foundation
import CryptoKit
import Darwin

struct DictionarySnapshotRequest: Codable, Sendable, Identifiable {
  enum Status: String, Codable, Sendable {
    case queued, preparing, applied, conflict, failed, cancelled
    var active: Bool { self == .queued || self == .preparing }
  }
  let id: UUID
  let accountID: String
  let cloudRevision: Int64
  let expectedLocalVersion: String
  let fileSHA256: String
  var status: Status = .queued
}

struct DictionarySnapshotQueueState: Codable, Sendable {
  var version = 1
  var localVersion: String?
  var request: DictionarySnapshotRequest?
}

// App/keyboard file handoff only. Engine owns dictionary staging and activation.
// The worker lease spans asynchronous preparation; the state lock only protects
// short metadata updates and the final version check / activation / acknowledgement.
final class DictionarySnapshotQueue: @unchecked Sendable {
  enum Failure: Error, LocalizedError {
    case unavailable, busy, invalid, conflict
    var errorDescription: String? {
      switch self {
      case .unavailable: return "无法访问键盘共享目录，请检查完全访问权限。"
      case .busy: return "另一份词库快照正在处理，请稍后再试。"
      case .invalid: return "词库交接文件无效，原有词库未更改。"
      case .conflict: return "本地词库已变化，请重新获取版本后再应用。"
      }
    }
  }
  final class WorkerLease: @unchecked Sendable {
    private let descriptor: Int32
    fileprivate let owner: URL
    fileprivate init(_ descriptor: Int32, owner: URL) { self.descriptor = descriptor; self.owner = owner }
    deinit { flock(descriptor, LOCK_UN); close(descriptor) }
  }
  private let directory: URL?
  private static let processLock = NSLock()
  init(directory: URL? = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: MSIMEAppEdition.appGroupIdentifier)) {
    self.directory = directory?.appendingPathComponent("DictionarySnapshots", isDirectory: true)
  }
  private static func digest(_ value: String) -> Bool {
    value.utf8.count == 64 && value.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
  }
  static func validVersion(_ value: String) -> Bool {
    let fields = value.split(separator: ":", omittingEmptySubsequences: false)
    guard fields.count == 3, fields[0] == "local-v1", digest(String(fields[2])) else { return false }
    return fields[1] == "legacy" || UUID(uuidString: String(fields[1]))?.uuidString == String(fields[1])
  }
  private func rejectSymlinkAncestors(_ path: URL) throws {
    guard !SafePath.hasRefusedSymbolicLink(path) else { throw Failure.unavailable }
  }
  private func rejectSymlinkFile(_ path: URL) throws {
    var status = stat()
    if lstat(path.standardizedFileURL.path, &status) == 0 {
      guard status.st_mode & S_IFMT != S_IFLNK else { throw Failure.unavailable }
    } else if errno != ENOENT {
      throw Failure.unavailable
    }
  }
  private func root() throws -> URL {
    guard let directory else { throw Failure.unavailable }
    try rejectSymlinkAncestors(directory)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    return directory
  }
  private func read(_ root: URL) throws -> DictionarySnapshotQueueState {
    let file = root.appendingPathComponent("state.json")
    try rejectSymlinkFile(file)
    guard FileManager.default.fileExists(atPath: file.path) else { return .init() }
    let descriptor = open(file.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
    guard descriptor >= 0 else { throw Failure.unavailable }
    var metadata = stat()
    guard fstat(descriptor, &metadata) == 0, (metadata.st_mode & S_IFMT) == S_IFREG else {
      close(descriptor)
      throw Failure.unavailable
    }
    let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
    defer { try? handle.close() }
    let data = try handle.read(upToCount: 65537) ?? Data()
    guard data.count <= 65536, let state = try? JSONDecoder().decode(DictionarySnapshotQueueState.self, from: data),
          state.version == 1, state.localVersion.map(Self.validVersion) ?? true else { throw Failure.invalid }
    if let request = state.request {
      guard !request.accountID.isEmpty, request.accountID.utf8.count <= 128,
            request.cloudRevision >= 0, Self.validVersion(request.expectedLocalVersion), Self.digest(request.fileSHA256) else { throw Failure.invalid }
    }
    return state
  }
  func read() throws -> DictionarySnapshotQueueState { try read(root()) }
  /// `persist` 返回 `false` 时不重写状态文件：没有改动的操作不应因写入失败（例如磁盘已满）而失败。
  private func update<T>(persist: (T) -> Bool = { _ in true },
                         _ action: (inout DictionarySnapshotQueueState) throws -> T) throws -> T {
    Self.processLock.lock()
    defer { Self.processLock.unlock() }
    let root = try root()
    let descriptor = open(root.appendingPathComponent("state.lock").path, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw Failure.unavailable }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { throw Failure.busy }
    defer { flock(descriptor, LOCK_UN) }
    var state = try read(root)
    let result = try action(&state)
    guard persist(result) else { return result }
    let data = try JSONEncoder().encode(state)
    guard data.count <= 65536 else { throw Failure.invalid }
    try data.write(to: root.appendingPathComponent("state.json"), options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    return result
  }
  func publishLocalVersion(_ version: String) throws {
    guard Self.validVersion(version) else { throw Failure.invalid }
    let applied = try update { state -> DictionarySnapshotRequest? in
      state.localVersion = version
      // The Engine's durable generation UUID is the receipt if publication
      // succeeded but queue acknowledgement failed. A terminal queue state
      // wins if cancellation or another transition was recorded first.
      let generation = version.split(separator: ":")[1]
      // A receipt can arrive after cancellation or another terminal transition.
      // Do not resurrect that request merely because the Engine generation matches.
      guard var request = state.request, request.status.active,
            request.id.uuidString == generation else { return nil }
      request.status = .applied
      state.request = request
      return request
    }
    if let applied { try? FileManager.default.removeItem(at: fileURL(for: applied)) }
  }
  func acquireWorkerLease() throws -> WorkerLease {
    let root = try root()
    let descriptor = open(root.appendingPathComponent("worker.lock").path, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw Failure.unavailable }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { close(descriptor); throw Failure.busy }
    return WorkerLease(descriptor, owner: root.standardizedFileURL)
  }
  func fileURL(for request: DictionarySnapshotRequest) throws -> URL {
    try root().appendingPathComponent(request.id.uuidString + ".ndjson")
  }
  @discardableResult
  func enqueue(file: URL, accountID: String, cloudRevision: Int64, expectedLocalVersion: String, fileSHA256: String) throws -> UUID {
    guard file.isFileURL, !accountID.isEmpty, accountID.utf8.count <= 128, cloudRevision >= 0,
          Self.validVersion(expectedLocalVersion), Self.digest(fileSHA256) else { throw Failure.invalid }
    let before = try read()
    guard before.request?.status.active != true else { throw Failure.busy }
    guard before.localVersion == expectedLocalVersion else { throw Failure.conflict }
    let request = DictionarySnapshotRequest(id: UUID(), accountID: accountID, cloudRevision: cloudRevision,
      expectedLocalVersion: expectedLocalVersion, fileSHA256: fileSHA256)
    let destination = try fileURL(for: request)
    let incoming = destination.appendingPathExtension("incoming")
    var committed = false
    defer {
      try? FileManager.default.removeItem(at: incoming)
      if !committed { try? FileManager.default.removeItem(at: destination) }
    }
    guard FileManager.default.createFile(atPath: incoming.path, contents: nil,
      attributes: [.posixPermissions: 0o600, .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication]) else { throw Failure.unavailable }
    let input = try openSnapshotSource(file)
    defer { try? input.close() }
    let output = try FileHandle(forWritingTo: incoming)
    defer { try? output.close() }
    var hash = SHA256(), count = 0
    while let data = try input.read(upToCount: 65536), !data.isEmpty {
      try Task.checkCancellation()
      count += data.count
      guard count <= 512 * 1024 * 1024 else { throw Failure.invalid }
      hash.update(data: data)
      try output.write(contentsOf: data)
    }
    guard count > 0, hash.finalize().map({ String(format: "%02x", $0) }).joined() == fileSHA256 else { throw Failure.invalid }
    try output.synchronize()
    try FileManager.default.moveItem(at: incoming, to: destination)
    let previous = try update { state -> DictionarySnapshotRequest? in
      guard state.request?.status.active != true else { throw Failure.busy }
      guard state.localVersion == expectedLocalVersion else { throw Failure.conflict }
      let previous = state.request
      state.request = request
      return previous
    }
    committed = true
    if let previous { try? FileManager.default.removeItem(at: fileURL(for: previous)) }
    return request.id
  }

  private func openSnapshotSource(_ file: URL) throws -> FileHandle {
    let descriptor = open(file.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
    guard descriptor >= 0 else { throw Failure.unavailable }
    var metadata = stat()
    guard fstat(descriptor, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG else {
      close(descriptor)
      throw Failure.unavailable
    }
    return FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
  }

  // A killed worker resumes the same UUID. It first checks the Engine's durable
  // active identifier; already-applied requests are acknowledged without replay.
  func claim(using lease: WorkerLease) throws -> DictionarySnapshotRequest? {
    guard lease.owner == (try root()).standardizedFileURL else { throw Failure.invalid }
    return try update { state in
      guard state.request?.status.active == true else { return nil }
      state.request?.status = .preparing
      return state.request
    }
  }
  // Call with the worker lease held and input quiesced. apply must publish
  // idempotently by request UUID and return the resulting local version.
  @discardableResult
  func complete(id: UUID, using lease: WorkerLease, currentVersion: String, alreadyApplied: Bool,
                apply: () throws -> String) throws -> Bool {
    guard lease.owner == (try root()).standardizedFileURL, Self.validVersion(currentVersion) else { throw Failure.invalid }
    let applied = try update { state -> Bool in
      guard var request = state.request, request.id == id, alreadyApplied || request.status.active else { throw Failure.conflict }
      if alreadyApplied {
        request.status = .applied
        state.localVersion = currentVersion
      } else if request.expectedLocalVersion != currentVersion {
        request.status = .conflict
        state.localVersion = currentVersion
      } else {
        let next = try apply()
        guard Self.validVersion(next) else { throw Failure.invalid }
        request.status = .applied
        state.localVersion = next
      }
      state.request = request
      return request.status == .applied
    }
    try? FileManager.default.removeItem(at: root().appendingPathComponent(id.uuidString + ".ndjson"))
    return applied
  }
  func cancel(accountID: String) throws {
    for attempt in 0..<40 {
      do {
        let cancelled = try update(persist: { $0 != nil }) { state -> DictionarySnapshotRequest? in
          guard state.request?.accountID == accountID, state.request?.status.active == true else { return nil }
          state.request?.status = .cancelled
          return state.request
        }
        if let cancelled { try? FileManager.default.removeItem(at: fileURL(for: cancelled)) }
        return
      } catch Failure.busy {
        if attempt == 39 { throw Failure.busy }
        usleep(25_000)
      }
    }
  }

  /// 账号操作取消旧账号快照的入口。没有 App Group 容器（例如测试宿主）时不存在队列，直接返回。状态文件损坏或版本不符时键盘同样领取不了其中的请求，视为没有可取消的请求，不改动该文件。其余错误（目录不可访问、状态锁一直被占用、写入失败）照常抛出，由调用方决定是否阻止账号操作。
  func cancelIfPresent(accountID: String) throws {
    guard directory != nil else { return }
    do { try cancel(accountID: accountID) } catch Failure.invalid { return }
  }
  func fail(id: UUID) throws {
    let failed = try update { state -> DictionarySnapshotRequest? in
      guard state.request?.id == id, state.request?.status.active == true else { return nil }
      state.request?.status = .failed
      return state.request
    }
    if let failed { try? FileManager.default.removeItem(at: fileURL(for: failed)) }
  }
}

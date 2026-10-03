import Foundation
import Darwin

struct VoiceTextHandoff: Codable, Equatable, Identifiable, Sendable {
  var version = 1
  let id: UUID
  let text: String
  let createdAt: Date
  let expiresAt: Date
}

// Explicit, local transfer of one recognized result. Audio and credentials never enter
// this file. All readers/writers use the same lock, including expiry cleanup and claiming.
final class VoiceTextHandoffStore: @unchecked Sendable {
  enum Failure: LocalizedError {
    case unavailable, busy, invalid, stale
    var errorDescription: String? {
      switch self {
      case .unavailable: "无法访问语音共享目录，请检查键盘的完全访问权限。"
      case .busy: "语音结果正在更新，请稍后重试。"
      case .invalid: "语音结果为空、过长或无法读取，请重新发送。"
      case .stale: "语音结果已过期、已被使用或已更新，请重新打开语音结果。"
      }
    }
  }
  private let directory: URL?
  private static let lock = NSLock()
  private static let maximumBytes = 256 * 1024
  static let lifetime: TimeInterval = 600

  static var defaultDirectory: URL? {
    let group = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.msime.ios")
    #if DEBUG && targetEnvironment(simulator)
    let arguments = ProcessInfo.processInfo.arguments
    if let index = arguments.firstIndex(of: "-voiceHandoffTestID"), index + 1 < arguments.count,
       let id = UUID(uuidString: arguments[index + 1]) {
      return group?.appendingPathComponent("voice-ui-" + id.uuidString)
    }
    #endif
    return group
  }

  init(directory: URL? = VoiceTextHandoffStore.defaultDirectory) {
    self.directory = directory?.appendingPathComponent("VoiceHandoff", isDirectory: true)
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

  private func locked<T>(_ action: (URL) throws -> T) throws -> T {
    guard let directory else { throw Failure.unavailable }
    Self.lock.lock()
    defer { Self.lock.unlock() }
    try rejectSymlinkAncestors(directory)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let descriptor = open(directory.appendingPathComponent("transfer.lock").path,
                          O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw Failure.unavailable }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { throw Failure.busy }
    defer { flock(descriptor, LOCK_UN) }
    let result = directory.appendingPathComponent("result.json")
    try rejectSymlinkFile(result)
    return try action(result)
  }

  private func readFile(_ file: URL, now: Date) throws -> VoiceTextHandoff? {
    guard FileManager.default.fileExists(atPath: file.path) else { return nil }
    let data = try Self.readBounded(file)
    guard data.count <= Self.maximumBytes,
          let entry = try? JSONDecoder().decode(VoiceTextHandoff.self, from: data), entry.version == 1,
          !entry.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, entry.text.count <= 10_000,
          abs(entry.expiresAt.timeIntervalSince(entry.createdAt) - Self.lifetime) < 1 else { throw Failure.invalid }
    guard entry.expiresAt > now, entry.createdAt <= now.addingTimeInterval(60) else {
      try FileManager.default.removeItem(at: file)
      return nil
    }
    return entry
  }

  static func readBounded(_ file: URL) throws -> Data {
    let handle = try FileHandle(forReadingFrom: file)
    defer { try? handle.close() }
    var data = Data()
    data.reserveCapacity(min(Self.maximumBytes, 64 * 1024))
    while true {
      let remaining = Self.maximumBytes - data.count
      let chunk = try handle.read(upToCount: min(64 * 1024, remaining + 1)) ?? Data()
      if chunk.isEmpty { return data }
      guard chunk.count <= remaining else { throw Failure.invalid }
      data.append(chunk)
    }
  }

  func read(now: Date = Date()) throws -> VoiceTextHandoff? {
    try locked { try readFile($0, now: now) }
  }

  @discardableResult
  func save(_ text: String, now: Date = Date()) throws -> VoiceTextHandoff {
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, text.count <= 10_000 else { throw Failure.invalid }
    let entry = VoiceTextHandoff(id: UUID(), text: text, createdAt: now, expiresAt: now.addingTimeInterval(Self.lifetime))
    let data = try JSONEncoder().encode(entry)
    guard data.count <= Self.maximumBytes else { throw Failure.invalid }
    try locked { try data.write(to: $0, options: [.atomic, .completeFileProtection]) }
    return entry
  }

  // Claim before the synchronous host insertion so a second keyboard cannot insert it twice.
  // Host text mutation cannot be atomic with a file claim; a process interruption in between
  // can lose the pending result. Do not retry insertion automatically.
  func consume(_ id: UUID, now: Date = Date()) throws -> String {
    try locked { file in
      guard let entry = try readFile(file, now: now), entry.id == id else { throw Failure.stale }
      try FileManager.default.removeItem(at: file)
      return entry.text
    }
  }

  func discard(_ id: UUID, now: Date = Date()) throws {
    _ = try consume(id, now: now)
  }
}

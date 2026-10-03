import Foundation
import Darwin

/// 「诊断日志」: the keyboard's host log, the iOS counterpart of the macOS and Linux host logs, turned on by the shared `diagnostic_log.server`.
///
/// Callers pass only fixed event labels, never key values, input text, candidates, paths or provider responses. Every record is cut to 192 bytes of printable ASCII anyway, so a mistake at a call site cannot leak text into the file. The log sits next to the shared preference document in the App Group, where the App can read, share and clear it; past 1 MiB it keeps one `.1` copy, like the desktop hosts. Writing is best-effort: a keyboard without full access cannot write to the App Group, and a failed write never reaches the input path.
final class DiagnosticLog: @unchecked Sendable {
  struct TailRead {
    let size: Int
    let data: Data
  }

  enum TailReadFailure: Error {
    case invalidLimit
    case tooLarge
  }

  static let shared = DiagnosticLog()
  static let fileName = "diagnostic.log"
  static let maxBytes = 1024 * 1024
  static let maxEventBytes = 192

  private static func rejectsSymlinkAncestors(_ path: URL) -> Bool {
    SafePath.hasRefusedSymbolicLink(path)
  }

  /// Reads at most `maximumBytes` from the end while retaining the file's full size for display.
  static func readTail(from url: URL, maximumBytes: Int) throws -> TailRead {
    guard maximumBytes > 0 else { throw TailReadFailure.invalidLimit }
    guard !rejectsSymlinkAncestors(url) else { throw TailReadFailure.tooLarge }
    let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
    let fileSize = (attributes[.size] as? NSNumber)?.int64Value ?? 0
    guard fileSize >= 0, fileSize <= Int64(Int.max) else { throw TailReadFailure.tooLarge }

    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    let offset = max(Int64(0), fileSize - Int64(maximumBytes))
    try handle.seek(toOffset: UInt64(offset))
    let data = try handle.read(upToCount: maximumBytes) ?? Data()
    return TailRead(size: Int(fileSize), data: data)
  }

  private let lock = NSLock()
  private var file: URL?

  /// Whether `diagnostic_log.server` is on in a shared preference document. Only a real boolean counts, as on macOS.
  static func isEnabled(in preferences: [String: Any]?) -> Bool {
    guard let value = (preferences?["diagnostic_log"] as? [String: Any])?["server"] as? NSNumber else { return false }
    return CFGetTypeID(value) == CFBooleanGetTypeID() && value.boolValue
  }

  static func url(in directory: String) -> URL? {
    directory.hasPrefix("/") ? URL(fileURLWithPath: directory, isDirectory: true).appendingPathComponent(fileName) : nil
  }

  /// Printable ASCII only, cut to `maxEventBytes`; anything else becomes `?`.
  static func sanitize(_ event: String) -> String {
    String(decoding: event.utf8.prefix(maxEventBytes).map { (0x20...0x7e).contains($0) ? $0 : UInt8(ascii: "?") }, as: UTF8.self)
  }

  func configure(directory: String?, enabled: Bool) {
    lock.lock(); defer { lock.unlock() }
    file = enabled ? directory.flatMap(Self.url(in:)) : nil
  }

  func write(_ event: String) {
    lock.lock(); defer { lock.unlock() }
    guard let file else { return }
    guard !Self.rejectsSymlinkAncestors(file) else { return }
    let manager = FileManager.default
    if let size = (try? manager.attributesOfItem(atPath: file.path))?[.size] as? NSNumber, size.intValue > Self.maxBytes {
      let rotated = file.appendingPathExtension("1")
      try? manager.removeItem(at: rotated)
      try? manager.moveItem(at: file, to: rotated)
    }
    let stamp = Self.formatter.string(from: Date())
    let record = Data("\(stamp) [p\(ProcessInfo.processInfo.processIdentifier)] \(Self.sanitize(event))\n".utf8)
    if !manager.fileExists(atPath: file.path) {
      manager.createFile(atPath: file.path, contents: nil, attributes: [.posixPermissions: 0o600])
    }
    guard let handle = try? FileHandle(forWritingTo: file) else { return }
    defer { try? handle.close() }
    _ = try? handle.seekToEnd()
    try? handle.write(contentsOf: record)
  }

  private static let formatter: DateFormatter = {
    let formatter = DateFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.dateFormat = "yyyy-MM-dd HH:mm:ss"
    return formatter
  }()
}

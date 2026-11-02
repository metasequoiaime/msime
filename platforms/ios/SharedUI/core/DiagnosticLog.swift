import Foundation
import Darwin
import os

@_silgen_name("msime_client_set_diagnostic_sink")
private func msime_client_set_diagnostic_sink(_ sink: (@convention(c) (UnsafePointer<CChar>?) -> Void)?)

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
    let descriptor = open(url.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
    guard descriptor >= 0 else {
      throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
    }
    let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
    defer { try? handle.close() }
    var fileStatus = stat()
    guard fstat(descriptor, &fileStatus) == 0 else {
      throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
    }
    guard fileStatus.st_mode & S_IFMT == S_IFREG, fileStatus.st_nlink == 1 else {
      throw TailReadFailure.tooLarge
    }
    let fileSize = fileStatus.st_size
    guard fileSize >= 0, fileSize <= Int64(Int.max) else { throw TailReadFailure.tooLarge }

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

  /// Route host-api's own diagnostic lines (`<category>: <detail>`, see `msime_client_set_diagnostic_sink`) into this log as `host_api: <category>`, as the macOS input method does. Only the category is kept: it is a fixed English phrase, while the detail can hold a pack id, a path or an error text. Among them is the line saying which keystroke reranker 整句联想「增强」 attached (`keyboard reranker onnx large` and so on). The sink is process-wide and set once; lines arriving while the log is off are dropped like every other record, so the line from the first session of a keyboard process, created before the log is configured, only shows up for later sessions and switch changes.
  static func forwardHostLines() {
    msime_client_set_diagnostic_sink { line in
      guard let line else { return }
      let text = String(cString: line)
      let category = text.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? ""
      guard !category.isEmpty else { return }
      DiagnosticLog.shared.write("host_api: \(category)")
    }
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
    var existing = stat()
    if lstat(file.path, &existing) == 0 &&
        ((existing.st_mode & S_IFMT) != S_IFREG || existing.st_nlink != 1) { return }
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
    var opened = stat()
    guard fstat(handle.fileDescriptor, &opened) == 0,
          (opened.st_mode & S_IFMT) == S_IFREG, opened.st_nlink == 1 else { return }
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

extension DiagnosticLog {
  /// 进程内存的一次读数，键盘把它以 `mem=`、`avail=` 字段附在加载、出现、设置应用和内存警告这几行后面（#6685）。
  ///
  /// `footprint` 取 `task_vm_info.phys_footprint`，是 iOS 判断扩展是否超出内存上限时用的那个数，也是 Xcode 内存仪表显示的数；`available` 取 `os_proc_available_memory()`，即本进程离上限还剩多少（与 `task_vm_info.limit_bytes_remaining` 同值）。按 SDK 头文件 `os/proc.h`，调用方「不是 App」或已经超出上限时它返回 0，模拟器上也是 0；键盘扩展在真机上算不算「App」还没有实测过。所以 0 不能读成「没有上限」，也不能直接读成「已经超限」：日志里把 0 写成 `avail=-`，与余量不足 1 MiB 时向下取整得到的 `avail=0` 区分开。两者都只是字节数，写进日志时取整到 MiB，不含任何输入内容。
  struct MemorySample: Equatable {
    let footprint: UInt64
    let available: UInt64

    static func current() -> MemorySample {
      var info = task_vm_info_data_t()
      var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
      let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
          task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
        }
      }
      return MemorySample(footprint: result == KERN_SUCCESS ? info.phys_footprint : 0,
                          available: UInt64(os_proc_available_memory()))
    }

    /// 字节数向下取整到 MiB。
    static func mebibytes(_ bytes: UInt64) -> UInt64 { bytes >> 20 }

    /// `mem=<MiB> avail=<MiB>`，日志行里的固定写法；系统没给出余量（`available` 为 0）时写 `avail=-`。
    var fields: String { "mem=\(Self.mebibytes(footprint)) avail=\(available == 0 ? "-" : String(Self.mebibytes(available)))" }
  }
}

import Foundation
import CoreFoundation
import Darwin

@_silgen_name("msime_client_telemetry_begin")
private func msimeClientTelemetryBegin(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_end")
private func msimeClientTelemetryEnd(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_record_crash")
private func msimeClientTelemetryRecordCrash(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_flush")
private func msimeClientTelemetryFlush(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_clear")
private func msimeClientTelemetryClear(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

/// 经 client-services 的遥测队列（msime_client.h 中的 `msime_client_telemetry_*`）进行匿名使用上报，App 和键盘通过 App Group 共用。
///
/// 键盘就是输入法本身，所以会话归它管：每次正常结束的键盘展示记一个会话，只有崩溃处理器留下了记录时才记一条 `session_crash`。App 负责发送队列里的内容，并把 MetricKit 的崩溃诊断转成崩溃记录：诊断按进程号对回键盘当时未结束的会话（键盘自己的处理程序已经记下的不再记第二份），系统强制结束（SIGKILL）按共享语义不算会话崩溃，只报 `crash`。`usage_reporting` 偏好（默认开启）关闭时，什么都不记录也不发送。
enum UsageReporting {
  static let preferenceKey = "usage_reporting"

  /// The App Group directory with the queue, the random install id, the session marker and crash records.
  static var directory: URL? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier)?
      .appendingPathComponent("MSIME/telemetry", isDirectory: true)
  }

  static func isEnabled(in preferences: [String: Any]?) -> Bool {
    preferences?[preferenceKey] as? Bool ?? true
  }

  static var isEnabled: Bool { isEnabled(in: MetasequoiaInputSessionBridge.loadSharedPreferences()) }

  /// Turns usage reporting on or off in the shared preferences; off also drops everything queued. Returns false when the preference could not be saved.
  @discardableResult
  static func setEnabled(_ enabled: Bool) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences({ $0[preferenceKey] = enabled }) else { return false }
    if !enabled, let directory { ClientRequestEnvelope.call(msimeClientTelemetryClear, ["directory": directory.path]) }
    return true
  }

  private static var version: String {
    Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
  }

  private static func request(_ directory: URL) -> [String: Any] {
    var request: [String: Any] = ["directory": directory.path, "platform": "ios", "version": version]
    let preferences = MetasequoiaInputSessionBridge.sharedStateDirectory
    if preferences.hasPrefix("/") { request["preferences_directory"] = preferences } else { request["enabled"] = true }
    return request
  }

  /// 一次已经开始的键盘会话：结束它、为它写崩溃记录都按这个 id，不看标记现在属于谁（同时可能有别的键盘进程在开始会话）。
  struct Session: Equatable {
    let id: String
    /// 信号处理函数写这个会话崩溃记录的文件。
    let crashRecordPath: String
  }

  /// Starts a keyboard session: closes the previous one, queues crash records and today's active. Nil when reporting is off. No network I/O.
  static func begin() -> Session? {
    guard let directory else { return nil }
    return begin(in: directory)
  }

  static func begin(in directory: URL) -> Session? {
    guard let value = ClientRequestEnvelope.call(msimeClientTelemetryBegin, request(directory)) as? [String: Any],
          value["enabled"] as? Bool == true,
          let id = value["session_id"] as? String,
          let path = value["crash_record_path"] as? String else { return nil }
    return Session(id: id, crashRecordPath: path)
  }

  /// The keyboard was dismissed normally: queues this session's event. No network I/O.
  static func end(session: String) {
    guard let directory else { return }
    ClientRequestEnvelope.call(msimeClientTelemetryEnd, ["directory": directory.path, "session": session])
  }

  /// Queues today's active and sends the queue. Blocks on the network: never on the main thread.
  static func flush() {
    guard let directory else { return }
    ClientRequestEnvelope.call(msimeClientTelemetryFlush, request(directory))
  }

  /// From an uncaught-exception handler in the keyboard: writes this session's crash record, sent on a later start.
  static func recordCrash(session: String, message: String, stack: String) {
    guard let directory else { return }
    ClientRequestEnvelope.call(msimeClientTelemetryRecordCrash,
                               ["directory": directory.path, "message": message, "stack": stack, "session": session])
  }

  /// MetricKit 崩溃诊断的摘要行和归类。`killed` 为真表示进程是被系统用 SIGKILL 强制结束的（看门狗、内存上限、挂起时持有 App Group 里的文件锁即 `0xdead10cc`），而不是自己出错：摘要以 `Killed` 开头，与以 `Crash` 开头的真崩溃分成不同的崩溃组。
  struct DiagnosticSummary: Equatable {
    let message: String
    let killed: Bool
  }

  /// 只用异常类型、代码、信号、ObjC 异常名和系统给出的终止原因这些数字与系统文本，不取 ObjC 异常的 `composedMessage`（它可能引用 App 里的数据）。
  static func diagnosticSummary(exceptionType: NSNumber?, exceptionCode: NSNumber?, signal: NSNumber?,
                                exceptionName: String?, terminationReason: String?) -> DiagnosticSummary {
    let killed = signal?.int32Value == SIGKILL
    var parts = [killed ? "Killed" : "Crash"]
    if let exceptionType { parts.append("exception \(exceptionType)") }
    if let exceptionCode { parts.append("code \(exceptionCode)") }
    if let signal { parts.append("signal \(signal)") }
    if let exceptionName, !exceptionName.isEmpty { parts.append(exceptionName) }
    if let terminationReason, !terminationReason.isEmpty { parts.append(terminationReason) }
    return DiagnosticSummary(message: parts.joined(separator: " ").replacingOccurrences(of: "\n", with: " "), killed: killed)
  }

  /// 系统事后报告的一次崩溃（MetricKit，App 下次打开时才送到），写成下次键盘启动时排队的崩溃记录。真崩溃按进程号 `pid` 对回该进程未结束的键盘会话，下次启动时连同那个会话的 `session_crash` 补报；键盘自己的处理程序已经为它写过记录时不再写。被强制结束的、`pid` 不可用的和不属于任何会话的（App 自身、收起后被挂起的键盘）只报 `crash`。会加锁写文件：不要在主线程上调用。
  static func recordDiagnosedCrash(_ summary: DiagnosticSummary, stack: String, pid: pid_t, in directory: URL) {
    var request: [String: Any] = ["directory": directory.path, "message": summary.message, "stack": stack]
    if summary.killed || pid <= 0 { request["outside_session"] = true } else { request["pid"] = Int(pid) }
    ClientRequestEnvelope.call(msimeClientTelemetryRecordCrash, request)
  }

  /// The frames of a MetricKit call stack tree (MXCallStackTree.jsonRepresentation()) as "index binary + offset" lines, the crashed thread's first: binary file names and offsets only, never paths or symbols from the device.
  static func frames(fromCallStackTree data: Data, limit: Int = 128) -> String {
    guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let stacks = root["callStacks"] as? [[String: Any]] else { return "" }
    let stack = stacks.first { $0["threadAttributed"] as? Bool == true } ?? stacks.first
    var lines: [String] = []
    var pending = (stack?["callStackRootFrames"] as? [[String: Any]]) ?? []
    while let frame = pending.first, lines.count < limit {
      pending.removeFirst()
      let binary = (frame["binaryName"] as? String).map { ($0 as NSString).lastPathComponent } ?? "???"
      let offset = unsignedInteger(frame["offsetIntoBinaryTextSegment"])
      lines.append("\(lines.count) \(binary) + \(offset)")
      // A frame's caller is its first sub-frame.
      pending.insert(contentsOf: (frame["subFrames"] as? [[String: Any]]) ?? [], at: 0)
    }
    return lines.joined(separator: "\n")
  }

  private static func unsignedInteger(_ value: Any?) -> UInt64 {
    SharedNumber.strictUInt64(value) ?? 0
  }

}

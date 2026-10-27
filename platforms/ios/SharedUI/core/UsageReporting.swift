import Foundation
import CoreFoundation
import CryptoKit
import Darwin
import os

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

@_silgen_name("msime_ios_crash_record_set")
private func msimeIOSCrashRecordSet(_ path: UnsafePointer<CChar>?, _ header: UnsafePointer<CChar>?)

/// 经 client-services 的遥测队列（msime_client.h 中的 `msime_client_telemetry_*`）进行匿名使用上报，App 和键盘通过 App Group 共用。
///
/// 键盘就是输入法本身，所以会话归它管：每次正常结束的键盘展示记一个会话，只有崩溃处理器留下了记录时才记一条 `session_crash`。App 负责发送队列里的内容，并把 MetricKit 的崩溃诊断转成崩溃记录：诊断按进程号对回键盘当时未结束的会话（键盘自己的处理程序已经记下的不再记第二份），App 自己的未捕获异常由它的处理器带着抛出点的栈记下（MetricKit 再送来时同样跳过），系统强制结束（SIGKILL）按共享语义不算会话崩溃，只报 `crash`。`usage_reporting` 偏好（默认开启）关闭时，什么都不记录也不发送。
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

  /// 异常回调只看内存，避免崩溃时重入共享偏好的 Rust 锁。读取中的旧值不能覆盖设置页刚保存的选择。
  struct ExceptionConsent {
    private struct State {
      var enabled = false
      var generation: UInt64 = 0
    }
    private let state = OSAllocatedUnfairLock(initialState: State())

    var isEnabled: Bool { state.withLockIfAvailable { $0.enabled } ?? false }

    func update(_ enabled: Bool) {
      state.withLock {
        $0.enabled = enabled
        $0.generation &+= 1
      }
    }

    func refresh(_ load: () -> Bool) -> Bool {
      let generation = state.withLock { $0.generation }
      let enabled = load()
      return state.withLock {
        if $0.generation == generation { $0.enabled = enabled }
        return $0.enabled
      }
    }
  }

  static let exceptionConsent = ExceptionConsent()

  static var isEnabled: Bool {
    exceptionConsent.refresh { isEnabled(in: MetasequoiaInputSessionBridge.loadSharedPreferences()) }
  }

  /// 保存上报开关并同步异常回调的内存权限；关闭时丢弃队列。偏好保存失败时返回 false。
  @discardableResult
  static func setEnabled(_ enabled: Bool) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences({ $0[preferenceKey] = enabled }) else { return false }
    exceptionConsent.update(enabled)
    if enabled {
      arm(prepareApplicationCapture())
    } else {
      msimeIOSCrashRecordSet(nil, nil)
      if let directory { ClientRequestEnvelope.call(msimeClientTelemetryClear, ["directory": directory.path]) }
    }
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
    var crashRecordHeader: String = ""
  }

  /// Starts a keyboard session: closes the previous one, queues crash records and today's active. Nil when reporting is off. No network I/O.
  static func begin() -> Session? {
    guard let directory else { return nil }
    return begin(in: directory)
  }

  static func begin(in directory: URL) -> Session? {
    var request = request(directory)
    request["component"] = "keyboard"
    guard let value = ClientRequestEnvelope.call(msimeClientTelemetryBegin, request) as? [String: Any],
          value["enabled"] as? Bool == true,
          let id = value["session_id"] as? String,
          let path = value["crash_record_path"] as? String else { return nil }
    return Session(id: id, crashRecordPath: path, crashRecordHeader: value["crash_record_header"] as? String ?? "")
  }

  /// App 只准备独立捕获，不开始或结束键盘会话。
  static func prepareApplicationCapture() -> Session? {
    guard let directory else { return nil }
    var request = request(directory)
    request["capture_only"] = true
    request["component"] = "application"
    guard let value = ClientRequestEnvelope.call(msimeClientTelemetryBegin, request) as? [String: Any],
          value["enabled"] as? Bool == true,
          let id = value["session_id"] as? String,
          let path = value["crash_record_path"] as? String,
          let header = value["crash_record_header"] as? String else { return nil }
    return Session(id: id, crashRecordPath: path, crashRecordHeader: header)
  }

  static func arm(_ session: Session?) {
    guard let session, isEnabled else { msimeIOSCrashRecordSet(nil, nil); return }
    session.crashRecordPath.withCString { path in
      session.crashRecordHeader.withCString { msimeIOSCrashRecordSet(path, $0) }
    }
  }

  /// 系统报告原版本已知而源码构建未知时明确标成 unknown，不能用升级后的身份填补。
  static func diagnosticHeader(version: String) -> String? {
    guard !version.isEmpty, version.utf8.count <= 64,
          version.utf8.allSatisfy({ (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || [45,46,95].contains($0) }) else { return nil }
    let context = ["platform": "ios", "version": version, "revision": "unknown", "run": "", "attempt": "", "arch": "unknown", "component": "system-diagnostic"]
    guard let data = try? JSONSerialization.data(withJSONObject: context, options: [.sortedKeys]),
          let json = String(data: data, encoding: .utf8) else { return nil }
    return "MSIME-CRASH/1 \(json)\n"
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
  static func recordDiagnosedCrash(_ summary: DiagnosticSummary, stack: String, pid: pid_t, version: String? = nil,
                                   diagnosticID: String? = nil, in directory: URL) {
    var request: [String: Any] = ["directory": directory.path, "message": summary.message, "stack": stack]
    if let diagnosticID { request["diagnostic_id"] = diagnosticID }
    if let version, let header = diagnosticHeader(version: version) ?? diagnosticHeader(version: "unknown") { request["crash_record_header"] = header }
    if summary.killed || pid <= 0 { request["outside_session"] = true } else { request["pid"] = Int(pid) }
    ClientRequestEnvelope.call(msimeClientTelemetryRecordCrash, request)
  }

  /// 同一份系统诊断重复送达时固定事件身份，原始诊断只参与本地散列，不上传。
  static func diagnosticID(kind: String, data: Data) -> String {
    "ios-\(kind)-" + SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
  }

  /// 主 App 的未捕获异常处理器调用：写一份不属于任何键盘会话的崩溃记录（新 id），下次键盘启动时报 `crash`；再把进程号追加到 `handledProcesses`，MetricKit 事后送来同一进程的诊断时由 `takeHandledProcess` 认出并跳过。MetricKit 只有 abort 时的线程栈，CFRunLoop 重新抛出后已经看不到抛出点，这里的栈来自异常本身。消息只有异常名，不带 `reason`（可能引用 App 里的数据）。只建文件、不加锁，可以在处理器里调用。
  static func recordUncaughtException(name: String, returnAddresses: [NSNumber], pid: pid_t, in directory: URL,
                                      handledProcesses: URL, capture: Session? = nil) {
    var request: [String: Any] = ["directory": directory.path, "message": "Uncaught \(name)",
                                "stack": frames(fromReturnAddresses: returnAddresses), "session": capture?.id ?? UUID().uuidString]
    if let capture { request["crash_record_header"] = capture.crashRecordHeader }
    guard ClientRequestEnvelope.call(msimeClientTelemetryRecordCrash, request) != nil else { return }
    let descriptor = open(handledProcesses.path, O_WRONLY | O_CREAT | O_APPEND, 0o600)
    guard descriptor >= 0 else { return }
    defer { close(descriptor) }
    let line = Array("\(pid)\n".utf8)
    _ = line.withUnsafeBytes { write(descriptor, $0.baseAddress, $0.count) }
  }

  /// 进程 `pid` 的崩溃是否已经由主 App 自己的处理器记下：是的话从 `handledProcesses` 里去掉它并返回 true。文件只留最近 32 个进程号（MetricKit 不一定送来诊断，未对上的不会一直累积）。
  static func takeHandledProcess(_ pid: pid_t, in handledProcesses: URL) -> Bool {
    guard let text = try? String(contentsOf: handledProcesses, encoding: .utf8) else { return false }
    var pids = text.split(separator: "\n").compactMap { pid_t($0) }
    let index = pids.firstIndex(of: pid)
    if let index { pids.remove(at: index) }
    pids = Array(pids.suffix(32))
    if pids.isEmpty {
      try? FileManager.default.removeItem(at: handledProcesses)
    } else {
      try? Data(pids.map { "\($0)\n" }.joined().utf8).write(to: handledProcesses, options: .atomic)
    }
    return index != nil
  }

  /// 把异常返回地址写成与 `frames(fromCallStackTree:)` 相同的「序号 二进制 + 偏移」；偏移从映像起点算，与 MetricKit 的 `offsetIntoBinaryTextSegment` 同基准，可用同一份 dSYM 符号化。
  static func frames(fromReturnAddresses addresses: [NSNumber], limit: Int = 128) -> String {
    addresses.prefix(limit).enumerated().map { index, address in
      var info = Dl_info()
      guard let pointer = UnsafeRawPointer(bitPattern: address.uintValue), dladdr(pointer, &info) != 0,
            let base = info.dli_fbase, let path = info.dli_fname else { return "\(index) ??? + 0" }
      let binary = (String(cString: path) as NSString).lastPathComponent
      return "\(index) \(binary) + \(UInt(bitPattern: pointer) - UInt(bitPattern: base))"
    }.joined(separator: "\n")
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

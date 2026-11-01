import Foundation
import CoreFoundation
import CryptoKit
import Darwin
import os

@_silgen_name("msime_client_telemetry_consent")
private func msimeClientTelemetryConsent(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
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
private func msimeIOSCrashRecordSet(_ path: UnsafePointer<CChar>?, _ header: UnsafePointer<CChar>?) -> Bool
@_silgen_name("msime_ios_crash_modules_refresh")
private func msimeIOSCrashModulesRefresh() -> Bool

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

  /// 缺字段沿用产品默认值；读盘失败和非布尔值暂时停采，不能扩大同意。
  static func consent(in preferences: [String: Any]?) -> Bool? {
    guard let preferences else { return nil }
    guard let value = preferences[preferenceKey] else { return true }
    guard let number = value as? NSNumber, CFGetTypeID(number) == CFBooleanGetTypeID() else { return nil }
    return number.boolValue
  }

  static func isEnabled(in preferences: [String: Any]?) -> Bool { consent(in: preferences) == true }

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

  struct Authorization: Equatable {
    let epoch: UUID
    let sinceUnixMilliseconds: UInt64
    let preferencesDirectory: String
    var since: Date { Date(timeIntervalSince1970: Double(sinceUnixMilliseconds) / 1000) }
  }

  enum CaptureConsent: Equatable {
    case disabled
    case enabled(Authorization)
  }

  /// 只接受共享层明确返回的布尔开关与有效代次，不从旧 UserDefaults 历史追认授权。
  static func captureConsent(from value: [String: Any]?, preferencesDirectory: String) -> CaptureConsent? {
    guard preferencesDirectory.hasPrefix("/"), let value,
          let enabled = value["enabled"] as? NSNumber, CFGetTypeID(enabled) == CFBooleanGetTypeID() else { return nil }
    if !enabled.boolValue { return .disabled }
    guard let text = value["epoch"] as? String, let epoch = UUID(uuidString: text),
          text.lowercased() == epoch.uuidString.lowercased(),
          let since = SharedNumber.strictUInt64(value["since_unix_ms"]),
          (1...253_402_300_799_999).contains(since) else { return nil }
    let canonical = Array(epoch.uuidString.utf8)
    guard canonical[14] == 52, [56, 57, 65, 66].contains(canonical[19]) else { return nil }
    return .enabled(.init(epoch: epoch, sinceUnixMilliseconds: since, preferencesDirectory: preferencesDirectory))
  }

  static func captureConsent(preferencesDirectory: String? = nil) -> CaptureConsent? {
    let preferences = preferencesDirectory ?? MetasequoiaInputSessionBridge.sharedStateDirectory
    guard preferences.hasPrefix("/") else { return nil }
    let value = ClientRequestEnvelope.call(msimeClientTelemetryConsent, ["directory": preferences]) as? [String: Any]
    return captureConsent(from: value, preferencesDirectory: preferences)
  }

  /// MetricKit 只给诊断批次的时间范围；跨越同意起点的整批保守跳过。
  static func diagnosticAllowed(begin: Date, end: Date, consentSince: Date?, now: Date = Date()) -> Bool {
    guard let consentSince else { return false }
    return begin >= consentSince && end >= begin && end <= now
  }

  static var isEnabled: Bool {
    exceptionConsent.refresh { isEnabled(in: MetasequoiaInputSessionBridge.loadSharedPreferences()) }
  }

  /// 保存上报开关并同步异常回调的内存权限；关闭时丢弃队列。偏好保存失败时返回 false。
  @discardableResult
  static func setEnabled(_ enabled: Bool) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences({ $0[preferenceKey] = enabled }) else { return false }
    exceptionConsent.update(enabled)
    // 关闭立即撤下故障快照，队列上的撤回也使紧接着重开的采集取得新身份。
    if !enabled { applicationCapture.revoke() }
    applicationCapture.refresh()
    if !enabled { clear() }
    return true
  }

  private static var version: String {
    Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
  }

  private static func request(_ directory: URL, preferencesDirectory: String? = nil,
                              authorization: Authorization? = nil) -> [String: Any]? {
    let preferences = preferencesDirectory ?? authorization?.preferencesDirectory ?? MetasequoiaInputSessionBridge.sharedStateDirectory
    // 无法确定共享偏好来源时不提交请求，也不能按“明确关闭”清掉已有记录。
    guard preferences.hasPrefix("/") else { return nil }
    let current: Authorization
    if let authorization {
      guard authorization.preferencesDirectory == preferences else { return nil }
      current = authorization
    } else {
      guard case let .enabled(loaded)? = captureConsent(preferencesDirectory: preferences) else { return nil }
      current = loaded
    }
    return ["directory": directory.path, "platform": "ios", "version": version,
            "preferences_directory": preferences, "consent_epoch": current.epoch.uuidString.lowercased()]
  }

  /// 一次已经开始的键盘会话：结束它、为它写崩溃记录都按这个 id，不看标记现在属于谁（同时可能有别的键盘进程在开始会话）。
  struct Session: Equatable {
    let id: String
    /// 信号处理函数写这个会话崩溃记录的文件。
    let crashRecordPath: String
    var crashRecordHeader: String = ""
  }

  /// 开始当前持续授权内的键盘会话；关闭或读取失败返回 nil，不联网。
  static func begin(authorization: Authorization? = nil) -> Session? {
    guard let directory else { return nil }
    return begin(in: directory, authorization: authorization)
  }

  static func begin(in directory: URL, preferencesDirectory: String? = nil,
                    authorization: Authorization? = nil) -> Session? {
    guard var request = request(directory, preferencesDirectory: preferencesDirectory, authorization: authorization) else { return nil }
    request["component"] = "keyboard"
    guard let value = ClientRequestEnvelope.call(msimeClientTelemetryBegin, request) as? [String: Any],
          value["enabled"] as? Bool == true,
          let id = value["session_id"] as? String,
          let path = value["crash_record_path"] as? String else { return nil }
    return Session(id: id, crashRecordPath: path, crashRecordHeader: value["crash_record_header"] as? String ?? "")
  }

  /// App 只准备独立捕获，不开始或结束键盘会话。
  static func prepareApplicationCapture(authorization: Authorization? = nil) -> Session? {
    guard let directory else { return nil }
    return prepareApplicationCapture(in: directory, authorization: authorization)
  }

  static func prepareApplicationCapture(in directory: URL, authorization: Authorization? = nil) -> Session? {
    guard var request = request(directory, authorization: authorization) else { return nil }
    request["capture_only"] = true
    request["component"] = "application"
    guard let value = ClientRequestEnvelope.call(msimeClientTelemetryBegin, request) as? [String: Any],
          value["enabled"] as? Bool == true,
          let id = value["session_id"] as? String,
          let path = value["crash_record_path"] as? String,
          let header = value["crash_record_header"] as? String else { return nil }
    return Session(id: id, crashRecordPath: path, crashRecordHeader: header)
  }

  /// 返回信号记录器是否已经接受这次发布；配置锁被占用时为 false，由生命周期下次重试。
  private static func configure(_ session: Session?) -> Bool {
    guard let session else { return msimeIOSCrashRecordSet(nil, nil) }
    return session.crashRecordPath.withCString { path in
      session.crashRecordHeader.withCString { msimeIOSCrashRecordSet(path, $0) }
    }
  }

  /// 配置锁被占用时没有补登记，映像数仍不一致，下一拍自然重试。
  private static func refreshModules() {
    _ = msimeIOSCrashModulesRefresh()
  }

  private static func clear() {
    if let directory { clear(in: directory) }
  }

  /// 只在共享开关当前仍关闭时清理；旧撤回回调不能删掉另一个进程重开后的队列。
  static func clear(in directory: URL, preferencesDirectory: String? = nil) {
    let preferences = preferencesDirectory ?? MetasequoiaInputSessionBridge.sharedStateDirectory
    guard preferences.hasPrefix("/") else { return }
    ClientRequestEnvelope.call(msimeClientTelemetryClear,
      ["directory": directory.path, "preferences_directory": preferences])
  }

  /// 观察与会话收尾共用独立串行队列，网络发送不能阻塞撤回或键盘收起。
  /// 暂时读失败仅解除武装并保留身份；明确关闭清队列并丢弃身份。
  final class CaptureLifecycle {
    private let queue = DispatchQueue(label: "app.msime.ios.crash-consent", qos: .utility)
    private struct Snapshot {
      var session: Session?
      var generation: UInt64 = 0
    }
    private let snapshot = OSAllocatedUnfairLock(initialState: Snapshot())
    private let load: () -> CaptureConsent?
    private let prepare: (Authorization) -> Session?
    private let configure: (Session?) -> Bool
    private let refreshModules: () -> Void
    private let clear: () -> Void
    private let finish: (Session) -> Void
    private var timer: DispatchSourceTimer?
    private var active = false
    private var session: Session?
    private var authorization: Authorization?
    private var disabled = false
    /// 信号记录器最后接受的快照，只在生命周期队列上读写；nil 表示未知或上次发布失败，下一拍重试。
    private var published: Session?? = nil

    init(load: @escaping () -> CaptureConsent?, prepare: @escaping (Authorization) -> Session?,
         configure: @escaping (Session?) -> Bool, refreshModules: @escaping () -> Void = {},
         clear: @escaping () -> Void, finish: @escaping (Session) -> Void = { _ in }) {
      self.load = load
      self.prepare = prepare
      self.configure = configure
      self.refreshModules = refreshModules
      self.clear = clear
      self.finish = finish
    }

    /// 故障路径不读盘、不等待生命周期队列；锁忙时放弃这份报告。
    var current: Session? { snapshot.withLockIfAvailable { $0.session } ?? nil }

    func start(_ ready: @escaping () -> Void = {}) {
      queue.async { [weak self] in
        guard let self else { return }
        if !self.active {
          self.active = true
          self.update()
          let timer = DispatchSource.makeTimerSource(queue: self.queue)
          timer.schedule(deadline: .now() + .milliseconds(500), repeating: .milliseconds(500))
          timer.setEventHandler { [weak self] in self?.update() }
          self.timer = timer
          timer.resume()
        }
        // 调用方只在这里向自己的网络队列投递任务，初次补录先于发送。
        ready()
      }
    }

    func refresh() { queue.async { self.update() } }

    func revoke() {
      // 本进程的设置回调无需等待下一次轮询就能停下异常和信号记录。
      snapshot.withLock {
        $0.generation &+= 1
        $0.session = nil
        _ = configure(nil)
      }
      queue.async {
        self.session = nil
        self.authorization = nil
        self.disabled = true
        self.snapshot.withLock { $0.session = nil }
        // 上面已在调用方线程撤下，队列上记下的发布状态已过时。
        self.publish(nil, always: true)
      }
    }

    func stop() {
      queue.async {
        self.active = false
        self.timer?.cancel()
        self.timer = nil
        self.snapshot.withLock { $0.session = nil }
        self.publish(nil)
        if let session = self.session { self.finish(session) }
        self.session = nil
        self.authorization = nil
        self.disabled = false
      }
    }

    private func update() {
      guard active else { return }
      let generation = snapshot.withLock { $0.generation }
      var consent = load()
      // 准备后再核对同一次持续授权；两次轮询间的关闭重开也不能沿用旧身份。
      if case let .enabled(current)? = consent {
        if authorization != current { session = nil; authorization = current }
        if session == nil {
          let prepared = prepare(current)
          let latest = load()
          if latest == consent {
            session = prepared
          } else {
            consent = latest
            session = nil
            authorization = nil
          }
        }
      }
      // 闭包在并发队列上执行，固定本轮授权值，避免捕获仍会被更新的局部变量。
      let resolvedConsent = consent
      let shouldClear = snapshot.withLock { current -> Bool in
        guard current.generation == generation else { return false }
        switch resolvedConsent {
        case .enabled?:
          disabled = false
          current.session = session
          publish(session)
        case .disabled?:
          current.session = nil
          publish(nil)
          session = nil
          authorization = nil
          let shouldClear = !disabled
          disabled = true
          return shouldClear
        case nil:
          current.session = nil
          publish(nil)
        }
        return false
      }
      // 清理会取得偏好与队列锁，不占用故障快照锁；共享层另核对当前仍明确关闭。
      if shouldClear { clear() }
    }

    /// 每次发布都会先撤下记录路径、在配置锁里重新遍历全部映像，这段时间里的故障不留记录：快照没变且上次已被接受时不再发布，只让记录器补登记之后加载的映像，否则那些映像里的帧只剩无法还原的绝对地址。
    private func publish(_ session: Session?, always: Bool = false) {
      if !always, published == .some(session) {
        if session != nil { refreshModules() }
        return
      }
      published = configure(session) ? .some(session) : nil
    }

    /// 供验证等待已排入的生命周期操作；不等待网络发送。
    func waitForPendingOperations() { queue.sync {} }
  }

  private static func loadCaptureConsent() -> CaptureConsent? {
    var loaded: CaptureConsent?
    let enabled = exceptionConsent.refresh {
      loaded = captureConsent()
      if case .enabled? = loaded { return true }
      return false
    }
    if case .disabled? = loaded { return .disabled }
    return enabled ? loaded : nil
  }

  static let applicationCapture = CaptureLifecycle(load: loadCaptureConsent, prepare: { authorization in
    let capture = prepareApplicationCapture(authorization: authorization)
    if let capture, let identities = applicationCaptureIdentities {
      rememberApplicationCapture(capture, pid: getpid(), in: identities)
    }
    return capture
  }, configure: configure, refreshModules: refreshModules, clear: clear)
  static let keyboardCapture = CaptureLifecycle(load: loadCaptureConsent,
    prepare: { begin(authorization: $0) }, configure: configure, refreshModules: refreshModules, clear: clear,
    finish: { end(session: $0.id) })

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
    guard let directory, let request = request(directory) else { return }
    ClientRequestEnvelope.call(msimeClientTelemetryFlush, request)
  }

  /// 键盘异常沿用预备身份与记录头；不在故障现场读偏好或会话状态。
  static func recordCrash(session: Session, message: String, stack: String) {
    guard let directory else { return }
    ClientRequestEnvelope.call(msimeClientTelemetryRecordCrash,
                               ["directory": directory.path, "message": message, "stack": stack, "session": session.id,
                                "crash_record_header": session.crashRecordHeader])
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
                                   diagnosticID: String? = nil, in directory: URL,
                                   authorization: Authorization, begin: Date, end: Date) {
    guard diagnosticAllowed(begin: begin, end: end, consentSince: authorization.since),
          let beginMilliseconds = milliseconds(begin), let endMilliseconds = milliseconds(end),
          let header = diagnosticHeader(version: version ?? "unknown") else { return }
    var request: [String: Any] = ["directory": directory.path, "message": summary.message, "stack": stack,
      "crash_record_header": header, "preferences_directory": authorization.preferencesDirectory,
      "consent_epoch": authorization.epoch.uuidString.lowercased(),
      "diagnostic_begin_unix_ms": beginMilliseconds, "diagnostic_end_unix_ms": endMilliseconds]
    if let diagnosticID { request["diagnostic_id"] = diagnosticID }
    if summary.killed || pid <= 0 { request["outside_session"] = true } else { request["pid"] = Int(pid) }
    ClientRequestEnvelope.call(msimeClientTelemetryRecordCrash, request)
  }

  private static func milliseconds(_ date: Date) -> UInt64? {
    let value = date.timeIntervalSince1970 * 1000
    guard value.isFinite, value >= 0, value <= 253_402_300_799_999 else { return nil }
    return UInt64(value.rounded(.down))
  }

  /// 同一份系统诊断重复送达时固定事件身份，原始诊断只参与本地散列，不上传。
  static func diagnosticID(kind: String, data: Data) -> String {
    "ios-\(kind)-" + SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
  }

  /// App 私有目录里「进程号 → 独立捕获身份」的记录：App 的信号处理函数按捕获 UUID 写现场记录而不记进程号，MetricKit 事后送来同一进程的诊断时按它取回原身份。
  static var applicationCaptureIdentities: URL? {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
      .appendingPathComponent("application-captures.json")
  }

  private struct CaptureIdentity: Codable {
    let pid: Int32
    let id: String
    let header: String
  }

  /// 准备捕获与收取诊断可能在不同队列上读改写同一个文件。
  private static let captureIdentitiesLock = NSLock()

  private static func captureIdentities(in file: URL) -> [CaptureIdentity] {
    guard let data = try? Data(contentsOf: file) else { return [] }
    return (try? JSONDecoder().decode([CaptureIdentity].self, from: data)) ?? []
  }

  private static func storeCaptureIdentities(_ identities: [CaptureIdentity], in file: URL) {
    // 只留最近 32 个进程：MetricKit 不一定送来诊断，正常退出的进程不会一直累积。
    let kept = identities.suffix(32)
    if kept.isEmpty {
      try? FileManager.default.removeItem(at: file)
      return
    }
    try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    guard let data = try? JSONEncoder().encode(Array(kept)) else { return }
    try? data.write(to: file, options: .atomic)
  }

  /// 正常执行路径上记下本进程当前的捕获身份；同一进程重新准备时替换旧的那条。
  static func rememberApplicationCapture(_ capture: Session, pid: pid_t, in file: URL) {
    captureIdentitiesLock.withLock {
      var identities = captureIdentities(in: file).filter { $0.pid != pid }
      identities.append(.init(pid: pid, id: capture.id, header: capture.crashRecordHeader))
      storeCaptureIdentities(identities, in: file)
    }
  }

  /// 取出并删掉进程 `pid` 记下的捕获身份；没有时返回 nil。
  static func takeApplicationCapture(_ pid: pid_t, in file: URL) -> Session? {
    captureIdentitiesLock.withLock {
      var identities = captureIdentities(in: file)
      guard let index = identities.lastIndex(where: { $0.pid == pid }) else { return nil }
      let identity = identities.remove(at: index)
      storeCaptureIdentities(identities, in: file)
      return Session(id: identity.id, crashRecordPath: "", crashRecordHeader: identity.header)
    }
  }

  /// App 进程的系统崩溃诊断按它准备时的捕获身份补录：信号处理函数已经写过同一 UUID 的现场记录（或已经上报过）时保留那一份，没写成时由诊断补上，一次崩溃只报一次；记录头沿用准备时的构建身份。会加锁写文件：不要在主线程上调用。
  static func recordDiagnosedCapture(_ summary: DiagnosticSummary, stack: String, capture: Session, in directory: URL,
                                     authorization: Authorization, begin: Date, end: Date) {
    guard diagnosticAllowed(begin: begin, end: end, consentSince: authorization.since),
          let beginMilliseconds = milliseconds(begin), let endMilliseconds = milliseconds(end) else { return }
    let request: [String: Any] = ["directory": directory.path, "message": summary.message, "stack": stack,
      "crash_record_header": capture.crashRecordHeader, "preferences_directory": authorization.preferencesDirectory,
      "consent_epoch": authorization.epoch.uuidString.lowercased(),
      "diagnostic_begin_unix_ms": beginMilliseconds, "diagnostic_end_unix_ms": endMilliseconds,
      "outside_session": true, "diagnostic_capture": capture.id]
    ClientRequestEnvelope.call(msimeClientTelemetryRecordCrash, request)
  }

  /// 主 App 的未捕获异常处理器调用：按当前已准备的身份写一份不属于任何键盘会话的崩溃记录，下次键盘启动时报 `crash`；再把进程号追加到 `handledProcesses`，MetricKit 事后送来同一进程的诊断时由 `takeHandledProcess` 认出并跳过。MetricKit 只有 abort 时的线程栈，CFRunLoop 重新抛出后已经看不到抛出点，这里的栈来自异常本身。消息只有异常名，不带 `reason`（可能引用 App 里的数据）。只建文件、不加锁，可以在处理器里调用。
  static func recordUncaughtException(name: String, returnAddresses: [NSNumber], pid: pid_t, in directory: URL,
                                      handledProcesses: URL, capture: Session) {
    var request: [String: Any] = ["directory": directory.path, "message": "Uncaught \(name)",
                                "stack": frames(fromReturnAddresses: returnAddresses), "session": capture.id]
    request["crash_record_header"] = capture.crashRecordHeader
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

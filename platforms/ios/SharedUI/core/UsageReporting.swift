import Foundation

private typealias UsageByte = UInt8

@_silgen_name("msime_client_telemetry_begin")
private func msimeClientTelemetryBegin(_ request: UnsafePointer<UsageByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_end")
private func msimeClientTelemetryEnd(_ request: UnsafePointer<UsageByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_record_crash")
private func msimeClientTelemetryRecordCrash(_ request: UnsafePointer<UsageByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_flush")
private func msimeClientTelemetryFlush(_ request: UnsafePointer<UsageByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_telemetry_clear")
private func msimeClientTelemetryClear(_ request: UnsafePointer<UsageByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientUsageStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// Anonymous usage reporting through client-core's telemetry queue (msime_client_telemetry_* in msime_client.h), shared by the app and the keyboard through the App Group.
///
/// The keyboard is the input method, so it owns the sessions: one per keyboard presentation that ended normally, and a session_crash only when its crash handler left a record. The app sends what is queued and turns MetricKit crash diagnostics into crash records. Nothing is recorded or sent while the usage_reporting preference (default on) is off.
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
    if !enabled, let directory { call(msimeClientTelemetryClear, ["directory": directory.path]) }
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

  /// Starts a keyboard session: closes the previous one, queues crash records and today's active. Returns the crash record path for the signal handler, or nil when reporting is off. No network I/O.
  static func begin() -> String? {
    guard let directory else { return nil }
    removeLegacyQueue()
    guard let value = call(msimeClientTelemetryBegin, request(directory)) as? [String: Any],
          value["enabled"] as? Bool == true else { return nil }
    return value["crash_record_path"] as? String
  }

  /// The keyboard was dismissed normally: queues the session event. No network I/O.
  static func end() {
    guard let directory else { return }
    call(msimeClientTelemetryEnd, ["directory": directory.path])
  }

  /// Queues today's active and sends the queue. Blocks on the network: never on the main thread.
  static func flush() {
    guard let directory else { return }
    removeLegacyQueue()
    call(msimeClientTelemetryFlush, request(directory))
  }

  /// From an uncaught-exception handler in the keyboard: writes the running session's crash record, sent on a later start.
  static func recordCrash(message: String, stack: String) {
    guard let directory else { return }
    call(msimeClientTelemetryRecordCrash, ["directory": directory.path, "message": message, "stack": stack])
  }

  /// A crash the system reported later (MetricKit), outside any session: written as a crash record under a fresh id, so the next keyboard start queues it as a crash event without counting a session_crash.
  static func storeCrashDiagnostic(message: String, stack: String, in directory: URL) {
    let crashes = directory.appendingPathComponent("telemetry-crashes", isDirectory: true)
    let file = crashes.appendingPathComponent(UUID().uuidString.lowercased() + ".crash")
    let summary = message.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? ""
    do {
      try FileManager.default.createDirectory(at: crashes, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
      FileManager.default.createFile(atPath: file.path, contents: Data((summary + "\n" + stack).utf8),
                                     attributes: [.posixPermissions: 0o600])
    } catch {}
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
      let offset = (frame["offsetIntoBinaryTextSegment"] as? NSNumber)?.uint64Value ?? 0
      lines.append("\(lines.count) \(binary) + \(offset)")
      // A frame's caller is its first sub-frame.
      pending.insert(contentsOf: (frame["subFrames"] as? [[String: Any]]) ?? [], at: 0)
    }
    return lines.joined(separator: "\n")
  }

  /// The queue of the earlier Swift reporter: per-install download events and crashes without an install id, which the server no longer wants.
  private static func removeLegacyQueue() {
    guard let group = FileManager.default.containerURL(
      forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier) else { return }
    try? FileManager.default.removeItem(at: group.appendingPathComponent("telemetry-events.json"))
  }

  /// The value of the {ok,value} envelope, or nil on any failure; reporting never affects the host.
  @discardableResult
  private static func call(_ function: (UnsafePointer<UsageByte>?, UInt) -> UnsafeMutablePointer<CChar>?,
                           _ request: [String: Any]) -> Any? {
    guard let body = try? JSONSerialization.data(withJSONObject: request) else { return nil }
    let raw = body.withUnsafeBytes { function($0.bindMemory(to: UsageByte.self).baseAddress, UInt(body.count)) }
    guard let raw else { return nil }
    defer { msimeClientUsageStringFree(raw) }
    guard let envelope = try? JSONSerialization.jsonObject(with: Data(bytes: raw, count: strlen(raw))) as? [String: Any],
          envelope["ok"] as? Bool == true else { return nil }
    return envelope["value"]
  }
}

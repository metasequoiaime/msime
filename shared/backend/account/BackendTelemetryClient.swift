import Foundation

/// Best-effort, privacy-preserving client telemetry. Events are queued on disk before the
/// network request so a transient offline period never loses a download or crash report.
public struct BackendTelemetryEvent: Codable, Equatable, Sendable {
  public let id: String
  public let kind: String
  public let platform: String
  public let version: String
  public let message: String?
  public let stack: String?

  public init(id: String = UUID().uuidString.lowercased(), kind: String, platform: String,
              version: String, message: String? = nil, stack: String? = nil) {
    self.id = id; self.kind = kind; self.platform = platform; self.version = version
    self.message = message; self.stack = stack
  }
}

public actor BackendTelemetryClient {
  public static let shared = BackendTelemetryClient()
  private let session: URLSession
  private let origin = URL(string: "https://api.msime.app")!
  private let queueURL: URL
  static let maxEvents = 64
  static let maxPayloadBytes = 64 * 1024
  static let maxResponseBytes = 64 * 1024
  // An older or foreign queue may exceed the payload bound; read it and keep what fits.
  static let maxQueueFileBytes = 1024 * 1024
  private static let queueLock = NSLock()

  public init(configuration: URLSessionConfiguration = .ephemeral, queueURL: URL? = nil) {
    let configuration = configuration.copy() as! URLSessionConfiguration
    configuration.httpCookieStorage = nil; configuration.urlCache = nil
    configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
    self.session = URLSession(configuration: configuration)
    self.queueURL = queueURL ?? Self.defaultQueueURL()
  }

  /// Record the first successful launch of this installation. The stable id makes this
  /// idempotent at the server and avoids counting every app launch as a download.
  public func recordFirstLaunch() {
    guard !UserDefaults.standard.bool(forKey: "msime.telemetry.firstLaunchRecorded") else {
      Task { await flush() }; return
    }
    UserDefaults.standard.set(true, forKey: "msime.telemetry.firstLaunchRecorded")
    enqueue(BackendTelemetryEvent(kind: "download", platform: Self.platform,
                                  version: Self.version))
    Task { await flush() }
  }

  public func recordCrash(message: String, stack: String? = nil) {
    let cleanMessage = String(message.prefix(2048))
    let cleanStack = stack.map { String($0.prefix(12_000)) }
    enqueue(BackendTelemetryEvent(kind: "crash", platform: Self.platform,
                                  version: Self.version, message: cleanMessage, stack: cleanStack))
    Task { await flush() }
  }

  /// Exception handlers run while the process is already unwinding; persist synchronously so the
  /// next launch can upload the report even if there is no time to schedule an async task.
  public nonisolated static func persistCrash(message: String, stack: String? = nil, queueURL: URL? = nil) {
    let event = BackendTelemetryEvent(kind: "crash", platform: platform, version: version,
                                      message: String(message.prefix(2048)),
                                      stack: stack.map { String($0.prefix(12_000)) })
    let url = queueURL ?? defaultQueueURL()
    queueLock.lock()
    defer { queueLock.unlock() }
    var events = readQueueUnlocked(url)
    events.append(event)
    saveUnlocked(events, at: url)
  }

  static func readQueue(_ url: URL) -> [BackendTelemetryEvent] {
    queueLock.lock()
    defer { queueLock.unlock() }
    return readQueueUnlocked(url)
  }

  private static func readQueueUnlocked(_ url: URL) -> [BackendTelemetryEvent] {
    guard let size = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.size] as? NSNumber,
          size.intValue <= maxQueueFileBytes,
          let data = try? Data(contentsOf: url),
          let events = try? JSONDecoder().decode([BackendTelemetryEvent].self, from: data) else { return [] }
    return events
  }

  /// The newest events whose JSON array fits maxPayloadBytes, encoded. A newest event too large
  /// on its own has its stack shortened rather than being dropped. Nil only if nothing fits.
  static func boundedEncode(_ events: [BackendTelemetryEvent]) -> Data? {
    let encoder = JSONEncoder()
    var kept = Array(events.suffix(maxEvents))
    guard var newest = kept.popLast() else { return try? encoder.encode(kept) }
    while let size = (try? encoder.encode(newest))?.count, size + 2 > maxPayloadBytes {
      guard let stack = newest.stack, !stack.isEmpty else { return nil }
      newest = BackendTelemetryEvent(id: newest.id, kind: newest.kind, platform: newest.platform, version: newest.version,
                                     message: newest.message, stack: String(stack.prefix(stack.count / 2)))
    }
    // Encode each event once and sum sizes, newest first: "[" + items joined by "," + "]".
    guard var total = (try? encoder.encode(newest))?.count.advanced(by: 2) else { return nil }
    var start = kept.count
    while start > 0, let size = (try? encoder.encode(kept[start - 1]))?.count, total + size + 1 <= maxPayloadBytes {
      total += size + 1; start -= 1
    }
    return try? encoder.encode(Array(kept[start...]) + [newest])
  }

  public func flush() async {
    let events = load()
    guard !events.isEmpty else { return }
    var pending: [BackendTelemetryEvent] = []
    for event in events {
      do {
        var request = URLRequest(url: origin.appendingPathComponent("v1/telemetry/events"))
        request.httpMethod = "POST"; request.timeoutInterval = 10
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("MSIME/Telemetry", forHTTPHeaderField: "User-Agent")
        request.httpBody = try JSONEncoder().encode(event)
        let (bytes, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
              http.expectedContentLength < 0 || http.expectedContentLength <= Self.maxResponseBytes else {
          throw URLError(.badServerResponse)
        }
        var count = 0
        for try await _ in bytes {
          guard count < Self.maxResponseBytes else { throw URLError(.dataLengthExceedsMaximum) }
          count += 1
        }
      } catch {
        pending.append(event)
      }
    }
    let sent = Set(events.map(\.id)).subtracting(pending.map(\.id))
    Self.mergeAfterFlush(sent: sent, pending: pending, at: queueURL)
  }

  private func enqueue(_ event: BackendTelemetryEvent) {
    Self.queueLock.lock()
    defer { Self.queueLock.unlock() }
    var events = Self.readQueueUnlocked(queueURL)
    events.append(event)
    Self.saveUnlocked(events, at: queueURL)
  }

  private func load() -> [BackendTelemetryEvent] {
    let events = Self.readQueue(queueURL)
    guard !events.isEmpty, let data = Self.boundedEncode(events),
          let bounded = try? JSONDecoder().decode([BackendTelemetryEvent].self, from: data) else { return [] }
    return bounded
  }

  private static func saveUnlocked(_ events: [BackendTelemetryEvent], at queueURL: URL) {
    guard let directory = queueURL.deletingLastPathComponent() as URL? else { return }
    do {
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
                                               attributes: [.posixPermissions: 0o700])
      guard let data = Self.boundedEncode(events) else { return }
      try data.write(to: queueURL, options: [.atomic])
      try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: queueURL.path)
    } catch { /* telemetry must never affect the host app */ }
  }

  private static func mergeAfterFlush(sent: Set<String>, pending: [BackendTelemetryEvent], at queueURL: URL) {
    queueLock.lock()
    defer { queueLock.unlock() }
    // Crash handlers can append while requests are in flight. Merge against the
    // current file so those events survive removal of successfully uploaded items.
    var current = readQueueUnlocked(queueURL).filter { !sent.contains($0.id) }
    let present = Set(current.map { $0.id })
    current.append(contentsOf: pending.filter { !present.contains($0.id) })
    saveUnlocked(current, at: queueURL)
  }

  private static var platform: String {
    #if os(iOS)
    return "ios"
    #elseif os(macOS)
    return "macos"
    #elseif os(Windows)
    return "windows"
    #elseif os(Android)
    return "android"
    #else
    return "unknown"
    #endif
  }

  private static var version: String {
    Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
  }

  private static func defaultQueueURL() -> URL {
    #if os(iOS)
    if let group = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.msime.ios") {
      return group.appendingPathComponent("telemetry-events.json")
    }
    #endif
    let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
      ?? FileManager.default.temporaryDirectory
    return base.appendingPathComponent("MSIME/telemetry-events.json")
  }
}

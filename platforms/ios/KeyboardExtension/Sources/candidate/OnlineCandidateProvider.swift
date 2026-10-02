import Foundation

/// 云候选和 AI 候选：组字停下来之后再问，不是每敲一个键都问。
///
/// The same flow as the Android host. The session builds every request - the cloud URL from the query, the AI descriptor with its credential - so this class only carries bytes and hands them back with the query document they answer. Engine has usually moved past that query by the time a provider replies; the session refuses a result for a composition that is no longer current, and the epoch drops a reply this keyboard has already stopped waiting for. The signature is what stops a second request for a state already asked about.
@MainActor
final class OnlineCandidateProvider {
  static let quietInterval: TimeInterval = 0.35
  /// `MSIME_CLOUD_CONNECT_TIMEOUT_MS` and `MSIME_CLOUD_REQUEST_TIMEOUT_MS`: a reply arriving after a private deadline is one the reference would have shown.
  static let cloudTimeout: TimeInterval = 2
  static let maxCloudResponseBytes = 256 * 1024
  static let maxAIResponseBytes = 1024 * 1024

  /// Receives the view a provider's candidates were applied to.
  var onApplied: ((MetasequoiaInputSnapshot) -> Void)?
  private let session: MetasequoiaInputSessionBridge
  private let transport: any OnlineCandidateTransport
  private var signature: String?
  private var epoch: UInt64 = 0
  private var debounce: Timer?
  private var task: Task<Void, Never>?

  static func shouldRetryAfterFetch(cloudRequested: Bool, cloudApplied: Bool,
                                    aiRequested: Bool, aiApplied: Bool) -> Bool {
    (cloudRequested && !cloudApplied) || (aiRequested && !aiApplied)
  }

  init(session: MetasequoiaInputSessionBridge,
       transport: any OnlineCandidateTransport = URLSessionOnlineCandidateTransport()) {
    self.session = session
    self.transport = transport
  }

  /// Ask again once the composition holds still, if it is one a provider could answer. `allowed` is the host's own gate: Full Access, a composition, no local mode; which schemes qualify is the Engine's call.
  func refresh(allowed: Bool) {
    guard allowed, let document = session.onlineQuery(), let query = Self.object(document),
          Self.requestsCloud(query) || Self.requestsAI(query) else { cancel(); return }
    let signature = Self.signature(query)
    guard signature != self.signature else { return }
    cancel()
    self.signature = signature
    let target = epoch
    let timer = Timer(timeInterval: Self.quietInterval, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated {
        guard let self, self.epoch == target else { return }
        self.debounce = nil
        self.task = Task { [weak self] in await self?.fetch(document: document, epoch: target) }
      }
    }
    RunLoop.main.add(timer, forMode: .common)
    debounce = timer
  }

  func cancel() {
    debounce?.invalidate()
    debounce = nil
    task?.cancel()
    task = nil
    signature = nil
    epoch &+= 1
  }

  private func fetch(document: Data, epoch target: UInt64) async {
    let initialQuery = Self.object(document)
    let cloudRequested = initialQuery.map(Self.requestsCloud) ?? false
    var cloudApplied = false
    var aiRequested = false
    var aiApplied = false
    defer {
      // 云候选成功不代表后续 AI 也成功；只要当前代次还有失败的请求，就释放签名允许重试。
      if Self.shouldRetryAfterFetch(cloudRequested: cloudRequested, cloudApplied: cloudApplied,
                                    aiRequested: aiRequested, aiApplied: aiApplied),
         target == epoch { signature = nil }
    }
    var aiDocument = document
    if let query = Self.object(document), Self.requestsCloud(query),
       let url = MetasequoiaInputSessionBridge.cloudRequestURL(query: document),
       let body = await transport.fetch(Self.cloudRequest(url)), target == epoch,
       // Applying a cloud result advances the Engine's generation, so the AI request has to be built from the query as it stands afterwards or it arrives stale.
       let refreshed = apply({ try self.session.applyCloudResponse(query: document, body: body) }) {
      cloudApplied = true
      aiDocument = refreshed
    }
    guard target == epoch, let query = Self.object(aiDocument), Self.requestsAI(query),
          let limit = Self.aiCandidateLimit(query),
          let descriptor = session.aiRequest(query: aiDocument),
          let request = Self.aiRequest(descriptor) else { return }
    aiRequested = true
    guard let body = await transport.fetch(request), target == epoch else { return }
    let candidates = MetasequoiaInputSessionBridge.parseAIResponse(body, limit: limit)
    guard !candidates.isEmpty else { return }
    if apply({ try self.session.applyOnlineCandidates(query: aiDocument, candidates: candidates, source: 1) }) != nil {
      aiApplied = true
    }
  }

  /// Hand one provider's result to the session and render it. Returns the query as it stands afterwards, or nil when nothing was applied.
  private func apply(_ call: () throws -> [String: Any]) -> Data? {
    // Online candidates are optional; a refused or failed result leaves the current view alone.
    guard let applied = try? call(), applied["applied"] as? Bool == true,
          let snapshot = try? session.snapshot(from: applied) else { return nil }
    onApplied?(snapshot)
    return session.onlineQuery()
  }

  static func requestsCloud(_ query: [String: Any]) -> Bool {
    query["cloud_candidates"] as? Bool == true && query["cloud_eligible"] as? Bool == true
  }

  static func requestsAI(_ query: [String: Any]) -> Bool {
    let assistant = query["ai_assistant"] as? [String: Any]
    return query["ai_eligible"] as? Bool == true && assistant?["enabled"] as? Bool == true
  }

  /// The assistant's candidate limit, or nil when it is outside what the shared parser accepts.
  static func aiCandidateLimit(_ query: [String: Any]) -> Int? {
    let limit = ((query["ai_assistant"] as? [String: Any])?["candidate_limit"] as? NSNumber)?.intValue ?? 0
    return (1...10).contains(limit) ? limit : nil
  }

  /// Identity of one online request. The assistant's configuration counts only while it is on: a changed model or prompt has to ask again for the same composition, while a query that only advanced its generation must not.
  static func signature(_ query: [String: Any]) -> String {
    var assistant = ""
    if let value = query["ai_assistant"] as? [String: Any], value["enabled"] as? Bool == true,
       let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) {
      assistant = String(decoding: data, as: UTF8.self)
    }
    let session = (query["session_id"] as? NSNumber)?.stringValue ?? "0"
    return "session=\(session)|cache=\(field(query["cache_key"] as? String))|identity=\(field(query["identity"] as? String))|cloud=\(query["cloud_candidates"] as? Bool == true)|assistant=\(field(assistant))"
  }

  private static func field(_ value: String?) -> String {
    let value = value ?? ""
    return "\(value.utf8.count):\(value)"
  }

  static func cloudRequest(_ url: URL) -> OnlineCandidateRequest {
    var request = URLRequest(url: url)
    request.httpMethod = "GET"
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    return OnlineCandidateRequest(urlRequest: request, connectTimeout: cloudTimeout, timeout: cloudTimeout,
                                  maxBytes: maxCloudResponseBytes)
  }

  /// The POST the session's descriptor describes, or nil when it is not an HTTPS POST with a JSON body.
  static func aiRequest(_ descriptor: [String: Any]) -> OnlineCandidateRequest? {
    guard let text = descriptor["url"] as? String, let url = URL(string: text), url.scheme == "https",
          (descriptor["method"] as? String ?? "POST") == "POST",
          let body = descriptor["body"], JSONSerialization.isValidJSONObject(body),
          let payload = try? JSONSerialization.data(withJSONObject: body) else { return nil }
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.httpBody = payload
    for (name, value) in descriptor["headers"] as? [String: String] ?? [:] {
      request.setValue(value, forHTTPHeaderField: name)
    }
    let maxBytes = min(maxAIResponseBytes, (descriptor["max_response_bytes"] as? NSNumber)?.intValue ?? maxAIResponseBytes)
    return OnlineCandidateRequest(urlRequest: request,
                                  connectTimeout: seconds(descriptor["connect_timeout_ms"], fallback: 2500),
                                  timeout: seconds(descriptor["timeout_ms"], fallback: 8000),
                                  maxBytes: max(1, maxBytes))
  }

  /// Milliseconds from the descriptor, held to the 1-10 s the Android host allows.
  private static func seconds(_ value: Any?, fallback: Int) -> TimeInterval {
    let milliseconds = (value as? NSNumber)?.intValue ?? fallback
    return TimeInterval(min(10_000, max(1_000, milliseconds))) / 1000
  }

  private static func object(_ document: Data) -> [String: Any]? {
    (try? JSONSerialization.jsonObject(with: document)) as? [String: Any]
  }
}

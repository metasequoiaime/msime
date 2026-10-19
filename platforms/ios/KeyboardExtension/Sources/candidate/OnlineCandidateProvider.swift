import Foundation
import CoreFoundation

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
  /// 问过 AI 却没有结果的那次请求的签名：同一组字不再向 AI 发请求；组字一变就清掉（#6888）。只读地公开给测试，用来确认空回答已经处理完。
  private(set) var aiResolvedSignature: String?
  private var epoch: UInt64 = 0
  private var debounce: Timer?
  private var task: Task<Void, Never>?

  /// 只有云候选失败才释放签名、在下一次渲染时重试。AI 没有结果时同一组字不再问（#6888），否则键盘每次重画都会再发一次请求；重试时也不再问 AI，见 `aiResolvedSignature`。
  static func shouldRetryAfterFetch(cloudRequested: Bool, cloudApplied: Bool) -> Bool {
    cloudRequested && !cloudApplied
  }

  init(session: MetasequoiaInputSessionBridge,
       transport: any OnlineCandidateTransport = URLSessionOnlineCandidateTransport()) {
    self.session = session
    self.transport = transport
  }

  /// Ask again once the composition holds still, if it is one a provider could answer. `allowed` is the host's own gate: Full Access, a composition, no local mode; which schemes qualify is the Engine's call.
  func refresh(allowed: Bool) {
    guard allowed, let document = session.onlineQuery(), let query = Self.object(document),
          Self.requestsCloud(query) || Self.requestsAI(query) else {
      cancel()
      // 组字变到一个问不了的状态（例如拼音没打完）：回到原来的组字时重新问 AI。
      aiResolvedSignature = nil
      // 宿主不让问（没有完全访问权限等）而组字本可以问 AI：照旧告诉 Host API 这次没有回答。共享内核已经不放占位行，这个调用答 applied:false，什么都不画（#6888）。
      if !allowed, let document = session.onlineQuery(), let query = Self.object(document),
         Self.requestsAI(query) {
        _ = apply { try self.session.resolveOnlineCandidates(query: document, source: 1) }
      }
      return
    }
    let signature = Self.signature(query)
    guard signature != self.signature else { return }
    cancel()
    self.signature = signature
    if aiResolvedSignature != signature { aiResolvedSignature = nil }
    let target = epoch
    let timer = Timer(timeInterval: Self.quietInterval, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated {
        guard let self, self.epoch == target else { return }
        self.debounce = nil
        self.task = Task { [weak self] in await self?.fetch(document: document, epoch: target, signature: signature) }
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

  private func fetch(document: Data, epoch target: UInt64, signature requestSignature: String) async {
    let initialQuery = Self.object(document)
    let cloudRequested = initialQuery.map(Self.requestsCloud) ?? false
    var cloudApplied = false
    var aiApplied = false
    defer {
      // 云候选失败时释放签名，下一次渲染重试；AI 没有结果时不为它重试。
      if Self.shouldRetryAfterFetch(cloudRequested: cloudRequested, cloudApplied: cloudApplied),
         target == epoch { signature = nil }
    }
    var aiDocument = document
    if let query = Self.object(document), Self.requestsCloud(query),
       let descriptor = query["cloud_request"] as? [String: Any],
       let request = Self.cloudRequest(descriptor),
       let body = await cloudBody(descriptor, request: request), target == epoch,
       // Applying a cloud result advances the Engine's generation, so the AI request has to be built from the query as it stands afterwards or it arrives stale.
       let refreshed = apply({ try self.session.applyCloudResponse(query: document, body: body) }) {
      cloudApplied = true
      aiDocument = refreshed
    }
    guard target == epoch, let query = Self.object(aiDocument), Self.requestsAI(query) else { return }
    defer {
      // 向 AI 发问了却没有采用任何回答（请求发不出去、失败、空回答、被拒）：告诉 Host API 这次没有回答（共享内核已经不放占位行，答 applied:false，候选不变），同一组字不再问（#6888）。
      if !aiApplied, target == epoch {
        _ = apply { try self.session.resolveOnlineCandidates(query: aiDocument, source: 1) }
        aiResolvedSignature = requestSignature
      }
    }
    // 这个组字问过 AI 而没有结果（云候选失败触发的重试走到这里）：不再发请求。
    guard aiResolvedSignature != requestSignature,
          let limit = Self.aiCandidateLimit(query),
          let descriptor = session.aiRequest(query: aiDocument),
          let request = Self.aiRequest(descriptor) else { return }
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
    let limit = SharedNumber.strictInt((query["ai_assistant"] as? [String: Any])?["candidate_limit"]) ?? 0
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
    let provider = (query["cloud_request"] as? [String: Any])?["provider_id"] as? String
    return "cloud_provider=\(field(provider))|session=\(session)|cache=\(field(query["cache_key"] as? String))|identity=\(field(query["identity"] as? String))|cloud=\(query["cloud_candidates"] as? Bool == true)|cloud_eligible=\(query["cloud_eligible"] as? Bool == true)|ai_eligible=\(query["ai_eligible"] as? Bool == true)|assistant=\(field(assistant))"
  }

  private static func field(_ value: String?) -> String {
    let value = value ?? ""
    return "\(value.utf8.count):\(value)"
  }

  private func cloudBody(_ descriptor: [String: Any], request: OnlineCandidateRequest) async -> Data? {
    if let cached = descriptor["cached_response"] as? String {
      let data = Data(cached.utf8)
      return data.count <= Self.maxCloudResponseBytes ? data : nil
    }
    return await transport.fetch(request)
  }

  /// 云候选只执行共享层的字节描述符，模板与响应路径由 Rust 处理。
  static func cloudRequest(_ descriptor: [String: Any]) -> OnlineCandidateRequest? {
    guard let address = descriptor["url"] as? String, let url = AIEndpointPolicy.validatedURL(address),
          let method = descriptor["method"] as? String, ["GET", "POST"].contains(method),
          let timeout = SharedNumber.strictInt(descriptor["timeout_ms"]), (100...10000).contains(timeout),
          let connect = SharedNumber.strictInt(descriptor["connect_timeout_ms"]), (1...timeout).contains(connect),
          SharedNumber.strictInt(descriptor["max_response_bytes"]) == maxCloudResponseBytes,
          let body = descriptor["body_utf8"] as? String, body.utf8.count <= 8192,
          let headers = descriptor["headers"] as? [String: String] else { return nil }
    var request = URLRequest(url: url)
    request.httpMethod = method
    request.allHTTPHeaderFields = headers
    request.httpShouldHandleCookies = false
    if method == "POST" { request.httpBody = Data(body.utf8) }
    return OnlineCandidateRequest(urlRequest: request, allowsLocalHTTP: true,
                                  connectTimeout: Double(connect) / 1000, timeout: Double(timeout) / 1000,
                                  maxBytes: maxCloudResponseBytes)
  }

  static func cloudRequest(_ url: URL) -> OnlineCandidateRequest {
    var request = URLRequest(url: url)
    request.httpMethod = "GET"
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    return OnlineCandidateRequest(urlRequest: request, connectTimeout: cloudTimeout, timeout: cloudTimeout,
                                  maxBytes: maxCloudResponseBytes)
  }

  /// The POST the session's descriptor describes, or nil when it is not a POST with a JSON body to an address `AIEndpointPolicy` accepts (HTTPS, or HTTP on the local network).
  static func aiRequest(_ descriptor: [String: Any]) -> OnlineCandidateRequest? {
    guard let text = descriptor["url"] as? String, let url = AIEndpointPolicy.validatedURL(text),
          (descriptor["method"] as? String ?? "POST") == "POST",
          let body = descriptor["body"], JSONSerialization.isValidJSONObject(body),
          let payload = try? JSONSerialization.data(withJSONObject: body) else { return nil }
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.httpBody = payload
    for (name, value) in descriptor["headers"] as? [String: String] ?? [:] {
      request.setValue(value, forHTTPHeaderField: name)
    }
    let maxBytes: Int
    if descriptor["max_response_bytes"] == nil {
      maxBytes = maxAIResponseBytes
    } else {
      guard let value = SharedNumber.strictInt(descriptor["max_response_bytes"]), value > 0 else { return nil }
      maxBytes = min(maxAIResponseBytes, value)
    }
    guard let connectTimeout = seconds(descriptor["connect_timeout_ms"], fallback: 2500),
          let requestTimeout = seconds(descriptor["timeout_ms"], fallback: 8000) else { return nil }
    return OnlineCandidateRequest(urlRequest: request,
                                  allowsLocalHTTP: true,
                                  connectTimeout: connectTimeout,
                                  timeout: requestTimeout,
                                  maxBytes: max(1, maxBytes))
  }

  /// Milliseconds from the descriptor, held to the 1-10 s the Android host allows.
  private static func seconds(_ value: Any?, fallback: Int) -> TimeInterval? {
    let milliseconds = value == nil ? fallback : SharedNumber.strictInt(value)
    guard let milliseconds, milliseconds > 0 else { return nil }
    return OnlineRequestPolicy.timeout(milliseconds: milliseconds)
  }

  private static func object(_ document: Data) -> [String: Any]? {
    (try? JSONSerialization.jsonObject(with: document)) as? [String: Any]
  }
}

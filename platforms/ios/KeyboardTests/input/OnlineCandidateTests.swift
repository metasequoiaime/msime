import XCTest

/// Cloud and AI candidates: the session builds the request and judges the reply, the keyboard only carries bytes. Cloud candidates are opt-in on iOS whatever the shared document says.
@MainActor
final class OnlineCandidateTests: XCTestCase {
  private static let cloudBody = Data(#"["SUCCESS", [["nihao", ["泥壕云"]]]]"#.utf8)
  private var state: URL!
  private var storedPreference: Any?

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-online-candidates-\(UUID().uuidString)", isDirectory: true)
    storedPreference = CloudCandidatePreference.defaults.object(forKey: CloudCandidatePreference.key)
  }

  override func tearDown() {
    CloudCandidatePreference.defaults.set(storedPreference, forKey: CloudCandidatePreference.key)
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testACloudReplyJoinsTheCandidates() throws {
    CloudCandidatePreference.enabled = true
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    type(bridge, "nihao")
    let document = try XCTUnwrap(bridge.onlineQuery(), "nihao has no online query")
    let query = try XCTUnwrap(try JSONSerialization.jsonObject(with: document) as? [String: Any])
    XCTAssertTrue(OnlineCandidateProvider.requestsCloud(query), "\(query)")
    let url = try XCTUnwrap(MetasequoiaInputSessionBridge.cloudRequestURL(query: document))
    XCTAssertEqual(url.scheme, "https")

    let applied = try bridge.applyCloudResponse(query: document, body: Self.cloudBody)
    XCTAssertEqual(applied["applied"] as? Bool, true)
    XCTAssertTrue(try bridge.snapshot(from: applied).candidates.contains("泥壕云"))
  }

  func testCloudCandidatesStayOffUntilTheSwitchIsOn() async throws {
    CloudCandidatePreference.enabled = false
    // A document synced from a desktop, where cloud candidates are on.
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { $0["cloud_candidates"] = true })
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    type(bridge, "nihao")
    if let document = bridge.onlineQuery() {
      let query = try XCTUnwrap(try JSONSerialization.jsonObject(with: document) as? [String: Any])
      XCTAssertFalse(OnlineCandidateProvider.requestsCloud(query))
      XCTAssertNil(MetasequoiaInputSessionBridge.cloudRequestURL(query: document))
      XCTAssertEqual(try bridge.applyCloudResponse(query: document, body: Self.cloudBody)["applied"] as? Bool, false)
    }

    // Turning the switch on reaches the live session on its next reload, and never rewrites the shared document.
    CloudCandidatePreference.enabled = true
    let reloaded = expectation(description: "reload")
    bridge.reloadSharedPreferences { _ in reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    type(bridge, "nihao")
    let document = try XCTUnwrap(bridge.onlineQuery())
    let query = try XCTUnwrap(try JSONSerialization.jsonObject(with: document) as? [String: Any])
    XCTAssertTrue(OnlineCandidateProvider.requestsCloud(query))

    CloudCandidatePreference.enabled = false
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { $0["cloud_candidates"] = true })
    let turnedOff = expectation(description: "reload off")
    bridge.reloadSharedPreferences { _ in turnedOff.fulfill() }
    await fulfillment(of: [turnedOff], timeout: 15)
    type(bridge, "nihao")
    let offDocument = bridge.onlineQuery()
    let offQuery = offDocument.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
    XCTAssertFalse(offQuery.map(OnlineCandidateProvider.requestsCloud) ?? false)
    XCTAssertEqual(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["cloud_candidates"] as? Bool, true)
  }

  func testAReplyForAnAbandonedCompositionIsRefused() throws {
    CloudCandidatePreference.enabled = true
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    type(bridge, "nihao")
    let document = try XCTUnwrap(bridge.onlineQuery())
    type(bridge, "shijie")
    let applied = try bridge.applyCloudResponse(query: document, body: Self.cloudBody)
    XCTAssertEqual(applied["applied"] as? Bool, false)
    XCTAssertFalse(try bridge.snapshot(from: applied).candidates.contains("泥壕云"))
  }

  func testTheProviderAsksOnceThePauseEndsAndRendersTheReply() throws {
    CloudCandidatePreference.enabled = true
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let transport = RecordingTransport(body: Self.cloudBody)
    let provider = OnlineCandidateProvider(session: bridge, transport: transport)
    let rendered = expectation(description: "rendered")
    var candidates: [String] = []
    provider.onApplied = { candidates = $0.candidates; rendered.fulfill() }

    type(bridge, "nihao")
    provider.refresh(allowed: true)
    // The same composition asked about again is not a second request.
    provider.refresh(allowed: true)
    wait(for: [rendered], timeout: 5)
    XCTAssertTrue(candidates.contains("泥壕云"))
    let requests = transport.requests
    XCTAssertEqual(requests.count, 1)
    XCTAssertEqual(requests.first?.urlRequest.httpMethod, "GET")
    XCTAssertEqual(requests.first?.urlRequest.url?.scheme, "https")
    XCTAssertEqual(requests.first?.timeout, 2)
    XCTAssertEqual(requests.first?.maxBytes, 256 * 1024)
  }

  func testAFailedRequestCanBeRetriedForTheSameComposition() throws {
    CloudCandidatePreference.enabled = true
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let transport = FlakyTransport(body: Self.cloudBody)
    let provider = OnlineCandidateProvider(session: bridge, transport: transport)

    type(bridge, "nihao")
    let firstCall = expectation(description: "first request")
    transport.onCall = { if $0 == 1 { firstCall.fulfill() } }
    provider.refresh(allowed: true)
    wait(for: [firstCall], timeout: 5)
    RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    XCTAssertEqual(transport.callCount, 1)

    let rendered = expectation(description: "rendered after retry")
    provider.onApplied = { _ in rendered.fulfill() }
    provider.refresh(allowed: true)
    wait(for: [rendered], timeout: 5)
    XCTAssertEqual(transport.callCount, 2)
    provider.cancel()
  }

  func testNothingIsAskedWithoutTheHostsPermission() {
    CloudCandidatePreference.enabled = true
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let transport = RecordingTransport(body: Self.cloudBody)
    let provider = OnlineCandidateProvider(session: bridge, transport: transport)
    provider.onApplied = { _ in XCTFail("nothing should be applied") }
    type(bridge, "nihao")
    provider.refresh(allowed: false)
    RunLoop.main.run(until: Date().addingTimeInterval(OnlineCandidateProvider.quietInterval + 0.3))
    XCTAssertTrue(transport.requests.isEmpty)
  }

  func testTheAIDescriptorMustBeAnHTTPSPost() {
    let descriptor: [String: Any] = [
      "url": "https://example.invalid/v1/chat/completions", "method": "POST",
      "headers": ["Authorization": "Bearer secret", "Content-Type": "application/json"],
      "body": ["model": "m"], "timeout_ms": 8000, "connect_timeout_ms": 2500, "max_response_bytes": 1_048_576,
    ]
    let request = OnlineCandidateProvider.aiRequest(descriptor)
    XCTAssertEqual(request?.urlRequest.httpMethod, "POST")
    XCTAssertEqual(request?.urlRequest.value(forHTTPHeaderField: "Authorization"), "Bearer secret")
    XCTAssertEqual(request?.timeout, 8)
    XCTAssertEqual(request?.connectTimeout, 2.5)
    XCTAssertEqual(request?.maxBytes, 1_048_576)

    var plain = descriptor
    plain["url"] = "http://example.invalid/v1/chat/completions"
    XCTAssertNil(OnlineCandidateProvider.aiRequest(plain))
    var get = descriptor
    get["method"] = "GET"
    XCTAssertNil(OnlineCandidateProvider.aiRequest(get))
  }

  func testTheSignatureIgnoresTheGenerationButNotTheAssistant() {
    let query: [String: Any] = [
      "session_id": 1, "generation": 3, "cache_key": "nihao", "identity": "q", "cloud_candidates": true,
      "ai_assistant": ["enabled": true, "model": "a"],
    ]
    var advanced = query
    advanced["generation"] = 4
    XCTAssertEqual(OnlineCandidateProvider.signature(query), OnlineCandidateProvider.signature(advanced))
    var otherModel = query
    otherModel["ai_assistant"] = ["enabled": true, "model": "b"]
    XCTAssertNotEqual(OnlineCandidateProvider.signature(query), OnlineCandidateProvider.signature(otherModel))
    var splitCache = query
    splitCache["cache_key"] = "a:b"
    splitCache["identity"] = "c"
    var splitIdentity = query
    splitIdentity["cache_key"] = "a"
    splitIdentity["identity"] = "b:c"
    XCTAssertNotEqual(OnlineCandidateProvider.signature(splitCache), OnlineCandidateProvider.signature(splitIdentity))
  }

  func testCloudSuccessStillAllowsRetryWhenAIHasNoAnswer() {
    XCTAssertTrue(
      OnlineCandidateProvider.shouldRetryAfterFetch(
        cloudRequested: true, cloudApplied: true, aiRequested: true, aiApplied: false),
      "云候选成功但 AI 没有结果时必须释放签名，以便同一组字重试 AI")
    XCTAssertFalse(
      OnlineCandidateProvider.shouldRetryAfterFetch(
        cloudRequested: true, cloudApplied: true, aiRequested: true, aiApplied: true))
    XCTAssertFalse(
      OnlineCandidateProvider.shouldRetryAfterFetch(
        cloudRequested: true, cloudApplied: true, aiRequested: false, aiApplied: false))
  }

  private func type(_ bridge: MetasequoiaInputSessionBridge, _ letters: String) {
    _ = bridge.cancel()
    for letter in letters { _ = bridge.handleCharacter(String(letter)) }
  }
}

private final class RecordingTransport: OnlineCandidateTransport, @unchecked Sendable {
  private let body: Data
  private let lock = NSLock()
  private var recorded: [OnlineCandidateRequest] = []

  init(body: Data) { self.body = body }

  var requests: [OnlineCandidateRequest] { lock.withLock { recorded } }

  func fetch(_ request: OnlineCandidateRequest) async -> Data? {
    lock.withLock { recorded.append(request) }
    return body
  }
}

private final class FlakyTransport: OnlineCandidateTransport, @unchecked Sendable {
  private let body: Data
  private let lock = NSLock()
  private var calls = 0
  var onCall: ((Int) -> Void)?

  init(body: Data) { self.body = body }

  var callCount: Int { lock.withLock { calls } }

  func fetch(_ request: OnlineCandidateRequest) async -> Data? {
    let call = lock.withLock {
      calls += 1
      return calls
    }
    if call == 1 {
      // 在提供器处理完失败返回后再通知测试，避免把仍在执行的请求误当成已结束。
      DispatchQueue.main.async { [weak self] in self?.onCall?(call) }
    } else {
      onCall?(call)
    }
    return call == 1 ? nil : body
  }
}

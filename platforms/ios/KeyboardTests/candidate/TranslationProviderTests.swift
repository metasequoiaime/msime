import XCTest

/// 账号释义的服务端：旧令牌被拒（401），刷新换新令牌，新令牌按 `reply` 回答；记下每次 `/v1/translate` 的请求体和令牌。
private final class AccountGlossProtocol: URLProtocol {
  static let oldToken = String(repeating: "a", count: 64)
  static let newToken = String(repeating: "c", count: 64)
  private static let lock = NSLock()
  private static var recorded: [(authorization: String, body: [String: Any])] = []
  nonisolated(unsafe) static var reply = #"{"code":200,"data":["hello"]}"#

  static var translations: [(authorization: String, body: [String: Any])] { lock.lock(); defer { lock.unlock() }; return recorded }
  static func reset(reply: String) { lock.lock(); recorded = []; Self.reply = reply; lock.unlock() }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var data = request.httpBody ?? Data()
    if data.isEmpty, let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var buffer = [UInt8](repeating: 0, count: 1024)
      while stream.hasBytesAvailable { let n = stream.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; data.append(contentsOf: buffer.prefix(n)) }
    }
    let authorization = request.value(forHTTPHeaderField: "Authorization") ?? ""
    var status = 200
    var body = "{}"
    switch request.url?.path {
    case "/v1/translate":
      Self.lock.lock()
      Self.recorded.append((authorization, (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]))
      Self.lock.unlock()
      if authorization == "Bearer \(Self.oldToken)" { status = 401; body = #"{"error":{"code":"unauthorized"}}"# } else { body = Self.reply }
    case "/v1/auth/refresh":
      body = "{\"access_token\":\"\(Self.newToken)\",\"refresh_token\":\"\(String(repeating: "d", count: 64))\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"signed-in-user\",\"display_name\":\"示例\",\"created_at\":\"2026-09-08\"}}"
    default: status = 404
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class CandidateSessionStorage: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?

  init(_ value: BackendSavedSession?) { self.value = value }
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
}

private final class CandidateAccountTranslationProtocol: URLProtocol {
  static let accountToken = String(repeating: "a", count: 64)
  private static let lock = NSLock()
  private static var authorizations: [String] = []

  static var recordedAuthorizations: [String] {
    lock.lock(); defer { lock.unlock() }
    return authorizations
  }

  static func reset() {
    lock.lock(); defer { lock.unlock() }
    authorizations = []
  }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

  override func startLoading() {
    let authorization = request.value(forHTTPHeaderField: "Authorization") ?? ""
    Self.lock.lock()
    Self.authorizations.append(authorization)
    Self.lock.unlock()
    let response: HTTPURLResponse
    let body: String
    if request.url?.path == "/v1/translate" {
      response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                 headerFields: ["Content-Type": "application/json"])!
      body = #"{"code":200,"data":["hello"]}"#
    } else {
      response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil,
                                 headerFields: ["Content-Type": "application/json"])!
      body = #"{"error":{"code":"not_found"}}"#
    }
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }

  override func stopLoading() {}
}

private final class RecordingTransport: OnlineCandidateTransport, @unchecked Sendable {
  private let lock = NSLock()
  private var recorded: [URLRequest] = []
  let reply: @Sendable (URLRequest) -> Data?
  init(reply: @escaping @Sendable (URLRequest) -> Data?) { self.reply = reply }
  var requests: [URLRequest] { lock.withLock { recorded } }
  func fetch(_ request: OnlineCandidateRequest) async -> Data? {
    lock.withLock { recorded.append(request.urlRequest) }
    return reply(request.urlRequest)
  }
}

final class TranslationProviderTests: XCTestCase {
  private let fixedDate: @Sendable () -> Date = { Date(timeIntervalSince1970: 1_700_000_000) }

  func testRouteFollowsTheSharedPrecedence() {
    XCTAssertEqual(TranslationProviderPreference.route(in: nil), .none)
    // Tencent is enabled by default but only usable secrets make it the route.
    XCTAssertEqual(TranslationProviderPreference.route(in: ["tencent_tmt": ["enabled": true, "secret_id": "<id>", "secret_key": "k"]]), .none)
    XCTAssertEqual(TranslationProviderPreference.route(in: ["translation_account": true]), .account)
    XCTAssertEqual(TranslationProviderPreference.route(in: ["tencent_tmt": ["secret_id": "AKID1", "secret_key": "key", "region": ""]]),
                   .tencent(secretID: "AKID1", secretKey: "key", region: "ap-guangzhou"))
    let everything: [String: Any] = [
      "niutrans": ["enabled": true, "app_id": "app", "apikey": "key"],
      "custom_translation": ["enabled": true, "endpoint": "https://example.com/t", "api_key": ""],
      "tencent_tmt": ["enabled": true, "secret_id": "AKID1", "secret_key": "key"],
    ]
    XCTAssertEqual(TranslationProviderPreference.route(in: everything), .niutrans(appID: "app", apiKey: "key"))
    var customOnly = everything
    customOnly["niutrans"] = ["enabled": false]
    XCTAssertEqual(TranslationProviderPreference.route(in: customOnly), .custom(endpoint: "https://example.com/t", apiKey: ""))
  }

  func testAChosenProviderWithoutCredentialsNeverFallsBack() {
    let niutrans: [String: Any] = [
      "niutrans": ["enabled": true, "app_id": "FAKESECRET_app", "apikey": "key"],
      "tencent_tmt": ["enabled": true, "secret_id": "AKID1", "secret_key": "key"],
    ]
    XCTAssertEqual(TranslationProviderPreference.route(in: niutrans), .none)
    XCTAssertEqual(TranslationProviderPreference.route(in: ["custom_translation": ["enabled": true, "endpoint": "  "]]), .none)
  }

  /// 自定义服务是否可用由共享请求构造决定：DeepL 官方地址没有密钥时不构造请求，所以不算可用，设置页显示「凭据不完整」；DeepLX 地址不需要密钥。
  func testACustomEndpointIsUsableOnlyWhenTheSharedBuilderAcceptsIt() {
    func route(_ endpoint: String, _ apiKey: String) -> TranslationRoute {
      TranslationProviderPreference.route(in: ["custom_translation": ["enabled": true, "endpoint": endpoint, "api_key": apiKey]])
    }
    XCTAssertEqual(route("https://api-free.deepl.com/v2/translate", ""), .none)
    XCTAssertEqual(route("https://api.deepl.com/v2/translate", "  "), .none)
    XCTAssertEqual(route("https://api-free.deepl.com/v2/translate", "synthetic-key:fx"),
                   .custom(endpoint: "https://api-free.deepl.com/v2/translate", apiKey: "synthetic-key:fx"))
    XCTAssertEqual(route("https://deeplx.example/translate", ""), .custom(endpoint: "https://deeplx.example/translate", apiKey: ""))
    XCTAssertEqual(route("http://example.com/t", ""), .none, "the shared builder refuses remote cleartext, so the keyboard would send nothing")
  }

  func testSelectKeepsOnlyOneProviderEnabledAndRoundTrips() {
    var document: [String: Any] = ["tencent_tmt": ["enabled": true, "secret_id": "old", "secret_key": "old"]]
    TranslationProviderPreference.select(.custom, niutrans: (" app ", "key"), tencent: ("AKID1", "key", ""),
                                         custom: (" https://example.com/t ", "token"), in: &document)
    XCTAssertEqual(TranslationProviderPreference.route(in: document), .custom(endpoint: "https://example.com/t", apiKey: "token"))
    XCTAssertEqual(TranslationProviderPreference.selected(in: document), .custom)
    XCTAssertEqual((document["niutrans"] as? [String: Any])?["app_id"] as? String, "app", "other credentials are kept, trimmed")
    TranslationProviderPreference.select(.account, niutrans: ("app", "key"), tencent: ("AKID1", "key", "ap-beijing"),
                                         custom: ("https://example.com/t", "token"), in: &document)
    XCTAssertEqual(TranslationProviderPreference.route(in: document), .account, "Tencent is disabled explicitly, not by losing its secrets")
    XCTAssertEqual(TranslationProviderPreference.selected(in: document), .account)
    XCTAssertEqual(document["translation_account"] as? Bool, true)
    XCTAssertEqual((document["tencent_tmt"] as? [String: Any])?["secret_id"] as? String, "AKID1")
    TranslationProviderPreference.select(.off, niutrans: ("app", "key"), tencent: ("AKID1", "key", "ap-beijing"),
                                         custom: ("https://example.com/t", "token"), in: &document)
    XCTAssertNil(document["translation_account"])
    XCTAssertEqual(TranslationProviderPreference.route(in: document), .none, "turning translation off sends nothing anywhere")
    XCTAssertEqual(TranslationProviderPreference.selected(in: document), .off)
  }

  /// The 水杉 account is reached only by choosing it: no document, an untouched one, a Tencent placeholder or an explicit no all stay offline, and a chosen but incomplete provider does not fall back to the account either.
  func testNoChoiceNeverRoutesToTheAccount() {
    let documents: [[String: Any]?] = [
      nil,
      [:],
      ["tencent_tmt": ["enabled": true, "secret_id": "<SecretId>", "secret_key": "FAKESECRET_key"]],
      ["translation_account": false],
    ]
    for document in documents {
      XCTAssertEqual(TranslationProviderPreference.route(in: document), .none, "\(String(describing: document))")
      XCTAssertEqual(TranslationProviderPreference.selected(in: document), .off, "\(String(describing: document))")
    }
    let incomplete: [String: Any] = [
      "translation_account": true,
      "niutrans": ["enabled": true, "app_id": "", "apikey": "key"],
    ]
    XCTAssertEqual(TranslationProviderPreference.route(in: incomplete), .none, "the account is never a fallback for the user's own service")
  }

  /// 「AI 辅助」要显式选择（`translation_ai`），与共享层 `selected_translation_services` 相同：候选栏 AI 候选（`ai_assistant.enabled`）开没开不参与；水杉账号和用户自己的服务排在它前面，选中的服务凭据不全时也不改用它；腾讯启用而没有密钥时共享层照样选中腾讯、什么都不发。
  func testAIIsUsedOnlyWhenChosenExplicitly() {
    let ai: [String: Any] = ["enabled": false, "provider": "openAI", "endpoint": "https://api.openai.com/v1/chat/completions",
                             "model": "synthetic-model"]
    let expected = TranslationRoute.ai(provider: "openAI", endpoint: "https://api.openai.com/v1/chat/completions",
                                       model: "synthetic-model", token: "")
    let chosen: [String: Any] = ["translation_ai": true, "tencent_tmt": ["enabled": false], "ai_assistant": ai]
    XCTAssertEqual(TranslationProviderPreference.route(in: chosen), expected, "AI candidates being off does not matter")
    XCTAssertEqual(TranslationProviderPreference.selected(in: chosen), .ai)

    var candidatesOn = chosen
    candidatesOn["translation_ai"] = nil
    candidatesOn["ai_assistant"] = ai.merging(["enabled": true]) { _, new in new }
    XCTAssertEqual(TranslationProviderPreference.route(in: candidatesOn), .none, "AI candidates alone no longer choose AI translation")
    XCTAssertEqual(TranslationProviderPreference.selected(in: candidatesOn), .off)

    var tencentDefault = chosen
    tencentDefault["tencent_tmt"] = nil
    XCTAssertEqual(TranslationProviderPreference.route(in: tencentDefault), .none)
    XCTAssertEqual(TranslationProviderPreference.selected(in: tencentDefault), .off)

    var account = chosen
    account["translation_account"] = true
    XCTAssertEqual(TranslationProviderPreference.route(in: account), .account)
    XCTAssertEqual(TranslationProviderPreference.selected(in: account), .account)

    var niutrans = chosen
    niutrans["niutrans"] = ["enabled": true, "app_id": "", "apikey": "key"]
    XCTAssertEqual(TranslationProviderPreference.route(in: niutrans), .none, "a chosen but incomplete service does not fall back to AI")

    var unconfigured = chosen
    unconfigured["ai_assistant"] = ["provider": "openAI", "endpoint": "", "model": "synthetic-model"]
    XCTAssertEqual(TranslationProviderPreference.route(in: unconfigured), .none)
    XCTAssertEqual(TranslationProviderPreference.selected(in: unconfigured), .ai, "the page keeps the choice and asks for the configuration")
  }

  func testSelectingAIWritesTheExplicitChoice() {
    var document: [String: Any] = [
      "translation_account": true,
      "ai_assistant": ["provider": "openAI", "endpoint": "https://api.openai.com/v1/chat/completions", "model": "synthetic-model"],
    ]
    TranslationProviderPreference.select(.ai, niutrans: ("", ""), tencent: ("", "", ""), custom: ("", ""), in: &document)
    XCTAssertEqual(document["translation_ai"] as? Bool, true)
    XCTAssertNil(document["translation_account"])
    XCTAssertEqual((document["tencent_tmt"] as? [String: Any])?["enabled"] as? Bool, false)
    XCTAssertEqual(TranslationProviderPreference.selected(in: document), .ai)
    if case .ai = TranslationProviderPreference.route(in: document) {} else { XCTFail("choosing AI routes to AI") }
    TranslationProviderPreference.select(.off, niutrans: ("", ""), tencent: ("", "", ""), custom: ("", ""), in: &document)
    XCTAssertNil(document["translation_ai"])
    XCTAssertEqual(TranslationProviderPreference.route(in: document), .none)
    XCTAssertEqual(TranslationProvider.allCases.last, .ai, "the page lists AI 辅助")
  }

  func testURLRequestRejectsMalformedNumericDescriptorValues() {
    let descriptor: [String: Any] = [
      "url": "https://example.invalid/translate", "method": "POST", "body": ["text": "x"],
      "timeout_ms": 2_500, "max_response_bytes": 1_024,
    ]
    var fractionalTimeout = descriptor
    fractionalTimeout["timeout_ms"] = 2_500.5
    XCTAssertNil(TranslationProviderClient.urlRequest(fractionalTimeout))
    var booleanLimit = descriptor
    booleanLimit["max_response_bytes"] = true
    XCTAssertNil(TranslationProviderClient.urlRequest(booleanLimit))
    var negativeTimeout = descriptor
    negativeTimeout["timeout_ms"] = -1
    XCTAssertNil(TranslationProviderClient.urlRequest(negativeTimeout))
    var zeroLimit = descriptor
    zeroLimit["max_response_bytes"] = 0
    XCTAssertNil(TranslationProviderClient.urlRequest(zeroLimit))
  }

  func testURLRequestRejectsCredentialsFragmentsAndMissingHosts() {
    let descriptor: [String: Any] = [
      "url": "https://example.invalid/translate", "method": "POST", "body": ["text": "x"],
    ]
    for url in [
      "https://user:password@example.invalid/translate",
      "https://example.invalid/translate#fragment",
      "https:///translate",
    ] {
      var malformed = descriptor
      malformed["url"] = url
      XCTAssertNil(TranslationProviderClient.urlRequest(malformed), "must reject \(url)")
    }
  }

  func testCacheScopeChangesWithProviderAndCredentials() {
    let a = TranslationRoute.niutrans(appID: "app", apiKey: "one").cacheScope
    XCTAssertNotEqual(a, TranslationRoute.niutrans(appID: "app", apiKey: "two").cacheScope)
    XCTAssertNotEqual(a, TranslationRoute.account.cacheScope)
    XCTAssertFalse(a.contains("one"), "the scope never carries a secret in clear")
  }

  func testAccountCacheScopeChangesWhenTheLoginChanges() {
    let firstLogin = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
    let secondLogin = UUID(uuidString: "00000000-0000-0000-0000-000000000002")!

    let firstScope = TranslationRoute.accountCacheScope(sessionID: firstLogin)
    let secondScope = TranslationRoute.accountCacheScope(sessionID: secondLogin)

    XCTAssertNotEqual(firstScope, secondScope)
    XCTAssertEqual(firstScope, TranslationRoute.accountCacheScope(sessionID: firstLogin))
  }

  @MainActor
  func testAccountGlossesAreDroppedForANewLogin() async throws {
    struct Fixed: CandidateTranslationService {
      let gloss: String
      func translate(words: [String], target: String) async throws -> [String] {
        words.map { _ in gloss }
      }
    }
    let firstScope = TranslationRoute.accountCacheScope(
      sessionID: UUID(uuidString: "00000000-0000-0000-0000-000000000001")!)
    let secondScope = TranslationRoute.accountCacheScope(
      sessionID: UUID(uuidString: "00000000-0000-0000-0000-000000000002")!)
    let store = CandidateTranslationStore(service: Fixed(gloss: "old"), scope: firstScope)
    let arrived = expectation(description: "old gloss")
    store.onArrival = { arrived.fulfill() }
    store.refresh(words: ["你好"], codes: ["EN"])
    await fulfillment(of: [arrived], timeout: 3)

    store.use(Fixed(gloss: "new"), scope: secondScope)
    XCTAssertNil(store.gloss(word: "你好", code: "EN"))
  }

  func testAccountTranslationUsesSignedInAccountBeforeAnonymousSession() async throws {
    CandidateAccountTranslationProtocol.reset()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [CandidateAccountTranslationProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let accountTokens = BackendAccountClient.Tokens(
      access_token: CandidateAccountTranslationProtocol.accountToken,
      refresh_token: String(repeating: "b", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "signed-in-user", display_name: "示例", created_at: "2026-09-08"))
    let account = BackendAccountSession(
      api: client, storage: CandidateSessionStorage(try BackendSavedSession.forTokens(accountTokens)),
      refreshLock: BackendProcessRefreshLock())
    let anonymous = BackendAccountSession(
      api: client, storage: CandidateSessionStorage(nil), refreshLock: BackendProcessRefreshLock())

    let service = BackendCandidateTranslationService(client: client, account: account, anonymous: anonymous)
    let result = try await service.translate(words: ["你好"], target: "en")
    XCTAssertEqual(result, ["hello"])
    XCTAssertEqual(CandidateAccountTranslationProtocol.recordedAuthorizations,
                   ["Bearer \(CandidateAccountTranslationProtocol.accountToken)"])
  }

  private func glossService(token: String) throws -> BackendCandidateTranslationService {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AccountGlossProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let tokens = BackendAccountClient.Tokens(access_token: token, refresh_token: String(repeating: "b", count: 64), token_type: "Bearer",
      expires_in: 900, user: .init(id: "signed-in-user", display_name: "示例", created_at: "2026-09-08"))
    let account = BackendAccountSession(api: client, storage: CandidateSessionStorage(try BackendSavedSession.forTokens(tokens)),
                                        refreshLock: BackendProcessRefreshLock())
    let anonymous = BackendAccountSession(api: client, storage: CandidateSessionStorage(nil), refreshLock: BackendProcessRefreshLock())
    return BackendCandidateTranslationService(client: client, account: account, anonymous: anonymous)
  }

  /// 请求体由 Rust 拼（`msime_client_account_gloss` 的 request）：源语言 ZH，目标语言转成大写，原文按顺序。
  func testAccountGlossRequestComesFromRust() async throws {
    AccountGlossProtocol.reset(reply: #"{"code":200,"data":["hello","world"]}"#)
    let result = try await glossService(token: AccountGlossProtocol.newToken).translate(words: ["你好", "世界"], target: "en")
    XCTAssertEqual(result, ["hello", "world"])
    let body = try XCTUnwrap(AccountGlossProtocol.translations.first?.body)
    XCTAssertEqual(body["source_lang"] as? String, "ZH")
    XCTAssertEqual(body["target_lang"] as? String, "EN")
    XCTAssertEqual(body["texts"] as? [String], ["你好", "世界"])
  }

  /// 一条坏值（含控制字符、与原文相同）只让那一条没有释义，其余照常显示；原来的 Swift 整页作废。
  func testOneBadAccountGlossLeavesTheRestOfThePage() async throws {
    AccountGlossProtocol.reset(reply: #"{"code":200,"data":["hello","bad\u0007value","世界"]}"#)
    let result = try await glossService(token: AccountGlossProtocol.newToken).translate(words: ["你好", "坏", "世界"], target: "en")
    XCTAssertEqual(result, ["hello", "", ""])
  }

  /// 服务端拒了令牌：会话刷新一次，用新令牌在同一个账号里重试；不合格的回答（条数不对）整页失败。
  func testRejectedAccountGlossTokenIsRefreshedOnce() async throws {
    AccountGlossProtocol.reset(reply: #"{"code":200,"data":["hello"]}"#)
    let result = try await glossService(token: AccountGlossProtocol.oldToken).translate(words: ["你好"], target: "en")
    XCTAssertEqual(result, ["hello"])
    XCTAssertEqual(AccountGlossProtocol.translations.map(\.authorization),
                   ["Bearer \(AccountGlossProtocol.oldToken)", "Bearer \(AccountGlossProtocol.newToken)"])
    AccountGlossProtocol.reset(reply: #"{"code":200,"data":["one","two"]}"#)
    do { _ = try await glossService(token: AccountGlossProtocol.newToken).translate(words: ["你好"], target: "en"); XCTFail("count mismatch") }
    catch let failure as AccountGloss.Failure { XCTAssertEqual(failure.code, "account_unavailable") }
  }

  func testURLRequestDefaultsAbsentTransportLimits() throws {
    let descriptor: [String: Any] = [
      "url": "https://example.com/translate",
      "body": ["text": "x"],
    ]
    let request = try XCTUnwrap(TranslationProviderClient.urlRequest(descriptor))
    XCTAssertEqual(request.connectTimeout, 2.5)
    XCTAssertEqual(request.timeout, 2.5)
    XCTAssertEqual(request.maxBytes, 1_048_576)
  }

  func testTencentSignsOneBatchAndSendsTheSignedBytes() async throws {
    let transport = RecordingTransport { _ in
      Data(#"{"Response":{"TargetTextList":["hello","world"],"RequestId":"r"}}"#.utf8)
    }
    let client = TranslationProviderClient(transport: transport, now: fixedDate)
    let route = TranslationRoute.tencent(secretID: "AKIDexample", secretKey: "secret", region: "ap-guangzhou")
    let glosses = await client.translate(words: ["你好", "世界"], target: "EN", route: route)
    XCTAssertEqual(glosses, ["hello", "world"])
    let request = try XCTUnwrap(transport.requests.first)
    XCTAssertEqual(transport.requests.count, 1)
    XCTAssertEqual(request.url?.absoluteString, "https://tmt.tencentcloudapi.com")
    XCTAssertTrue(request.value(forHTTPHeaderField: "Authorization")?.hasPrefix("TC3-HMAC-SHA256 Credential=AKIDexample/") == true)
    let body = try XCTUnwrap(request.httpBody.flatMap { String(data: $0, encoding: .utf8) })
    XCTAssertTrue(body.contains("你好") && body.contains(#""Target":"en""#), body)
  }

  func testTencentSplitsBatchesOfNine() async {
    let transport = RecordingTransport { request in
      let body = request.httpBody.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
      let texts = body?["SourceTextList"] as? [String] ?? []
      let reply: [String: Any] = ["Response": ["TargetTextList": texts.map { _ in "x" }]]
      return try? JSONSerialization.data(withJSONObject: reply)
    }
    let client = TranslationProviderClient(transport: transport, now: fixedDate)
    let words = (0..<12).map { "词\($0)" }
    let glosses = await client.translate(words: words, target: "JA",
                                         route: .tencent(secretID: "AKID1", secretKey: "k", region: "ap-guangzhou"))
    XCTAssertEqual(transport.requests.count, 2)
    XCTAssertEqual(glosses.compactMap { $0 }.count, 12)
  }

  func testNiuTransAndCustomTranslateWordByWord() async throws {
    let niutrans = RecordingTransport { _ in Data(#"{"tgtText":"hello"}"#.utf8) }
    let niutransGlosses = await TranslationProviderClient(transport: niutrans, now: fixedDate)
      .translate(words: ["你好", "世界"], target: "EN", route: .niutrans(appID: "app", apiKey: "key"))
    XCTAssertEqual(niutransGlosses, ["hello", "hello"])
    XCTAssertEqual(niutrans.requests.count, 2)
    let form = try XCTUnwrap(niutrans.requests.first?.httpBody.flatMap { String(data: $0, encoding: .utf8) })
    XCTAssertTrue(form.contains("timestamp=1700000000000") && form.contains("authStr="), form)

    let custom = RecordingTransport { _ in Data(#"{"code":200,"data":"hello"}"#.utf8) }
    let customGlosses = await TranslationProviderClient(transport: custom, now: fixedDate)
      .translate(words: ["你好"], target: "EN", route: .custom(endpoint: "https://example.com/translate", apiKey: "token"))
    XCTAssertEqual(customGlosses, ["hello"])
    let request = try XCTUnwrap(custom.requests.first)
    XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer token")
    let body = try XCTUnwrap(request.httpBody.flatMap { try JSONSerialization.jsonObject(with: $0) as? [String: Any] })
    XCTAssertEqual(body["target_lang"] as? String, "EN")
  }

  func testAITranslationUsesTheEndpointScopedCredentialForEveryProvider() async throws {
    let transport = RecordingTransport { _ in
      Data(#"{"choices":[{"message":{"content":"{\"translation\":\"hello\"}"}}]}"#.utf8)
    }
    let client = TranslationProviderClient(transport: transport, now: fixedDate)
    let route = TranslationRoute.ai(provider: "openai",
                                    endpoint: "https://api.openai.com/v1/chat/completions",
                                    model: "synthetic-model", token: "synthetic-token")
    let glosses = await client.translate(words: ["你好"], target: "EN", route: route)
    XCTAssertEqual(glosses, ["hello"])
    let request = try XCTUnwrap(transport.requests.first)
    XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer synthetic-token")
  }

  func testPlainHTTPAndFailuresYieldNoGloss() async {
    let transport = RecordingTransport { _ in Data(#"{"Response":{"Error":{"Code":"AuthFailure"}}}"#.utf8) }
    let client = TranslationProviderClient(transport: transport, now: fixedDate)
    let insecure = await client.translate(words: ["你好"], target: "EN", route: .custom(endpoint: "http://example.com/t", apiKey: ""))
    XCTAssertEqual(insecure, [nil])
    XCTAssertTrue(transport.requests.isEmpty, "iOS never sends a plain HTTP request")
    let failed = await client.translate(words: ["你好"], target: "EN",
                                        route: .tencent(secretID: "AKID1", secretKey: "k", region: "ap-guangzhou"))
    XCTAssertEqual(failed, [nil])
  }

  @MainActor
  func testStoreDropsGlossesWhenTheProviderChanges() async throws {
    struct Fixed: CandidateTranslationService {
      let gloss: String
      func translate(words: [String], target: String) async throws -> [String] { words.map { _ in gloss } }
    }
    let store = CandidateTranslationStore(service: Fixed(gloss: "hello"), scope: "a")
    let arrived = expectation(description: "gloss")
    store.onArrival = { arrived.fulfill() }
    store.refresh(words: ["你好"], codes: ["EN"])
    await fulfillment(of: [arrived], timeout: 3)
    XCTAssertEqual(store.gloss(word: "你好", code: "EN"), "hello")
    store.use(Fixed(gloss: "hi"), scope: "a")
    XCTAssertEqual(store.gloss(word: "你好", code: "EN"), "hello", "same scope keeps the cache")
    store.use(Fixed(gloss: "hi"), scope: "b")
    XCTAssertNil(store.gloss(word: "你好", code: "EN"))
  }
}

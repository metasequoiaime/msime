import XCTest

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

  func testCacheScopeChangesWithProviderAndCredentials() {
    let a = TranslationRoute.niutrans(appID: "app", apiKey: "one").cacheScope
    XCTAssertNotEqual(a, TranslationRoute.niutrans(appID: "app", apiKey: "two").cacheScope)
    XCTAssertNotEqual(a, TranslationRoute.account.cacheScope)
    XCTAssertFalse(a.contains("one"), "the scope never carries a secret in clear")
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

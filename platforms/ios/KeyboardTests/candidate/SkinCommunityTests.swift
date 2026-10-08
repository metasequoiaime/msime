import XCTest
import UIKit

private final class CommunityMemoryCredentials: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
}

/// 需要登录的用例用这个会话。内存存储只属于本进程，所以配进程内的刷新锁；默认的 App Group 文件锁在未签名的测试宿主里拿不到共享容器，`login` 会被它直接拒绝成 `Failure(status: 0)`。
private func communitySession(_ client: BackendAccountClient, _ storage: CommunityMemoryCredentials) -> BackendAccountSession {
  BackendAccountSession(api: client, storage: storage, refreshLock: BackendProcessRefreshLock())
}

private final class RetryReportProtocol: URLProtocol, @unchecked Sendable {
  static let skinID = UUID(uuidString: "a1234567-1234-1234-1234-123456789abc")!
  private static let lock = NSLock()
  private static var attempts = 0
  static var reportAttempts: Int { lock.lock(); defer { lock.unlock() }; return attempts }
  static func reset() { lock.lock(); defer { lock.unlock() }; attempts = 0 }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    let tokenA = String(repeating: "a", count: 64)
    let tokenB = String(repeating: "b", count: 64)
    let refreshA = String(repeating: "f", count: 64)
    let refreshB = String(repeating: "e", count: 64)
    let body: String
    let status: Int
    if path == "/v1/auth/login" {
      body = "{\"access_token\":\"\(tokenA)\",\"refresh_token\":\"\(refreshA)\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"fixture-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08T00:00:00Z\"}}"
      status = 200
    } else if path == "/v1/auth/refresh" {
      body = "{\"access_token\":\"\(tokenB)\",\"refresh_token\":\"\(refreshB)\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"fixture-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08T00:00:00Z\"}}"
      status = 200
    } else if path == "/v1/community/reports" {
      Self.lock.lock(); Self.attempts += 1; let attempt = Self.attempts; Self.lock.unlock()
      if attempt == 1 && request.value(forHTTPHeaderField: "Authorization") == "Bearer \(tokenA)" {
        body = #"{"error":{"code":"invalid_credentials"}}"#
        status = 401
      } else {
        // 与服务端 `community_reports.go` 一致：新记录 201、重复举报 200，响应体都是 `{"reported":true}`；`reportContent` 会校验这个字段。
        body = #"{"reported":true}"#
        status = 201
      }
    } else {
      body = "{}"
      status = 200
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class CommunityFixtureProtocol: URLProtocol, @unchecked Sendable {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    let body: String
    let status: Int
    if path == "/v1/auth/challenges" {
      body = #"{"challenge_id":"fixture-id","nonce":"server-nonce","expires_in":300}"#; status = 200
    } else if path == "/v1/auth/login" || path == "/v1/auth/refresh" {
      let token = String(repeating: path.hasSuffix("refresh") ? "b" : "a", count: 64)
      body = "{\"access_token\":\"\(token)\",\"refresh_token\":\"\(String(repeating: "f", count: 64))\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"fixture-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08T00:00:00Z\"}}"
      status = 200
    } else if path == "/v1/auth/logout" {
      body = ""; status = 204
    } else if path == "/v1/users/me" && request.httpMethod == "PATCH" {
      body = ""; status = 204
    } else if path == "/v1/users/me" {
      body = #"{"user":{"id":"fixture-user","display_name":"新昵称","created_at":"2026-09-08T00:00:00Z"},"identities":[{"provider":"apple","subject":"not-displayed"}]}"#
      status = 200
    } else if request.value(forHTTPHeaderField: "Authorization") == "Bearer " + String(repeating: "a", count: 64) {
      body = #"{"error":{"code":"invalid_credentials"}}"#; status = 401
    } else if request.url?.query?.contains("offset=20") == true {
      body = #"{"error":{"code":"rate_limit_exceeded","message":"internal"}}"#; status = 429
    } else {
      body = #"{"skins":[{"id":"a1234567-1234-1234-1234-123456789abc","name":"测试","description":"示例","author":"作者","design":{"background":15266027,"keyBackground":16777215,"keyForeground":1516829,"accent":1596487,"actionBackground":1596487,"cornerRadius":8,"borderWidth":0,"shadow":0,"pattern":0,"monospaced":false},"downloads":2,"rating_count":1,"rating_average":4,"owned":false,"my_rating":0}],"has_more":false}"#
      status = 200
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

/// 记录下载请求的方法、路径和 Authorization 头；登录接口给出匿名会话的令牌，下载接口返回一份合成皮肤。
private final class AnonymousDownloadProtocol: URLProtocol, @unchecked Sendable {
  static let anonymousToken = String(repeating: "c", count: 64)
  private static let lock = NSLock()
  private static var recorded: [(method: String, path: String, authorization: String?)] = []
  static var requests: [(method: String, path: String, authorization: String?)] { lock.lock(); defer { lock.unlock() }; return recorded }
  static func reset() { lock.lock(); defer { lock.unlock() }; recorded = [] }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    Self.lock.lock()
    Self.recorded.append((request.httpMethod ?? "", path, request.value(forHTTPHeaderField: "Authorization")))
    Self.lock.unlock()
    let body: String
    let status: Int
    if path == "/v1/auth/login" {
      body = "{\"access_token\":\"\(Self.anonymousToken)\",\"refresh_token\":\"\(String(repeating: "d", count: 64))\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"anonymous-user\",\"display_name\":\"匿名\",\"created_at\":\"2026-09-08T00:00:00Z\"}}"
      status = 200
    } else if path.hasPrefix("/v1/community/skins/") && path.hasSuffix("/download") {
      body = #"{"design":{"background":15266027,"keyBackground":16777215,"keyForeground":1516829,"accent":1596487,"actionBackground":1596487,"cornerRadius":8,"borderWidth":0,"shadow":0,"pattern":0,"monospaced":false}}"#
      status = 200
    } else {
      body = #"{"error":{"code":"not_found"}}"#
      status = 404
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class SkinCommunityTests: XCTestCase {
  func testCommunityWireFormatAndErrors() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [CommunityFixtureProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client, account: BackendAccountSession(api: client, storage: CommunityMemoryCredentials()))
    let page = try await api.list(search: "纸感 & 星光")
    XCTAssertEqual(page.skins.first?.downloads, 2)
    XCTAssertEqual(page.skins.first?.rating_average, 4)
    XCTAssertNil(page.skins.first?.design.photo)
    XCTAssertFalse(page.has_more)
    let challenge = try await api.challenge()
    XCTAssertEqual(challenge.nonce, "server-nonce")
    do { _ = try await api.list(offset: 20); XCTFail("expected limit error") }
    catch { XCTAssertEqual(error.localizedDescription, "操作较频繁，请稍后重试。") }
  }
  func testLoginConcurrentRefreshAndEmptyLogoutResponse() async throws {
    let memory = CommunityMemoryCredentials()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [CommunityFixtureProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client, account: communitySession(client, memory))
    try await api.login(challenge: "fixture", identityToken: "synthetic")
    let signedIn = try await api.signedIn()
    XCTAssertTrue(signedIn)
    let profile = try await api.currentUser()
    XCTAssertEqual(profile?.display_name, "测试")
    try await withThrowingTaskGroup(of: Void.self) { group in
      for _ in 0..<8 { group.addTask { _ = try await api.list() } }
      try await group.waitForAll()
    }
    XCTAssertEqual(try memory.load()?.tokens.access_token, String(repeating: "b", count: 64))
    try await api.logout()
    let signedOut = try await api.signedIn()
    XCTAssertFalse(signedOut)
    let profileAfterLogout = try await api.currentUser()
    XCTAssertNil(profileAfterLogout)
  }
  func testProfileFetchUpdateAndValidation() async throws {
    let memory = CommunityMemoryCredentials()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [CommunityFixtureProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client, account: communitySession(client, memory))
    try await api.login(challenge: "fixture", identityToken: "synthetic")
    for invalid in ["  ", String(repeating: "字", count: 65), "名字\n换行"] {
      do { _ = try await api.updateProfile(name: invalid); XCTFail("invalid name accepted") }
      catch { XCTAssertTrue(error.localizedDescription.contains("1–64")) }
    }
    XCTAssertEqual(try memory.load()?.tokens.user.display_name, "测试")
    let profile = try await api.updateProfile(name: " 新昵称 ")
    XCTAssertEqual(profile.user.display_name, "新昵称")
    XCTAssertEqual(profile.identities.first?.provider, "apple")
    XCTAssertEqual(profile.user.created_at, "2026-09-08T00:00:00Z")
    XCTAssertEqual(try memory.load()?.tokens.user.display_name, "新昵称")
    try await api.logout()
    do { _ = try await api.profile(); XCTFail("signed-out profile must require authentication") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 401) }
  }

  func testSignedInReportRefreshesRejectedToken() async throws {
    RetryReportProtocol.reset()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [RetryReportProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let memory = CommunityMemoryCredentials()
    let api = SkinCommunityAPI(client: client, account: communitySession(client, memory))
    try await api.login(challenge: "fixture", identityToken: "synthetic")

    try await api.report(kind: "skins", itemID: RetryReportProtocol.skinID.uuidString,
                         reason: "其他", detail: "合成说明")
    XCTAssertEqual(RetryReportProtocol.reportAttempts, 2)
    XCTAssertEqual(try memory.load()?.tokens.access_token,
                   String(repeating: "b", count: 64))
  }
  /// 没有登录账号时，下载与 Android 的「获取」一样用设备的匿名身份记一次下载，而不是要求先用 Apple 登录。
  func testSignedOutDownloadUsesAnonymousAccount() async throws {
    AnonymousDownloadProtocol.reset()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AnonymousDownloadProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let anonymous = communitySession(client, CommunityMemoryCredentials())
    try await anonymous.signIn(challenge: "fixture", credential: "synthetic")
    let api = SkinCommunityAPI(client: client, account: communitySession(client, CommunityMemoryCredentials()), anonymous: anonymous)
    let signedIn = try await api.signedIn()
    XCTAssertFalse(signedIn)

    let id = "b7654321-4321-4321-4321-cba987654321"
    let design = try await api.download(id)
    XCTAssertEqual(design.background, 15266027)
    let downloads = AnonymousDownloadProtocol.requests.filter { $0.path.hasSuffix("/download") }
    XCTAssertEqual(downloads.count, 1)
    XCTAssertEqual(downloads.first?.method, "POST")
    XCTAssertEqual(downloads.first?.path, "/v1/community/skins/\(id)/download")
    XCTAssertEqual(downloads.first?.authorization, "Bearer \(AnonymousDownloadProtocol.anonymousToken)")
  }
  @MainActor func testCommunityPreviewDoesNotChangeActiveDesign() throws {
    let previous = CustomKeyboardSkinStore.current
    let backdrop = KeyboardSkinBackgroundView(frame: CGRect(x: 0, y: 0, width: 320, height: 240))
    var design = CustomKeyboardSkin.templates[2].1
    design.pattern = 2
    backdrop.skin = .designed(design)
    let before = UIGraphicsImageRenderer(bounds: backdrop.bounds).image { backdrop.layer.render(in: $0.cgContext) }.pngData()
    design.background = 0xEEFFFF
    design.gradientEnd = nil
    backdrop.skin = .designed(design)
    let after = UIGraphicsImageRenderer(bounds: backdrop.bounds).image { backdrop.layer.render(in: $0.cgContext) }.pngData()
    XCTAssertNotEqual(before, after)
    XCTAssertEqual(CustomKeyboardSkinStore.current, previous)
    XCTAssertEqual(CustomKeyboardSkin.rgb(try XCTUnwrap(backdrop.backgroundColor)), design.background)
  }
}

private final class LargeResourceProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let entries = (0..<128).map { ["kind":"quick", "code":"key\($0)", "word":String(repeating:"🌱", count:99), "weight":10] as [String:Any] }
    let items = (0..<20).map { index in
      ["id":String(format:"10000000-0000-0000-0000-%012d",index),"kind":"dictionary","name":"词包","description":"","author":"测试",
       "content":["entries":entries],"revision":1,"saves":0,"saved":false,"owned":false,"rating_count":0,"rating_average":0,"my_rating":0] as [String:Any]
    }
    let data = try! JSONSerialization.data(withJSONObject:["items":items,"has_more":false])
    let status = request.url!.absoluteString.contains("C%2B%2B") ? 200 : 400
    client?.urlProtocol(self, didReceive:HTTPURLResponse(url:request.url!,statusCode:status,httpVersion:nil,
      headerFields:["Content-Type":"application/json","Content-Length":String(data.count)])!, cacheStoragePolicy:.notAllowed)
    client?.urlProtocol(self,didLoad:data); client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class OversizedSkinPageProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let design: [String: Any] = [
      "background": 15266027, "keyBackground": 16777215, "keyForeground": 1516829,
      "accent": 1596487, "actionBackground": 1596487, "cornerRadius": 8,
      "borderWidth": 0, "shadow": 0, "pattern": 0, "monospaced": false
    ]
    let skins = (0..<21).map { index in
      ["id": String(format: "a1234567-1234-1234-1234-%012d", index), "name": "测试",
       "description": "", "author": "作者", "design": design, "downloads": 0,
       "rating_count": 0, "rating_average": 0, "owned": false, "my_rating": 0] as [String: Any]
    }
    let data = try! JSONSerialization.data(withJSONObject: ["skins": skins, "has_more": true])
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!,
      cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class OversizedResourcePageProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let items = (0..<21).map { index in
      ["id": String(format: "10000000-0000-4000-8000-%012d", index), "kind": "dictionary",
       "name": "测试", "description": "", "author": "作者",
       "content": ["entries": []], "revision": 1, "saves": 0, "saved": false,
       "owned": false, "rating_count": 0, "rating_average": 0, "my_rating": 0] as [String: Any]
    }
    let data = try! JSONSerialization.data(withJSONObject: ["items": items, "has_more": true])
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!,
      cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class MalformedCommunityPageProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let design: [String: Any] = [
      "background": 15266027, "keyBackground": 16777215, "keyForeground": 1516829,
      "accent": 1596487, "actionBackground": 1596487, "cornerRadius": 8,
      "borderWidth": 0, "shadow": 0, "pattern": 0, "monospaced": false
    ]
    let value: [String: Any]
    if request.url?.path == "/v1/community/skins" {
      value = ["skins": [
        ["id": "a1234567-1234-1234-1234-123456789abc", "name": "测试", "description": "",
         "author": "作者", "design": design, "downloads": 0, "rating_count": 0,
         "rating_average": 0, "owned": false, "my_rating": 0],
        ["id": "A1234567-1234-1234-1234-123456789ABC", "name": "重复", "description": "",
         "author": "作者", "design": design, "downloads": 0, "rating_count": 0,
         "rating_average": 0, "owned": false, "my_rating": 0]
      ], "has_more": true]
    } else {
      value = ["items": [
        ["id": "10000000-0000-4000-8000-000000000001", "kind": "dictionary", "name": "测试",
         "description": "", "author": "作者", "content": ["entries": []], "revision": 1,
         "saves": 0, "saved": false, "owned": false, "rating_count": 0,
         "rating_average": 0, "my_rating": 0],
        ["id": "10000000-0000-4000-8000-000000000001", "kind": "dictionary", "name": "重复",
         "description": "", "author": "作者", "content": ["entries": []], "revision": 1,
         "saves": 0, "saved": false, "owned": false, "rating_count": 0,
         "rating_average": 0, "my_rating": 0]
      ], "has_more": true]
    }
    let data = try! JSONSerialization.data(withJSONObject: value)
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!,
      cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class InvalidCommunityPageProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let design: [String: Any] = [
      "background": 15266027, "keyBackground": 16777215, "keyForeground": 1516829,
      "accent": 1596487, "actionBackground": 1596487, "cornerRadius": 8,
      "borderWidth": 0, "shadow": 0, "pattern": 0, "monospaced": false
    ]
    let invalidSkin: [String: Any] = [
      "id": "not-a-uuid", "name": " ", "description": "", "author": "",
      "design": design, "downloads": 0, "rating_count": 0, "rating_average": 1,
      "owned": false, "my_rating": 6
    ]
    let invalidResource: [String: Any] = [
      "id": "not-a-uuid", "kind": "dictionary", "name": " ", "description": "",
      "author": "", "content": ["entries": []], "revision": 0, "saves": 0,
      "saved": false, "owned": false, "rating_count": 0, "rating_average": 1,
      "my_rating": 6
    ]
    let body: [String: Any] = request.url?.path == "/v1/community/skins"
      ? ["skins": [invalidSkin], "has_more": false]
      : ["items": [invalidResource], "has_more": false]
    let data = try! JSONSerialization.data(withJSONObject: body)
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!,
      cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class InvalidCommunityDetailProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let design: [String: Any] = [
      "background": 15266027, "keyBackground": 16777215, "keyForeground": 1516829,
      "accent": 1596487, "actionBackground": 1596487, "cornerRadius": 8,
      "borderWidth": 0, "shadow": 0, "pattern": 0, "monospaced": false
    ]
    let skin: [String: Any] = [
      "id": "not-a-uuid", "name": " ", "description": "", "author": "",
      "design": design, "downloads": 0, "rating_count": 0, "rating_average": 1,
      "owned": false, "my_rating": 6
    ]
    let resource: [String: Any] = [
      "id": "not-a-uuid", "kind": "dictionary", "name": " ", "description": "",
      "author": "", "content": ["entries": []], "revision": 0, "saves": 0,
      "saved": false, "owned": false, "rating_count": 0, "rating_average": 1,
      "my_rating": 6
    ]
    let body: [String: Any] = request.url?.path.hasPrefix("/v1/community/skins/") == true
      ? skin : resource
    let data = try! JSONSerialization.data(withJSONObject: body)
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!,
      cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class InvalidCommunityMutationProtocol: URLProtocol, @unchecked Sendable {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    let body: String
    let status: Int
    if path == "/v1/auth/challenges" {
      body = #"{"challenge_id":"fixture-id","nonce":"server-nonce","expires_in":300}"#; status = 200
    } else if path == "/v1/auth/login" {
      let token = String(repeating: "a", count: 64)
      let refresh = String(repeating: "f", count: 64)
      body = "{\"access_token\":\"\(token)\",\"refresh_token\":\"\(refresh)\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"fixture-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08T00:00:00Z\"}}"; status = 200
    } else if path == "/v1/users/me" {
      body = #"{"user":{"id":"fixture-user","display_name":"测试","created_at":"2026-09-08T00:00:00Z"},"identities":[]}"#; status = 200
    } else if path == "/v1/community/resources" && request.httpMethod == "GET" {
      body = #"{"items":[{"id":"10000000-0000-4000-8000-000000000001","kind":"dictionary","name":"测试","description":"","author":"作者","content":{"entries":[{"kind":"unknown","code":"a","word":"啊","weight":1}]},"revision":1,"saves":0,"saved":false,"owned":false,"rating_count":0,"rating_average":0,"my_rating":0}],"has_more":false}"#; status = 200
    } else if path == "/v1/community/resources" && request.httpMethod == "POST" {
      body = #"{"id":"10000000-0000-4000-8000-000000000001","revision":0}"#; status = 200
    } else if path == "/v1/community/skins" && request.httpMethod == "POST" {
      body = #"{"id":"a1234567-1234-1234-1234-123456789abc"}"#; status = 200
    } else if path.hasSuffix("/download") {
      body = #"{"design":{"background":16777216,"keyBackground":16777215,"keyForeground":1516829,"accent":1596487,"actionBackground":1596487,"cornerRadius":8,"borderWidth":0,"shadow":0,"pattern":0,"monospaced":false}}"#; status = 200
    } else if path.hasSuffix("/save") {
      body = #"{"saved":false}"#; status = 200
    } else if path.hasSuffix("/rating") {
      body = #"{"stars":1}"#; status = 200
    } else if path == "/v1/community/resources/10000000-0000-4000-8000-000000000001" && request.httpMethod == "DELETE" {
      body = #"{"deleted":false}"#; status = 200
    } else if path == "/v1/community/skins/10000000-0000-4000-8000-000000000001" && request.httpMethod == "DELETE" {
      body = #"{"deleted":false}"#; status = 200
    } else {
      body = #"{}"#; status = 200
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

extension SkinCommunityTests {
  func testUnknownDictionaryWordKindAndCommunityMutationResponsesAreRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [InvalidCommunityMutationProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let memory = CommunityMemoryCredentials()
    let api = SkinCommunityAPI(client: client, account: communitySession(client, memory))
    do {
      _ = try await api.resources(.dictionary)
      XCTFail("expected unknown dictionary word kind rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }

    try await api.login(challenge: "fixture", identityToken: "synthetic")
    let id = "10000000-0000-4000-8000-000000000001"
    let content = CommunityResourceContent(entries: [CommunityWord(PersonalWord(kind: .pinyin, key: "a", value: "啊", weight: 1))], prompt: nil)
    do {
      try await api.publishResource(id: id, kind: .dictionary, name: "测试", description: "", content: content, revision: 1)
      XCTFail("expected invalid publish response rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      try await api.saveResource(id, saved: true)
      XCTFail("expected invalid save response rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      try await api.rateResource(id, stars: 4)
      XCTFail("expected invalid rating response rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      try await api.unpublishResource(id)
      XCTFail("expected invalid delete response rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      try await api.publish(id: id, name: "测试", description: "", design: CustomKeyboardSkin())
      XCTFail("expected mismatched skin publication ID rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      _ = try await api.download(id)
      XCTFail("expected invalid downloaded design rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      try await api.rate(id, stars: 4)
      XCTFail("expected mismatched skin rating rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      try await api.unpublish(id)
      XCTFail("expected unsuccessful skin deletion rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
  }

  func testMalformedCommunityDetailsAreRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [InvalidCommunityDetailProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client,
                               account: BackendAccountSession(api: client,
                                                              storage: CommunityMemoryCredentials()))
    do {
      _ = try await api.detail("10000000-0000-4000-8000-000000000001")
      XCTFail("expected malformed skin detail rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    } catch {
      XCTFail("unexpected error: \(error)")
    }
    do {
      _ = try await api.resource("10000000-0000-4000-8000-000000000001")
      XCTFail("expected malformed resource detail rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    } catch {
      XCTFail("unexpected error: \(error)")
    }
  }

  func testMalformedCommunityItemsAreRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [InvalidCommunityPageProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client,
                               account: BackendAccountSession(api: client,
                                                              storage: CommunityMemoryCredentials()))
    do {
      _ = try await api.list(search: "malformed")
      XCTFail("expected malformed skin page rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    } catch {
      XCTFail("unexpected error: \(error)")
    }
    do {
      _ = try await api.resources(.dictionary, search: "malformed")
      XCTFail("expected malformed resource page rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    } catch {
      XCTFail("unexpected error: \(error)")
    }
  }

  func testDuplicateCommunityPageIsRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedCommunityPageProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client,
                               account: BackendAccountSession(api: client,
                                                              storage: CommunityMemoryCredentials()))
    do {
      _ = try await api.list()
      XCTFail("expected duplicate skin page rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
    do {
      _ = try await api.resources(.dictionary)
      XCTFail("expected duplicate resource page rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    }
  }

  func testOversizedSkinPageIsRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [OversizedSkinPageProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client,
                               account: BackendAccountSession(api: client,
                                                              storage: CommunityMemoryCredentials()))
    do {
      _ = try await api.list(search: "oversized")
      XCTFail("expected oversized page rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    } catch {
      XCTFail("unexpected error: \(error)")
    }
  }

  func testOversizedResourcePageIsRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [OversizedResourcePageProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client,
                               account: BackendAccountSession(api: client,
                                                              storage: CommunityMemoryCredentials()))
    do {
      _ = try await api.resources(.dictionary, search: "oversized")
      XCTFail("expected oversized resource page rejection")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    } catch {
      XCTFail("unexpected error: \(error)")
    }
  }

  func testLargeResourcePageKeepsPlusSearchAndOrdinaryLimit() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [LargeResourceProtocol.self]
    let client = BackendAccountClient(configuration:configuration)
    let api = SkinCommunityAPI(client:client,account:BackendAccountSession(api:client,storage:CommunityMemoryCredentials()))
    let page = try await api.resources(.dictionary,search:"C++")
    XCTAssertEqual(page.items.count,20)
    XCTAssertEqual(page.items.first?.content.entries?.count,128)
    do {
      _ = try await client.request("GET","/v1/community/resources?kind=dictionary&q=C%2B%2B")
      XCTFail("Ordinary limit was relaxed")
    } catch is BackendAccountClient.Failure { }
  }
}

/// A keychain the process cannot read. `BackendKeychain` hits this on an unsigned simulator build,
/// where SecItemCopyMatching answers -34018 (errSecMissingEntitlement), and on a locked device.
private struct UnreadableCredentials: BackendSessionStorage {
  func load() throws -> BackendSavedSession? { throw BackendAccountClient.Failure(status: 0) }
  func save(_ session: BackendSavedSession) throws {}
  func clear() throws {}
}

final class SkinCommunityFailureConversionTests: XCTestCase {
  /// Reading the session used to sit outside the block that converts backend failures into
  /// something this screen can say, so an unreadable keychain surfaced the account layer's own
  /// status-0 fallback — 请求未完成 — on a screen the user had opened to browse skins.
  func testUnreadableSessionSurfacesACommunityMessage() async {
    let api = SkinCommunityAPI(account: BackendAccountSession(storage: UnreadableCredentials()))
    do {
      _ = try await api.list()
      XCTFail("expected the unreadable session to fail the request")
    } catch let failure as CommunityFailure {
      XCTAssertEqual(failure.message, "社区暂时不可用，请稍后重试。")
    } catch let failure as BackendAccountClient.Failure {
      XCTFail("account failure reached the caller unconverted: status \(failure.status)")
    } catch {
      XCTFail("unexpected error: \(error)")
    }
  }
}

/// 记录每个请求的方法、URL 和请求体，并按路径返回带分类的合成条目。
private final class CategoryRecordingProtocol: URLProtocol, @unchecked Sendable {
  struct Recorded { let method: String; let url: URL; let body: Data }
  private static let lock = NSLock()
  private static var recorded: [Recorded] = []
  static func reset() { lock.lock(); defer { lock.unlock() }; recorded = [] }
  static var requests: [Recorded] { lock.lock(); defer { lock.unlock() }; return recorded }
  static let skinID = "a1234567-1234-1234-1234-123456789abc"

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var body = request.httpBody ?? Data()
    if body.isEmpty, let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var buffer = [UInt8](repeating: 0, count: 4096)
      while stream.hasBytesAvailable {
        let count = stream.read(&buffer, maxLength: buffer.count)
        guard count > 0 else { break }
        body.append(buffer, count: count)
      }
    }
    let path = request.url!.path
    Self.lock.lock()
    Self.recorded.append(Recorded(method: request.httpMethod ?? "GET", url: request.url!, body: body))
    Self.lock.unlock()
    let item = #"{"id":"a1234567-1234-1234-1234-123456789abc","name":"测试","description":"","author":"作者","design":{"background":15266027,"keyBackground":16777215,"keyForeground":1516829,"accent":1596487,"actionBackground":1596487,"cornerRadius":8,"borderWidth":0,"shadow":0,"pattern":0,"monospaced":false},"downloads":0,"rating_count":0,"rating_average":0,"owned":true,"my_rating":0,"category":"CATEGORY"}"#
    let response: String
    if path == "/v1/auth/login" {
      response = "{\"access_token\":\"\(String(repeating: "c", count: 64))\",\"refresh_token\":\"\(String(repeating: "f", count: 64))\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"fixture-user\",\"display_name\":\"测试\",\"created_at\":\"2026-09-08T00:00:00Z\"}}"
    } else if path == "/v1/community/skins" && request.httpMethod == "POST" {
      response = #"{"id":"a1234567-1234-1234-1234-123456789abc"}"#
    } else if path == "/v1/community/skins" {
      response = "{\"skins\":[\(item.replacingOccurrences(of: "CATEGORY", with: "acg"))],\"has_more\":false}"
    } else if request.httpMethod == "PATCH" {
      let requested = (try? JSONSerialization.jsonObject(with: body) as? [String: String])?["category"] ?? "other"
      response = item.replacingOccurrences(of: "CATEGORY", with: requested)
    } else {
      response = item.replacingOccurrences(of: "CATEGORY", with: "food")
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(response.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class SkinCommunityCategoryTests: XCTestCase {
  private func skinJSON(category: String?) -> Data {
    var value: [String: Any] = [
      "id": "a1234567-1234-1234-1234-123456789abc", "name": "测试", "description": "", "author": "作者",
      "design": ["background": 15266027, "keyBackground": 16777215, "keyForeground": 1516829,
                 "accent": 1596487, "actionBackground": 1596487, "cornerRadius": 8,
                 "borderWidth": 0, "shadow": 0, "pattern": 0, "monospaced": false],
      "downloads": 0, "rating_count": 0, "rating_average": 0, "owned": false, "my_rating": 0
    ]
    if let category { value["category"] = category }
    return try! JSONSerialization.data(withJSONObject: value)
  }

  func testCategoryDecoding() throws {
    let decoder = JSONDecoder()
    XCTAssertEqual(try decoder.decode(CommunitySkin.self, from: skinJSON(category: "guofeng")).category, .guofeng)
    XCTAssertNil(try decoder.decode(CommunitySkin.self, from: skinJSON(category: nil)).category)
    // 服务端将来新增的分类读作 other，条目本身不能因此读取失败。
    XCTAssertEqual(try decoder.decode(CommunitySkin.self, from: skinJSON(category: "future-category")).category, .other)
    XCTAssertEqual(CommunitySkinCategory.allCases.map(\.rawValue),
                   ["nature", "guofeng", "acg", "cute", "food", "tech", "minimal", "other"])
    XCTAssertEqual(CommunitySkinCategory.allCases.map(\.label),
                   ["自然", "国风", "二次元", "可爱", "美食", "科技夜色", "简约", "其他"])
  }

  func testFailedCategoryLoadRestoresDisplayedSelection() {
    XCTAssertEqual(CommunitySkinCategorySelectionPolicy.afterFailedLoad(previous: .nature), .nature)
    XCTAssertNil(CommunitySkinCategorySelectionPolicy.afterFailedLoad(previous: nil))
  }

  func testCategoryRequestsCarryIncludeAndFilter() async throws {
    CategoryRecordingProtocol.reset()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [CategoryRecordingProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let api = SkinCommunityAPI(client: client, account: communitySession(client, CommunityMemoryCredentials()))
    let id = CategoryRecordingProtocol.skinID

    let all = try await api.list(search: "纸感")
    XCTAssertEqual(all.skins.first?.category, .acg)
    _ = try await api.list(offset: 20, search: "", category: .tech)
    let detail = try await api.detail(id)
    XCTAssertEqual(detail.category, .food)
    try await api.login(challenge: "fixture", identityToken: "synthetic")
    try await api.publish(id: id, name: "测试", description: "", design: CustomKeyboardSkin(), category: .cute)
    try await api.publish(id: id, name: "测试", description: "", design: CustomKeyboardSkin())
    let changed = try await api.setCategory(id, category: .minimal)
    XCTAssertEqual(changed.category, .minimal)

    let community = CategoryRecordingProtocol.requests.filter { $0.url.path.hasPrefix("/v1/community/skins") }
    XCTAssertEqual(community.map(\.method), ["GET", "GET", "GET", "POST", "POST", "PATCH"])
    func query(_ request: CategoryRecordingProtocol.Recorded) -> [String: String] {
      let items = URLComponents(url: request.url, resolvingAgainstBaseURL: false)?.queryItems ?? []
      return Dictionary(items.map { ($0.name, $0.value ?? "") }, uniquingKeysWith: { first, _ in first })
    }
    // 发布只返回 `{"id"}`，服务端忽略 include；其余返回皮肤条目的请求都要带上它。
    for (index, request) in community.enumerated() where request.method != "POST" {
      XCTAssertEqual(query(request)["include"], "category", "\(index) \(request.url.absoluteString)")
    }
    XCTAssertNil(query(community[3])["include"])
    XCTAssertNil(query(community[0])["category"])
    XCTAssertEqual(query(community[1])["category"], "tech")
    XCTAssertEqual(query(community[1])["offset"], "20")
    XCTAssertEqual(community[2].url.path, "/v1/community/skins/\(id)")

    func json(_ request: CategoryRecordingProtocol.Recorded) throws -> [String: Any] {
      try XCTUnwrap(JSONSerialization.jsonObject(with: request.body) as? [String: Any])
    }
    XCTAssertEqual(try json(community[3])["category"] as? String, "cute")
    XCTAssertEqual(try json(community[4])["category"] as? String, "other")
    XCTAssertEqual(community[5].url.path, "/v1/community/skins/\(id)")
    XCTAssertEqual(query(community[5]), ["include": "category"])
    XCTAssertEqual(try json(community[5]) as? [String: String], ["category": "minimal"])
  }
}

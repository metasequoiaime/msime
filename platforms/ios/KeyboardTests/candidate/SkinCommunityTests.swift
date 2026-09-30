import XCTest
import UIKit

private final class CommunityMemoryCredentials: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
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
    let api = SkinCommunityAPI(client: client, account: BackendAccountSession(api: client, storage: memory))
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
    let api = SkinCommunityAPI(client: client, account: BackendAccountSession(api: client, storage: memory))
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
        ["id": "a1234567-1234-1234-1234-123456789abc", "name": "重复", "description": "",
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

extension SkinCommunityTests {
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

import Foundation
import XCTest
@testable import MSIMEBackend

private final class AccountProtocol: URLProtocol {
  nonisolated(unsafe) static var clipboardQuery: String?
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    let authenticated = request.value(forHTTPHeaderField: "Authorization") == "Bearer session"
    var status = 200
    var body = "{}"
    switch (request.httpMethod!, path) {
    case ("GET", "/v1/auth/providers"):
      body = #"{"providers":{"apple":true,"email":false}}"#
    case ("POST", "/v1/auth/challenges"):
      body = #"{"challenge_id":"challenge","expires_in":300,"nonce":"server-nonce"}"#
    case ("PATCH", "/v1/users/me"), ("POST", "/v1/auth/logout"), ("DELETE", "/v1/users/me"):
      status = authenticated ? 204 : 401; body = ""
    case ("GET", "/v1/users/me/clipboard"):
      // 服务端不再搜索：记下查询串，好让测试确认搜索词没有发出去。
      Self.clipboardQuery = request.url!.query
      let object: [String: Any] = ["enabled": true, "items": [
        ["id": String(repeating: "a", count: 64), "text": "学习 & q=other + % #", "updated_at": "2026-09-08"],
        ["id": String(repeating: "b", count: 64), "text": "合成", "updated_at": "2026-09-08"],
      ]]
      body = String(data: try! JSONSerialization.data(withJSONObject: object), encoding: .utf8)!
      if !authenticated { status = 401 }
    case ("PUT", "/v1/users/me/clipboard/settings"), ("DELETE", "/v1/users/me/clipboard"):
      status = authenticated ? 204 : 401; body = ""
    case ("GET", "/v1/users/me/dictionaries/quick"):
      let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
      let search = components.queryItems?.first(where: { $0.name == "q" })?.value ?? ""
      let object: [String: Any] = ["entries": [["id": String(repeating: "a", count: 64), "kind": "quick", "code": "test", "word": search, "weight": 100000, "revision": 1]], "has_more": false, "offset": 0]
      body = String(data: try! JSONSerialization.data(withJSONObject: object), encoding: .utf8)!
      if !authenticated { status = 401 }
    case ("POST", "/v1/auth/login"):
      // A compromised/misconfigured endpoint must not persist malformed tokens.
      body = #"{"access_token":"invalid","refresh_token":"invalid","token_type":"Bearer","expires_in":900,"user":{"id":"synthetic","display_name":"","created_at":"2026-09-08"}}"#
    default:
      status = 401; body = "private credential and input must not be shown"
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    if !body.isEmpty { client?.urlProtocol(self, didLoad: Data(body.utf8)) }
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class AuthInputProtocol: URLProtocol {
  static var requests = 0
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    Self.requests += 1
    let response = HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil,
      headerFields: nil)!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class ClipboardRetryStorage: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?

  init(_ value: BackendSavedSession?) { self.value = value }
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
}

private final class ClipboardRetryProtocol: URLProtocol {
  static let oldToken = String(repeating: "a", count: 64)
  static let newToken = String(repeating: "b", count: 64)

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    let authorization = request.value(forHTTPHeaderField: "Authorization") ?? ""
    let status: Int
    let body: String
    switch path {
    case "/v1/users/me/clipboard":
      if authorization == "Bearer \(Self.newToken)" {
        status = 200
        body = #"{"enabled":true,"items":[]}"#
      } else {
        status = 401
        body = #"{"error":{"code":"invalid_credentials"}}"#
      }
    case "/v1/users/me/dictionaries/pinyin":
      if authorization == "Bearer \(Self.newToken)" {
        status = 200
        let id = String(repeating: "a", count: 64)
        body = "{\"entries\":[{\"id\":\"\(id)\",\"kind\":\"pinyin\",\"code\":\"ni\",\"word\":\"你\",\"weight\":1,\"revision\":1}],\"has_more\":false,\"offset\":0}"
      } else {
        status = 401
        body = #"{"error":{"code":"invalid_credentials"}}"#
      }
    case "/v1/users/me/dictionaries/quick":
      if authorization == "Bearer \(Self.newToken)" {
        status = 200
        let id = String(repeating: "b", count: 64)
        body = "{\"revision\":2,\"previous\":null,\"replacement\":{\"id\":\"\(id)\",\"kind\":\"quick\",\"code\":\"sample\",\"word\":\"合成\",\"weight\":1,\"revision\":2}}"
      } else {
        status = 401
        body = #"{"error":{"code":"invalid_credentials"}}"#
      }
    case let path where path.hasSuffix("/catalog"):
      if authorization == "Bearer \(Self.newToken)" {
        status = 200
        body = #"{"entries":[],"offset":0,"has_more":false,"revision":1,"normalized":""}"#
      } else {
        status = 401
        body = #"{"error":{"code":"invalid_credentials"}}"#
      }
    case let path where path.hasSuffix("/apply"):
      if authorization == "Bearer \(Self.newToken)" {
        status = 200
        body = #"{"revision":2,"imported":1,"resource_revision":1}"#
      } else {
        status = 401
        body = #"{"error":{"code":"invalid_credentials"}}"#
      }
    case "/v1/auth/refresh":
      status = 200
      let refresh = String(repeating: "c", count: 64)
      body = "{\"access_token\":\"\(Self.newToken)\",\"refresh_token\":\"\(refresh)\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"synthetic-user\",\"display_name\":\"示例\",\"created_at\":\"2026-09-08\"}}"
    default:
      status = 404
      body = #"{"error":{"code":"not_found"}}"#
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class OversizedAccountProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/auth/login" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let token = String(repeating: "a", count: 64)
    let refresh = String(repeating: "b", count: 64)
    let body = try! JSONSerialization.data(withJSONObject: [
      "access_token": token, "refresh_token": refresh, "token_type": "Bearer", "expires_in": 2_592_001,
      "user": ["id": "synthetic", "display_name": "", "created_at": "2026-09-08"]
    ])
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedAccountPayloadProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool {
    ["/v1/auth/challenges", "/v1/auth/login", "/v1/users/me"].contains(request.url?.path)
  }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    let object: [String: Any]
    switch path {
    case "/v1/auth/challenges":
      object = ["challenge_id": "bad\u{0001}challenge", "expires_in": 300, "nonce": "server"]
    case "/v1/auth/login":
      let token = String(repeating: "a", count: 64)
      object = ["access_token": token, "refresh_token": token, "token_type": "Bearer", "expires_in": 900,
        "user": ["id": "synthetic", "display_name": String(repeating: "x", count: 65), "created_at": "2026-09-08"]]
    default:
      object = ["user": ["id": "synthetic", "display_name": "ok", "created_at": "2026-09-08"],
        "identities": [["provider": "Bad Provider", "subject": "subject"]]]
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: object))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedClipboardProtocol: URLProtocol {
  /// 这一次要返回哪种不合法的条目。
  nonisolated(unsafe) static var mode = ""
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/users/me/clipboard" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let validID = String(repeating: "a", count: 64)
    let item: [String: Any]
    switch request.httpMethod == "POST" ? "bad-id" : Self.mode {
    case "bad-id": item = ["id": "../logout", "text": "fixture", "updated_at": "2026-09-08"]
    case "bad-text": item = ["id": validID, "text": "\u{0000}fixture", "updated_at": "2026-09-08"]
    case "bad-time": item = ["id": validID, "text": "fixture", "updated_at": String(repeating: "t", count: 129)]
    default: item = ["id": validID, "text": "fixture", "updated_at": "2026-09-08"]
    }
    let body = try! JSONSerialization.data(withJSONObject: request.httpMethod == "POST" ? item : ["enabled": true, "items": [item]])
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
final class BackendAccountClientTests: XCTestCase {
  func testDefaultNicknameIsStableAndPreservesChosenName() {
    let empty = BackendAccountClient.User(id: "a7c2ef123456", display_name: "", created_at: "")
    XCTAssertEqual(empty.preferredDisplayName, "水杉小鹿·A7C2EF")
    let whitespace = BackendAccountClient.User(id: empty.id, display_name: " \n", created_at: "")
    XCTAssertEqual(whitespace.preferredDisplayName, empty.preferredDisplayName)
    let renamed = BackendAccountClient.User(id: empty.id, display_name: "我的昵称", created_at: "")
    XCTAssertEqual(renamed.preferredDisplayName, "我的昵称")
  }

  /// Rust 的 `AccountUser` 会把 `email` 和 `avatar_url` 写进同一份存储的会话；Swift 改写时必须保留它们，旧文档里没有时也不能凭空补出来。
  func testUserKeepsOptionalEmailAndAvatarAcrossARewrite() throws {
    let full = Data(#"{"id":"synthetic-user","display_name":"","created_at":"2026-01-01T00:00:00Z","email":"person@example.test","avatar_url":"https://media.msime.app/avatars/synthetic.jpg"}"#.utf8)
    let user = try JSONDecoder().decode(BackendAccountClient.User.self, from: full)
    XCTAssertEqual(user.email, "person@example.test")
    XCTAssertEqual(user.avatar_url, "https://media.msime.app/avatars/synthetic.jpg")
    let rewritten = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(user)) as? [String: Any])
    XCTAssertEqual(rewritten["email"] as? String, "person@example.test")
    XCTAssertEqual(rewritten["avatar_url"] as? String, "https://media.msime.app/avatars/synthetic.jpg")

    let old = Data(#"{"id":"synthetic-user","display_name":"","created_at":"2026-01-01T00:00:00Z"}"#.utf8)
    let older = try JSONDecoder().decode(BackendAccountClient.User.self, from: old)
    XCTAssertNil(older.email)
    XCTAssertNil(older.avatar_url)
    let encoded = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(older)) as? [String: Any])
    XCTAssertNil(encoded["email"])
    XCTAssertNil(encoded["avatar_url"])
    XCTAssertEqual(Set(encoded.keys), ["id", "display_name", "created_at"])
  }

  private func client() -> BackendAccountClient {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [AccountProtocol.self]
    return BackendAccountClient(configuration: config)
  }
  func testServerNonce() async throws {
    let client = client()
    let challenge = try await client.challenge(provider: "apple")
    XCTAssertEqual(challenge.nonce, "server-nonce")
    // iOS 的 Google 登录走 ID token 流程，不带回调地址。
    let google = try await client.challenge(provider: "google")
    XCTAssertEqual(google.nonce, "server-nonce")
  }
  func testAuthenticatedNoContentOperations() async throws {
    let client = client()
    try await client.rename("测试账号", token: "session")
    try await client.logout(token: "session")
    try await client.deleteAccount(token: "session")
  }
  func testRenameRejectsInvalidDisplayNamesBeforeSending() async throws {
    let client = client()
    for name in ["", " leading", "trailing ", String(repeating: "x", count: 65), "bad\u{0001}name"] {
      do {
        try await client.rename(name, token: "session")
        XCTFail("invalid display name sent: \(name.debugDescription)")
      } catch let failure as BackendAccountClient.Failure {
        XCTAssertEqual(failure.status, 400)
      }
    }
  }
  func testFailuresDoNotExposeServerText() async throws {
    do { _ = try await client().profile(token: "wrong"); XCTFail("must reject") }
    catch let error as BackendAccountClient.Failure {
      XCTAssertEqual(error.status, 401)
      XCTAssertFalse(error.localizedDescription.contains("private"))
    }
  }
  func testMalformedTokensAreRejected() async throws {
    do { _ = try await client().login(challenge: "challenge", credential: "synthetic"); XCTFail("must reject") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
  }
  func testTokenExpiryBeyondThirtyDaysIsRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [OversizedAccountProtocol.self]
    do {
      _ = try await BackendAccountClient(configuration: configuration).login(challenge: "challenge", credential: "synthetic")
      XCTFail("must reject")
    } catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
  }
  func testMalformedChallengeProfileAndTokenUsersAreRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedAccountPayloadProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    do {
      _ = try await client.challenge(provider: "apple")
      XCTFail("malformed challenge accepted")
    } catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
    do {
      _ = try await client.profile(token: "session")
      XCTFail("malformed profile accepted")
    } catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
    do {
      _ = try await client.login(challenge: "challenge", credential: "synthetic")
      XCTFail("malformed token user accepted")
    } catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
  }
  func testAuthInputsAreRejectedBeforeNetworking() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AuthInputProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    AuthInputProtocol.requests = 0

    for (provider, target) in [("unknown", ""), ("apple", "unexpected"),
                               ("email", String(repeating: "a", count: 321)),
                               ("email", "bad\u{0001}target"),
                               ("google", "http://127.0.0.1:80/callback")] {
      do {
        _ = try await client.challenge(provider: provider, target: target)
        XCTFail("invalid challenge target was sent")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 400)
      }
    }
    for (challenge, credential) in [("", "123456"), ("challenge", "bad\u{0001}credential"),
                                    ("challenge", String(repeating: "x", count: 16_385))] {
      do {
        _ = try await client.login(challenge: challenge, credential: credential)
        XCTFail("invalid login input was sent")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 400)
      }
    }
    for token in [String(repeating: "a", count: 63), String(repeating: "A", count: 64)] {
      do {
        _ = try await client.refresh(token)
        XCTFail("invalid refresh token was sent")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 400)
      }
    }
    for name in ["", " leading", String(repeating: "名", count: 65), "bad\nname"] {
      do {
        try await client.rename(name, token: "session")
        XCTFail("invalid display name was sent")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 400)
      }
    }
    XCTAssertEqual(AuthInputProtocol.requests, 0)
  }
  /// 搜索交给驱动在解密后进行，搜索词不出现在发往服务端的请求里。
  func testClipboardSearchIsLeftToTheDriver() async throws {
    let search = "学习 & q=other + % #"
    let driver = PassthroughClipboardDriver()
    AccountProtocol.clipboardQuery = "unset"
    let page = try await client().clipboard(token: "session", search: search, e2e: driver.e2e())
    XCTAssertTrue(page.enabled)
    XCTAssertEqual(page.items.map(\.text), [search])
    XCTAssertNil(AccountProtocol.clipboardQuery)
    XCTAssertEqual((driver.inputs.first?["start"] as? [String: Any])?["search"] as? String, search)
  }

  func testClipboardSessionRetriesRejectedAccessToken() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ClipboardRetryProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let tokens = BackendAccountClient.Tokens(
      access_token: ClipboardRetryProtocol.oldToken,
      refresh_token: String(repeating: "d", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-user", display_name: "示例", created_at: "2026-09-08"))
    let storage = ClipboardRetryStorage(try BackendSavedSession.forTokens(tokens))
    let session = BackendAccountSession(api: client, storage: storage,
                                        refreshLock: BackendProcessRefreshLock())

    // 键盘扩展的写法（`BackendKeyboardCloudClipboardService`）：令牌被拒时会话刷新一次、同一个账号里重试。
    let identity = try await session.credentials()
    let page = try await session.authenticated(matchingUserID: identity.userID, matchingSessionID: identity.sessionID) { token in
      try await client.clipboard(token: token, e2e: PassthroughClipboardDriver().e2e())
    }.value
    XCTAssertTrue(page.enabled)
  }

  func testClipboardSearchRejectsOversizedAndUnsafeValues() async throws {
    for search in [String(repeating: "a", count: 1025), "safe\u{0007}query"] {
      do {
        _ = try await client().clipboard(token: "session", search: search, e2e: PassthroughClipboardDriver().e2e())
        XCTFail("unsafe search")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 400)
      }
    }
  }
  func testClipboardRejectsOversizedUTF16AndUnsafeID() async throws {
    let e2e = PassthroughClipboardDriver().e2e()
    do { _ = try await client().addClipboard(String(repeating: "😀", count: 2001), token: "session", e2e: e2e); XCTFail("too long") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 400) }
    do { _ = try await client().addClipboard("safe\u{0007}text", token: "session", e2e: e2e); XCTFail("control character") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 400) }
  }
  func testClipboardRejectsMalformedServerItems() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedClipboardProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    for mode in ["bad-id", "bad-text", "bad-time"] {
      MalformedClipboardProtocol.mode = mode
      do {
        _ = try await client.clipboard(token: "session", e2e: PassthroughClipboardDriver().e2e())
        XCTFail("malformed clipboard item accepted: \(mode)")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 0)
      }
    }
    do {
      _ = try await client.addClipboard("fixture", token: "session", e2e: PassthroughClipboardDriver().e2e())
      XCTFail("malformed added clipboard item accepted")
    } catch let error as BackendAccountClient.Failure {
      XCTAssertEqual(error.status, 0)
    }
  }
  func testCredentialsCannotGoToAnotherOrigin() async throws {
    for path in ["https://other.invalid/v1/users/me", "//other.invalid/v1/users/me", "/v1/\\other.invalid", "/v1/users/me#fragment"] {
      do { _ = try await client().request("GET", path, token: "session"); XCTFail(path) }
      catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
    }
  }
  func testCredentialsCannotEscapeVersionedApiPathWithDotSegments() async throws {
    for path in ["/v1/../auth/logout", "/v1/users/../auth/logout", "/v1/%2e%2e/auth/logout"] {
      do { _ = try await client().request("GET", path, token: "session"); XCTFail(path) }
      catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
    }
  }
}

import Foundation
import XCTest
@testable import MSIMEBackend

private final class AccountProtocol: URLProtocol {
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
      let search = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "q" })?.value ?? ""
      let object: [String: Any] = ["enabled": true, "items": [["id": String(repeating: "a", count: 64), "text": search, "updated_at": "2026-09-08"]]]
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
private final class OversizedProvidersProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/auth/providers" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var providers: [String: Bool] = [:]
    for suffix in Array("abcdefghijklmnopq") { providers["provider-\(suffix)"] = true }
    let body = try! JSONSerialization.data(withJSONObject: ["providers": providers])
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class InvalidProviderKeyProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/auth/providers" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let body = Data(#"{"providers":{"Bad Provider":true}}"#.utf8)
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
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/users/me/clipboard" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let search = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "q" }?.value ?? ""
    let validID = String(repeating: "a", count: 64)
    let item: [String: Any]
    switch request.httpMethod == "POST" ? "bad-id" : search {
    case "bad-id": item = ["id": "../logout", "text": "fixture", "updated_at": "2026-09-08"]
    case "bad-text": item = ["id": validID, "text": "\u{0000}fixture", "updated_at": "2026-09-08"]
    case "bad-time": item = ["id": validID, "text": "fixture", "updated_at": String(repeating: "t", count: 129)]
    default: item = ["id": validID, "text": "fixture", "updated_at": "2026-09-08"]
    }
    let body = try! JSONSerialization.data(withJSONObject: ["enabled": true, "items": [item]])
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedDictionaryProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/users/me/dictionaries/pinyin" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let query = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "q" }?.value ?? ""
    let entry: [String: Any]
    switch query {
    case "bad-id": entry = ["id": "../logout", "kind": "pinyin", "code": "ni", "word": "你", "weight": 1, "revision": 1]
    case "bad-code": entry = ["id": String(repeating: "a", count: 64), "kind": "pinyin", "code": "ni2", "word": "你", "weight": 1, "revision": 1]
    case "bad-kind": entry = ["id": String(repeating: "a", count: 64), "kind": "wubi", "code": "ni", "word": "你", "weight": 1, "revision": 1]
    case "bad-offset": entry = ["id": String(repeating: "a", count: 64), "kind": "pinyin", "code": "ni", "word": "你", "weight": 1, "revision": 1]
    case "bad-weight": entry = ["id": String(repeating: "a", count: 64), "kind": "pinyin", "code": "ni", "word": "你", "weight": -1, "revision": 1]
    default: entry = ["id": String(repeating: "a", count: 64), "kind": "pinyin", "code": "ni", "word": "你", "weight": 1, "revision": 1]
    }
    let object: [String: Any] = ["entries": [entry], "has_more": false, "offset": query == "bad-offset" ? 100 : 0]
    let body = try! JSONSerialization.data(withJSONObject: object)
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedImportProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path.hasSuffix("/import") == true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type":"application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"imported":1000001,"revision":-1}"#.utf8))
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

  private func client() -> BackendAccountClient {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [AccountProtocol.self]
    return BackendAccountClient(configuration: config)
  }
  func testProvidersAndServerNonce() async throws {
    let client = client()
    let providers = try await client.providers()
    XCTAssertEqual(providers["apple"], true)
    XCTAssertEqual(providers["email"], false)
    let challenge = try await client.challenge(provider: "apple")
    XCTAssertEqual(challenge.nonce, "server-nonce")
  }
  func testProvidersRejectsUnboundedOrMalformedMap() async throws {
    for protocolClass in [OversizedProvidersProtocol.self, InvalidProviderKeyProtocol.self] {
      let configuration = URLSessionConfiguration.ephemeral
      configuration.protocolClasses = [protocolClass]
      do {
        _ = try await BackendAccountClient(configuration: configuration).providers()
        XCTFail("malformed providers accepted")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 0)
      }
    }
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
  func testClipboardSearchIsEncodedAsOneQueryValue() async throws {
    let search = "学习 & q=other + % #"
    let page = try await client().clipboard(token: "session", search: search)
    XCTAssertTrue(page.enabled)
    XCTAssertEqual(page.items.first?.text, search)
    try await client().setClipboardEnabled(false, token: "session")
    try await client().deleteClipboard(token: "session")
  }
  func testClipboardSearchRejectsOversizedAndUnsafeValues() async throws {
    for search in [String(repeating: "a", count: 1025), "safe\u{0007}query"] {
      do {
        _ = try await client().clipboard(token: "session", search: search)
        XCTFail("unsafe search")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 400)
      }
    }
  }
  func testClipboardRejectsOversizedUTF16AndUnsafeID() async throws {
    do { _ = try await client().addClipboard(String(repeating: "😀", count: 2001), token: "session"); XCTFail("too long") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 400) }
    do { _ = try await client().addClipboard("safe\u{0007}text", token: "session"); XCTFail("control character") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 400) }
    do { try await client().deleteClipboard(id: "../auth/logout", token: "session"); XCTFail("unsafe id") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 400) }
  }
  func testClipboardRejectsMalformedServerItems() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedClipboardProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    for search in ["bad-id", "bad-text", "bad-time"] {
      do {
        _ = try await client.clipboard(token: "session", search: search)
        XCTFail("malformed clipboard item accepted: \(search)")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 0)
      }
    }
    do {
      _ = try await client.addClipboard("fixture", token: "session")
      XCTFail("malformed added clipboard item accepted")
    } catch let error as BackendAccountClient.Failure {
      XCTAssertEqual(error.status, 0)
    }
  }
  func testDictionaryImportRejectsMalformedServerResult() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedImportProtocol.self]
    do {
      _ = try await BackendAccountClient(configuration: configuration).importDictionary(
        .pinyin, text: "ni\t你", format: .standard, token: "session")
      XCTFail("malformed dictionary import result accepted")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }
  }
  func testDictionaryRejectsMalformedServerEntries() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedDictionaryProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    for search in ["bad-id", "bad-code", "bad-kind", "bad-offset", "bad-weight"] {
      do {
        _ = try await client.dictionary(.pinyin, search: search, token: "session")
        XCTFail("malformed dictionary entry accepted: \(search)")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 0)
      }
    }
  }
  func testDictionarySearchCannotInjectAnotherQueryParameter() async throws {
    let text = "合成 & q=other + % #"
    let page = try await client().dictionary(.quick, search: text, token: "session")
    XCTAssertEqual(page.entries.first?.word, text)
    XCTAssertFalse(page.has_more)
  }
  func testDictionarySearchRejectsControlCharactersLocally() async throws {
    do {
      _ = try await client().dictionary(.quick, search: "safe\u{0007}query", token: "session")
      XCTFail("control character accepted in dictionary search")
    } catch let error as BackendAccountClient.Failure {
      XCTAssertEqual(error.status, 400)
    }
  }
  func testDictionaryMutationRejectsUnsafeEntryIDAndInvalidRevision() async throws {
    for (id, revision) in [("../../auth/logout", 1), (String(repeating: "a", count: 64), 0)] {
      let entry = BackendAccountClient.DictionaryEntry(id: id, kind: .quick, code: "test", word: "合成", weight: 1, revision: Int64(revision))
      do { _ = try await client().deleteDictionary(entry, token: "session"); XCTFail("unsafe mutation") }
      catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 400) }
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

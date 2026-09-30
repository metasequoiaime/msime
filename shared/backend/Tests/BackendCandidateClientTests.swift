import Foundation
import XCTest
@testable import MSIMEBackend

private final class CandidateProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var body = request.httpBody ?? Data()
    if let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var bytes = [UInt8](repeating: 0, count: 4096)
      while true { let n = stream.read(&bytes, maxLength: bytes.count); if n <= 0 { break }; body.append(contentsOf: bytes.prefix(n)) }
    }
    let object = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
    var status = 200
    let data: Data
    if request.url?.path.hasSuffix("ranking") == true {
      let action = object?["action"] as? [String: Any]
      if action?["code"] as? String != "ni'hao" || object?["revision"] as? Int != 42 { status = 400 }
      data = Data(#"{"revision":43,"changed":true,"selection":{"count":0}}"#.utf8)
    } else {
      if object?.keys.contains("position") != false || object?["context"] as? String != "server:context" || request.httpMethod != "DELETE" { status = 400 }
      data = Data(#"{"revision":43}"#.utf8)
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type":"application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedCandidatesProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/users/me/dictionary/candidates" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var body = request.httpBody ?? Data()
    if let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var bytes = [UInt8](repeating: 0, count: 4096)
      while true { let n = stream.read(&bytes, maxLength: bytes.count); if n <= 0 { break }; body.append(contentsOf: bytes.prefix(n)) }
    }
    let query = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
    let text = query?["text"] as? String ?? ""
    let candidate: [String: Any]
    switch text {
    case "bad-code": candidate = ["code": "", "word": "你", "weight": 1, "canonical_pinyin": NSNull()]
    case "bad-weight": candidate = ["code": "ni", "word": "你", "weight": -1, "canonical_pinyin": NSNull()]
    default: candidate = ["code": "ni", "word": "你", "weight": 1, "canonical_pinyin": NSNull()]
    }
    let candidates: [[String: Any]] = text == "too-many" ? Array(repeating: candidate, count: 2) : [candidate]
    let object: [String: Any] = [
      "candidates": candidates,
      "context": text == "bad-context" ? "bad\u{0001}context" : "pinyin",
      "revision": text == "bad-revision" ? -1 : 1,
    ]
    let data = try! JSONSerialization.data(withJSONObject: object)
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedFixedPositionsProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/users/me/dictionary/positions" }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
    let context = components?.queryItems?.first { $0.name == "context" }?.value ?? ""
    let position: Int = context == "bad-position" ? 0 : 1
    let entryContext = context == "bad-context" ? "other" : context
    let entry: [String: Any] = ["context": entryContext, "code": context == "bad-code" ? "bad\u{0001}" : "ni", "word": "你", "position": position]
    let positions: [[String: Any]] = context == "too-many" ? Array(repeating: entry, count: 101) : [entry]
    let object: [String: Any] = ["positions": positions, "offset": context == "bad-offset" ? 100 : 0, "has_more": false]
    let data = try! JSONSerialization.data(withJSONObject: object)
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedRankingProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path.hasSuffix("/ranking") == true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type":"application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"revision":-1,"changed":true,"selection":{"count":-1}}"#.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
final class BackendCandidateClientTests: XCTestCase {
  private func client() -> BackendAccountClient {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [CandidateProtocol.self]
    return BackendAccountClient(configuration: config)
  }
  func testRankingUsesCanonicalPinyinInsteadOfDisplayCode() async throws {
    let candidate = BackendAccountClient.PersonalCandidate(code: "nihc", word: "你好", weight: 10, canonical_pinyin: "ni'hao")
    let query = BackendAccountClient.CandidateQuery(text: "nihc", kind: "pinyin", scheme: "shuangpin", profile: "xiaohe", limit: 100)
    let result = try await client().rankCandidate(candidate, query: query, revision: 42, mode: .pin, token: "session")
    XCTAssertTrue(result.changed)
    XCTAssertEqual(result.revision, 43)
  }
  func testUnfixPreservesServerContextAndOmitsPositionField() async throws {
    let result = try await client().setFixedPosition(context: "server:context", code: "ni'hao", word: "你好", position: nil, revision: 42, token: "session")
    XCTAssertEqual(result.revision, 43)
  }
  func testPersonalCandidatesRejectMalformedServerResponses() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedCandidatesProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    for text in ["bad-code", "bad-weight", "too-many", "bad-context", "bad-revision"] {
      let query = BackendAccountClient.CandidateQuery(text: text, kind: "pinyin", scheme: "pinyin", profile: "xiaohe", limit: 1)
      do {
        _ = try await client.personalCandidates(query, token: "session")
        XCTFail("malformed candidate response accepted: \(text)")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 0)
      }
    }
  }
  func testFixedPositionsRejectMalformedServerResponses() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedFixedPositionsProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    for context in ["bad-position", "bad-context", "bad-code", "bad-offset", "too-many"] {
      do {
        _ = try await client.fixedPositions(context: context, token: "session")
        XCTFail("malformed fixed-position response accepted: \(context)")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 0)
      }
    }
  }
  func testRankingRejectsMalformedServerResponses() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedRankingProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let candidate = BackendAccountClient.PersonalCandidate(code: "ni", word: "你", weight: 1, canonical_pinyin: nil)
    let query = BackendAccountClient.CandidateQuery(text: "ni", kind: "pinyin", scheme: "pinyin", profile: "xiaohe", limit: 1)
    do {
      _ = try await client.rankCandidate(candidate, query: query, revision: 1, mode: .pin, token: "session")
      XCTFail("malformed ranking response accepted")
    } catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
  }
}

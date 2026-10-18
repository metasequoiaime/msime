import Foundation
import XCTest
@testable import MSIMEBackend

/// Answers every request from a fixed table and records what was sent; the status, headers and body come from `reply`.
private final class ModerationProtocol: URLProtocol {
  static var reply: (status: Int, body: [String: Any]) = (200, [:])
  static var sent: [(method: String, url: URL, body: [String: Any])] = []
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var data = request.httpBody ?? Data()
    if let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var buffer = [UInt8](repeating: 0, count: 4096)
      while true { let n = stream.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; data.append(contentsOf: buffer.prefix(n)) }
    }
    let body = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    Self.sent.append((request.httpMethod ?? "", request.url!, body))
    let encoded = try! JSONSerialization.data(withJSONObject: Self.reply.body)
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: Self.reply.status, httpVersion: nil,
      headerFields: ["Content-Type": "application/json", "Content-Length": String(encoded.count)])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: encoded)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendCommunityModerationTests: XCTestCase {
  private func client() -> BackendAccountClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ModerationProtocol.self]
    return BackendAccountClient(configuration: configuration)
  }

  override func setUp() {
    super.setUp()
    ModerationProtocol.sent = []
    ModerationProtocol.reply = (200, [:])
  }

  func testRefusalCodesBecomeTheirOwnMessages() async throws {
    let cases: [(Int, String, String)] = [
      (422, "blocked_content", "内容包含不允许发布的词语，请修改后再提交"),
      (503, "screening_unavailable", "审核服务暂时不可用，请稍后重试"),
      (403, "account_banned", "该账号已被封禁，暂时无法使用账号相关功能"),
    ]
    for (status, code, message) in cases {
      ModerationProtocol.reply = (status, ["error": ["code": code, "message": "server text that is never shown"]])
      do {
        _ = try await client().request("POST", "/v1/community/resources", token: "token", body: Data("{}".utf8))
        XCTFail("\(code) was accepted")
      } catch let failure as BackendAccountClient.Failure {
        XCTAssertEqual(failure.status, status)
        XCTAssertEqual(failure.code, code)
        XCTAssertEqual(failure.errorDescription, message)
      }
    }
  }

  func testOtherRefusalsKeepTheGenericMessages() {
    XCTAssertEqual(BackendAccountClient.Failure(status: 503).errorDescription, "此服务暂不可用，请稍后再试。")
    // A code only counts with the status the contract pairs it with.
    XCTAssertNil(BackendAccountClient.Failure(status: 400, code: "blocked_content").moderationMessage)
    XCTAssertEqual(BackendAccountClient.errorCode(Data(#"{"error":{"code":"blocked_content"}}"#.utf8)), "blocked_content")
    XCTAssertNil(BackendAccountClient.errorCode(Data(#"{"error":{"code":"Bad Code"}}"#.utf8)))
    XCTAssertNil(BackendAccountClient.errorCode(Data("not json".utf8)))
  }
}

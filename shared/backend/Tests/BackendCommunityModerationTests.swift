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
  private let id = UUID(uuidString: "10000000-0000-0000-0000-000000000002")!

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

  private func resource(owned: Bool, moderation: String?) -> [String: Any] {
    var value: [String: Any] = [
      "id": id.uuidString.lowercased(), "kind": "reply", "name": "合成模板", "description": "", "author": "测试作者",
      "content": ["prompt": "请礼貌回复"], "revision": 1, "saves": 0, "saved": false, "owned": owned,
      "rating_count": 0, "rating_average": 0, "my_rating": 0,
    ]
    if let moderation { value["moderation"] = moderation }
    return value
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

  func testOwnListAndDetailAskForTheModerationState() async throws {
    ModerationProtocol.reply = (200, ["items": [resource(owned: true, moderation: "removed")], "has_more": false])
    let page = try await client().communityResources(.reply, scope: .mine, token: "token")
    XCTAssertTrue(page.items[0].removed)
    XCTAssertTrue(ModerationProtocol.sent[0].url.query?.contains("fields=moderation") == true)

    ModerationProtocol.reply = (200, ["items": [resource(owned: false, moderation: nil)], "has_more": false])
    let everyone = try await client().communityResources(.reply)
    XCTAssertFalse(everyone.items[0].removed)
    XCTAssertFalse(ModerationProtocol.sent[1].url.query?.contains("fields") == true)

    ModerationProtocol.reply = (200, resource(owned: true, moderation: "pending"))
    let detail = try await client().communityResource(id, token: "token")
    // Pending is post-moderation: already public, so nothing is shown for it.
    XCTAssertEqual(detail.moderation, "pending")
    XCTAssertFalse(detail.removed)
    XCTAssertEqual(ModerationProtocol.sent[2].url.query, "fields=moderation")
  }

  func testReportSendsTheFixedReasonAndOptionalDetail() async throws {
    ModerationProtocol.reply = (201, ["reported": true])
    try await client().reportContent(kind: "dictionaries", itemID: id, reason: "垃圾广告", detail: "  ", token: "token")
    try await client().reportContent(kind: "replies", itemID: id, reason: "其他", detail: "合成说明", token: "token")
    XCTAssertEqual(ModerationProtocol.sent.count, 2)
    XCTAssertEqual(ModerationProtocol.sent[0].method, "POST")
    XCTAssertEqual(ModerationProtocol.sent[0].url.path, "/v1/community/reports")
    XCTAssertEqual(ModerationProtocol.sent[0].body["kind"] as? String, "dictionaries")
    XCTAssertEqual(ModerationProtocol.sent[0].body["item_id"] as? String, id.uuidString.lowercased())
    XCTAssertEqual(ModerationProtocol.sent[0].body["reason"] as? String, "垃圾广告")
    XCTAssertNil(ModerationProtocol.sent[0].body["detail"])
    XCTAssertEqual(ModerationProtocol.sent[1].body["detail"] as? String, "合成说明")
  }

  func testReportRejectsUnknownReasonsKindsAndLongDetail() async {
    for (kind, reason, detail) in [("skins", "不喜欢", ""), ("users", "其他", ""), ("skins", "其他", String(repeating: "字", count: 1001))] {
      do {
        try await client().reportContent(kind: kind, itemID: id, reason: reason, detail: detail, token: "token")
        XCTFail("\(kind) \(reason) was sent")
      } catch let failure as BackendAccountClient.Failure {
        XCTAssertEqual(failure.status, 400)
      } catch { XCTFail("\(error)") }
    }
    XCTAssertTrue(ModerationProtocol.sent.isEmpty)
  }
}

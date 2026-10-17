import XCTest
@testable import MSIMEBackend

/// 记下发到打字统计和 QQ 绑定接口的请求，按路径回固定的状态码和正文。
private final class TypingUploadProtocol: URLProtocol {
  struct Sent: Equatable { let method: String; let path: String; let body: Data; let authorization: String? }
  private static let lock = NSLock()
  private static var status = 200
  private static var reply = ""
  private static var sent: [Sent] = []

  static func respond(_ status: Int, _ reply: String) {
    lock.lock(); defer { lock.unlock() }
    self.status = status
    self.reply = reply
    sent = []
  }

  static func requests() -> [Sent] {
    lock.lock(); defer { lock.unlock() }
    return sent
  }

  override class func canInit(with request: URLRequest) -> Bool {
    guard let path = request.url?.path else { return false }
    return path == "/v1/users/me/typing-statistics" || path.hasPrefix("/v1/users/me/qq-links")
  }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var body = request.httpBody ?? Data()
    if let stream = request.httpBodyStream {
      stream.open()
      var buffer = [UInt8](repeating: 0, count: 4096)
      while stream.hasBytesAvailable {
        let count = stream.read(&buffer, maxLength: buffer.count)
        guard count > 0 else { break }
        body.append(buffer, count: count)
      }
      stream.close()
    }
    Self.lock.lock()
    Self.sent.append(Sent(method: request.httpMethod ?? "", path: request.url?.path ?? "", body: body,
                          authorization: request.value(forHTTPHeaderField: "Authorization")))
    let status = Self.status, reply = Self.reply
    Self.lock.unlock()
    let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(reply.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendTypingUploadTests: XCTestCase {
  private typealias Client = BackendAccountClient

  private func client() -> Client {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [TypingUploadProtocol.self]
    return Client(configuration: configuration)
  }

  func testCodesAndIdsHaveTheServerShape() {
    XCTAssertEqual(Client.normalizedQQLinkCode(" 1234-5678 "), "12345678")
    XCTAssertEqual(Client.normalizedQQLinkCode("1234 5678"), "12345678")
    for code in ["1234567", "123456789", "1234567a", "１２３４５６７８", ""] {
      XCTAssertNil(Client.normalizedQQLinkCode(code), code)
    }
    XCTAssertTrue(Client.validQQLinkID("12"))
    XCTAssertTrue(Client.validQQLinkID(String(repeating: "9", count: 19)))
    for id in ["", "0", "012", "1a", "../1", String(repeating: "9", count: 20)] {
      XCTAssertFalse(Client.validQQLinkID(id), id)
    }
  }

  func testTheListKeepsWellFormedRowsAndRejectsAMalformedResponse() throws {
    let rows = (1...12).map { #"{"id":"\#($0)","created_at":"2026-10-17T12:00:00Z"}"# }.joined(separator: ",")
    let links = try Client.parseQQLinks(Data(#"""
      {"links":[{"id":"7","created_at":"2026-10-17T12:00:00+08:00"},{"id":"0","created_at":"x"},{"id":"8"},"x",
      {"id":"9","created_at":"2026-10-17T13:00:00Z"}]}
      """#.utf8))
    XCTAssertEqual(links, [.init(id: "7", createdAt: "2026-10-17T12:00:00+08:00"), .init(id: "9", createdAt: "2026-10-17T13:00:00Z")])
    XCTAssertEqual(try Client.parseQQLinks(Data("{\"links\":[\(rows)]}".utf8)).count, 10)
    for json in [#"{}"#, #"{"links":{}}"#, "[]", "x"] {
      XCTAssertThrowsError(try Client.parseQQLinks(Data(json.utf8)), json)
    }
  }

  func testUploadSendsTheSharedBodyToTheOnlyPathItAccepts() async throws {
    TypingUploadProtocol.respond(204, "")
    let body = Data(#"{"device":"d","platform":"ios","days":[]}"#.utf8)
    try await client().uploadTypingStatistics(path: "/v1/users/me/typing-statistics", body: body, token: "session")
    XCTAssertEqual(TypingUploadProtocol.requests(),
                   [.init(method: "PUT", path: "/v1/users/me/typing-statistics", body: body, authorization: "Bearer session")])
    for path in ["/v1/users/me/phrases", "/v1/users/me/typing-statistics/x"] {
      do {
        try await client().uploadTypingStatistics(path: path, body: body, token: "session")
        XCTFail("sent to \(path)")
      } catch let failure as Client.Failure {
        XCTAssertEqual(failure.status, 400)
      }
    }
    do {
      try await client().uploadTypingStatistics(path: "/v1/users/me/typing-statistics", body: Data(), token: "session")
      XCTFail("sent an empty body")
    } catch let failure as Client.Failure {
      XCTAssertEqual(failure.status, 400)
    }
    XCTAssertEqual(TypingUploadProtocol.requests().count, 1)

    TypingUploadProtocol.respond(204, "")
    try await client().deleteTypingStatistics(token: "session")
    XCTAssertEqual(TypingUploadProtocol.requests().map(\.method), ["DELETE"])
  }

  func testRedeemSendsTheNormalizedCodeAndRefusesABadOneWithoutTheNetwork() async throws {
    TypingUploadProtocol.respond(201, #"{"id":"42","created_at":"2026-10-17T12:00:00Z"}"#)
    let link = try await client().redeemQQLink(code: "1234 5678", token: "session")
    XCTAssertEqual(link, .init(id: "42", createdAt: "2026-10-17T12:00:00Z"))
    let sent = try XCTUnwrap(TypingUploadProtocol.requests().first)
    XCTAssertEqual(sent.method, "POST")
    XCTAssertEqual(try JSONSerialization.jsonObject(with: sent.body) as? [String: String], ["code": "12345678"])

    TypingUploadProtocol.respond(201, "")
    do {
      _ = try await client().redeemQQLink(code: "1234", token: "session")
      XCTFail("redeemed a short code")
    } catch let failure as Client.Failure {
      XCTAssertEqual(failure.code, "invalid_qq_link_code")
      XCTAssertEqual(Client.qqLinkMessage(failure), "绑定码是 8 位数字，请检查后重试。")
    }
    XCTAssertTrue(TypingUploadProtocol.requests().isEmpty)

    TypingUploadProtocol.respond(404, #"{"error":{"code":"qq_link_code_not_found","message":"x"}}"#)
    do {
      _ = try await client().redeemQQLink(code: "12345678", token: "session")
      XCTFail("expected a refusal")
    } catch {
      XCTAssertEqual(Client.qqLinkMessage(error),
                     "绑定码不对或已过期（10 分钟内有效，只能用一次）。请在 QQ 群里重新发 /bind 拿一个新的。")
    }
  }

  func testListAndDelete() async throws {
    TypingUploadProtocol.respond(200, #"{"links":[{"id":"3","created_at":"2026-10-17T12:00:00Z"}]}"#)
    let links = try await client().qqLinks(token: "session")
    XCTAssertEqual(links.map(\.id), ["3"])
    TypingUploadProtocol.respond(204, "")
    try await client().deleteQQLink(id: "3", token: "session")
    XCTAssertEqual(TypingUploadProtocol.requests().map(\.path), ["/v1/users/me/qq-links/3"])
    TypingUploadProtocol.respond(204, "")
    do {
      try await client().deleteQQLink(id: "../3", token: "session")
      XCTFail("deleted a malformed id")
    } catch let failure as Client.Failure {
      XCTAssertEqual(failure.status, 400)
    }
    XCTAssertTrue(TypingUploadProtocol.requests().isEmpty)
  }

  /// 说法与共享设置页、Android 相同。
  func testMessages() {
    XCTAssertEqual(Client.qqLinkMessage(Client.Failure(status: 409, code: "qq_link_limit")),
                   "这个账号已经绑定了 10 个 QQ 群友，先解绑一个再试。")
    XCTAssertEqual(Client.qqLinkMessage(Client.Failure(status: 429, code: "rate_limit_exceeded")), "尝试次数太多，请一小时后再试。")
    XCTAssertEqual(Client.typingUploadMessage(Client.Failure(status: 401), enabling: true), "先在「我的」页登录水杉账号，再打开上传。")
    XCTAssertEqual(Client.typingUploadMessage(Client.Failure(status: 429), enabling: true), "上传过于频繁，过一会儿会自动再传。")
    XCTAssertEqual(Client.typingUploadMessage(Client.Failure(status: 500), enabling: true), "已打开上传，但这次没有传上去，稍后会自动再试。")
    XCTAssertEqual(Client.typingUploadMessage(URLError(.notConnectedToInternet), enabling: false),
                   "已关闭上传，但云端的数据没有删掉，请联网后打开再关闭一次。")
    XCTAssertEqual(Client.accountServiceMessage(URLError(.timedOut)), "连不上服务器，请检查网络后再试。")
    XCTAssertEqual(Client.accountServiceMessage(Client.Failure(status: 401)), "登录已失效，请重新登录后再试。")
    XCTAssertEqual(Client.accountServiceMessage(Client.Failure(status: 503)), "这项服务暂时不可用，请稍后再试。")
    XCTAssertEqual(Client.accountServiceMessage(Client.Failure(status: 500, code: "boom")), "没有成功，请稍后再试（HTTP 500 boom）。")
  }
}

import XCTest
@testable import MSIMEBackend

private final class PhrasesProtocol: URLProtocol {
  private static let lock = NSLock()
  private static var status = 200
  private static var reply = ""
  private static var bodies: [Data] = []

  static func respond(_ status: Int, _ reply: String) {
    lock.lock(); defer { lock.unlock() }
    self.status = status
    self.reply = reply
    bodies = []
  }

  static func sentBodies() -> [Data] {
    lock.lock(); defer { lock.unlock() }
    return bodies
  }

  override class func canInit(with request: URLRequest) -> Bool { request.url?.path == "/v1/users/me/phrases" }
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
    if request.httpMethod == "PUT" { Self.bodies.append(body) }
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

final class BackendPhrasesTests: XCTestCase {
  private typealias Phrase = BackendSyncPolicy.Phrase

  private func parse(_ json: String) throws -> BackendAccountClient.CloudPhrases {
    try BackendAccountClient.parsePhrases(Data(json.utf8))
  }

  private func client() -> BackendAccountClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [PhrasesProtocol.self]
    return BackendAccountClient(configuration: configuration)
  }

  func testParseSortsByPositionAndKeepsResponseOrderForBadPositions() throws {
    let value = try parse(#"""
      {"revision":4,"phrases":[
        {"id":"c","text":"三","group":"工作","position":2},
        {"id":"a","text":"一","position":0},
        {"id":"x","text":"无位置","group":null},
        {"id":"b","text":"二","position":1.5},
        {"id":"d","text":"越界","position":900}
      ]}
      """#)
    XCTAssertEqual(value.revision, 4)
    // Bad positions fall back to the row's index: x→2, b→3, d→4; "三" also sits at 2 but came first.
    XCTAssertEqual(value.phrases.map(\.id), ["a", "c", "x", "b", "d"])
    XCTAssertEqual(value.phrases.first { $0.id == "c" }?.group, "工作")
    XCTAssertEqual(value.phrases.first { $0.id == "x" }?.group, "")
  }

  func testParseRejectsMalformedResponses() {
    let rows = (0...500).map { #"{"id":"p\#($0)","text":"t\#($0)"}"# }.joined(separator: ",")
    for json in [
      #"{"phrases":[]}"#,
      #"{"revision":-1,"phrases":[]}"#,
      #"{"revision":1.5,"phrases":[]}"#,
      #"{"revision":true,"phrases":[]}"#,
      #"{"revision":1}"#,
      #"{"revision":1,"phrases":[{"id":1,"text":"a"}]}"#,
      #"{"revision":1,"phrases":[{"id":"a"}]}"#,
      #"{"revision":1,"phrases":[{"id":"a","text":"b","group":3}]}"#,
      #"{"revision":1,"phrases":["a"]}"#,
      "{\"revision\":1,\"phrases\":[\(rows)]}",
    ] {
      XCTAssertThrowsError(try parse(json), json)
    }
    XCTAssertEqual(try parse(#"{"revision":0,"phrases":[]}"#), .init(revision: 0, phrases: []))
  }

  func testPutSendsTheNormalizedListWithTheReadRevision() async throws {
    PhrasesProtocol.respond(200, #"{"revision":8,"phrases":[{"id":"a","text":"一","group":"","position":0}]}"#)
    let saved = try await client().putPhrases(revision: 7, phrases: [
      Phrase(id: "a", text: "一", position: 5),
      Phrase(id: "b", text: "一"),
      Phrase(id: "c", text: "  "),
    ], token: "session")
    XCTAssertEqual(saved.revision, 8)
    let body = try XCTUnwrap(PhrasesProtocol.sentBodies().first)
    let root = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
    XCTAssertEqual(root["revision"] as? Int, 7)
    let rows = try XCTUnwrap(root["phrases"] as? [[String: Any]])
    XCTAssertEqual(rows.count, 1)
    XCTAssertEqual(rows[0]["id"] as? String, "a")
    XCTAssertEqual(rows[0]["group"] as? String, "")
    XCTAssertEqual(rows[0]["position"] as? Int, 0)
  }

  func testPutConflictIsRecognisedForRetry() async {
    PhrasesProtocol.respond(409, #"{"error":{"code":"revision_conflict","message":"x"}}"#)
    do {
      _ = try await client().putPhrases(revision: 1, phrases: [], token: "session")
      XCTFail("expected a conflict")
    } catch {
      XCTAssertTrue(BackendSyncPolicy.isConflict(error))
    }
  }

  func testMergeKeepsLocalByIdThenDedupesByText() {
    let local = [Phrase(id: "1", text: "本机改过"), Phrase(id: "2", text: "共有")]
    let cloud = [Phrase(id: "1", text: "云端旧的", group: "g"), Phrase(id: "3", text: "共有"), Phrase(id: "4", text: "云端独有")]
    let merged = BackendSyncPolicy.mergePhrases(local, cloud)
    XCTAssertEqual(merged.map(\.id), ["1", "2", "4"])
    XCTAssertEqual(merged.map(\.text), ["本机改过", "共有", "云端独有"])
    XCTAssertEqual(merged.map(\.position), [0, 1, 2])
  }

  func testNormalizedDropsUnusableTextAndCapsTheList() {
    let long = String(repeating: "字", count: 2000)
    let tooLong = String(repeating: "😀", count: 1001)
    let input = [Phrase(id: "", text: "无 id"), Phrase(id: "a", text: long), Phrase(id: "b", text: tooLong),
                 Phrase(id: "c", text: "带\0NUL"), Phrase(id: "d", text: " \n ")]
      + (0..<600).map { Phrase(id: "p\($0)", text: "第\($0)条") }
    let result = BackendSyncPolicy.normalized(input)
    XCTAssertEqual(result.count, 500)
    XCTAssertEqual(result.first?.id, "a")
    XCTAssertFalse(result.contains { ["b", "c", "d", ""].contains($0.id) })
    XCTAssertEqual(result.last?.position, 499)
  }

  func testUploadKeepsUnheldCloudPhrasesButNotLocalDeletions() {
    let local = [Phrase(id: "1", text: "留着")]
    let cloud = [Phrase(id: "1", text: "留着"), Phrase(id: "2", text: "本机删掉的"), Phrase(id: "3", text: "本机放不下的")]
    let upload = BackendSyncPolicy.uploadPhrases(local, cloud, unheld: ["本机放不下的"])
    XCTAssertEqual(upload.map(\.text), ["留着", "本机放不下的"])
  }

  func testLocalPlanAlignsByText() {
    let plan = BackendSyncPolicy.localPlan([(id: "l1", text: "甲"), (id: "l2", text: "乙")],
                                           target: [Phrase(id: "c1", text: "乙"), Phrase(id: "c2", text: "丙")])
    XCTAssertEqual(plan.add, ["丙"])
    XCTAssertEqual(plan.remove, ["l1"])
  }

  func testLocalListTakesTheCloudGroupForTheSameText() {
    let mine = BackendSyncPolicy.asCloudPhrases([(id: "l1", text: "甲"), (id: "l2", text: "乙")],
                                                cloud: [Phrase(id: "c", text: "乙", group: "家")])
    XCTAssertEqual(mine, [Phrase(id: "l1", text: "甲", group: "", position: 0), Phrase(id: "l2", text: "乙", group: "家", position: 1)])
  }
}

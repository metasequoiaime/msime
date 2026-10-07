import Foundation
import XCTest
@testable import MSIMEBackend

private final class ChangesProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let cursor = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "after" }?.value
    let body: String
    switch cursor {
    case "0":
      body = #"{"changes":[{"revision":2,"previous":null,"replacement":null,"ranking":[{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","kind":"pinyin","code":"ni","word":"你","weight":8,"revision":2,"user_inserted":false}],"selection":{"context":"pinyin","code":"ni","word":"你","count":0}},{"revision":3,"position":{"context":"pinyin","code":"ni","word":"你","position":0}},{"revision":4,"reset":true}],"next":4,"has_more":true}"#
    case "4": body = #"{"changes":[],"next":4,"has_more":false}"#
    case "5": body = #"{"changes":[{"revision":5}],"next":5,"has_more":false}"#
    case "6": body = #"{"changes":[],"next":7,"has_more":false}"#
    case "8": body = #"{"changes":[{"revision":9,"ranking":[{"id":"","kind":"pinyin","code":"ni","word":"你","weight":1,"revision":9}]}],"next":9,"has_more":false}"#
    default: body = #"{"changes":[],"next":7,"has_more":true}"#
    }
    let status = request.url?.path == "/v1/users/me/dictionary/changes" && request.httpMethod == "GET" && request.value(forHTTPHeaderField: "Authorization") == "Bearer synthetic" ? 200 : 400
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type":"application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedDictionaryChangeProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool {
    request.url?.path == "/v1/users/me/dictionaries/pinyin"
      || request.url?.path == "/v1/users/me/dictionaries/pinyin/edit"
  }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type":"application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    let body = #"{"revision":1,"previous":{"id":"../logout","kind":"pinyin","code":"ni","word":"你","weight":1,"revision":1},"replacement":null}"#
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class DictionaryMutationInputProtocol: URLProtocol {
  private static let lock = NSLock()
  private static var requestCount = 0

  static func reset() {
    lock.lock(); defer { lock.unlock() }
    requestCount = 0
  }

  static func requests() -> Int {
    lock.lock(); defer { lock.unlock() }
    return requestCount
  }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    Self.lock.lock(); Self.requestCount += 1; Self.lock.unlock()
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type":"application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"revision":1,"previous":null,"replacement":null}"#.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
final class BackendDictionaryChangesTests: XCTestCase {
  private func client() -> BackendAccountClient {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [ChangesProtocol.self]
    return BackendAccountClient(configuration: config)
  }
  func testPreservesRankingOwnershipCounterClearPositionRemovalAndReset() async throws {
    let page = try await client().dictionaryChanges(after: 0, token: "synthetic")
    XCTAssertEqual(page.changes.count, 3)
    XCTAssertEqual(page.changes[0].ranking?.first?.user_inserted, false)
    XCTAssertEqual(page.changes[0].selection?.count, 0)
    XCTAssertEqual(page.changes[1].position?.position, 0)
    XCTAssertEqual(page.changes[2].reset, true)
    XCTAssertTrue(page.has_more)
    let end = try await client().dictionaryChanges(after: page.next, token: "synthetic")
    XCTAssertTrue(end.changes.isEmpty)
    XCTAssertEqual(end.next, 4)
    XCTAssertFalse(end.has_more)
  }
  func testRejectsNonAdvancingAndInconsistentCursors() async throws {
    for cursor: Int64 in [5, 6, 7, 8] {
      do {
        _ = try await client().dictionaryChanges(after: cursor, token: "synthetic")
        XCTFail("Invalid change cursor accepted")
      } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 502) }
    }
  }
  func testRejectsInvalidRequestBounds() async throws {
    for (cursor, limit): (Int64, Int) in [(-1, 100), (0, 0), (0, 101)] {
      do {
        _ = try await client().dictionaryChanges(after: cursor, limit: limit, token: "synthetic")
        XCTFail("Invalid request accepted")
      } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    }
  }
  func testDictionaryMutationRejectsMalformedChange() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedDictionaryChangeProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    do {
      _ = try await client.addDictionary(.pinyin, value: .init(code: "ni", word: "你", weight: 1), token: "synthetic")
      XCTFail("malformed dictionary change accepted")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }
  }
  func testDictionaryMutationsRejectInvalidValuesBeforeSending() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [DictionaryMutationInputProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let invalid = BackendAccountClient.DictionaryValue(code: "ni", word: "合成", weight: -1)
    DictionaryMutationInputProtocol.reset()
    do {
      _ = try await client.addDictionary(.pinyin, value: invalid, token: "synthetic")
      XCTFail("invalid dictionary value sent")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 400)
    }
    XCTAssertEqual(DictionaryMutationInputProtocol.requests(), 0)

    let entry = BackendAccountClient.DictionaryEntry(
      id: String(repeating: "a", count: 64), kind: .pinyin, code: "ni", word: "合成", weight: 1, revision: 1)
    DictionaryMutationInputProtocol.reset()
    do {
      _ = try await client.updateDictionary(entry, value: invalid, token: "synthetic")
      XCTFail("invalid dictionary update sent")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 400)
    }
    XCTAssertEqual(DictionaryMutationInputProtocol.requests(), 0)

    let catalogEntry = BackendAccountClient.CatalogEntry(kind: .pinyin, code: "ni", word: "合成", weight: 1)
    DictionaryMutationInputProtocol.reset()
    do {
      _ = try await client.editCatalog(catalogEntry, revision: 1, replacement: invalid, token: "synthetic")
      XCTFail("invalid dictionary replacement sent")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 400)
    }
    XCTAssertEqual(DictionaryMutationInputProtocol.requests(), 0)
  }
}

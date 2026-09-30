import Foundation
import XCTest
@testable import MSIMEBackend

private final class DictionaryCatalogProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var status = 200
    var data = Data(#"{"entries":[{"kind":"pinyin","code":"ni'hao","word":"你好","weight":100000}],"offset":0,"has_more":false,"revision":42,"normalized":"ni'hao"}"#.utf8)
    if request.httpMethod == "POST" {
      var body = request.httpBody ?? Data()
      if let stream = request.httpBodyStream {
        stream.open(); defer { stream.close() }
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
          let count = stream.read(&buffer, maxLength: buffer.count)
          if count <= 0 { break }
          body.append(contentsOf: buffer.prefix(count))
        }
      }
      let value = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
      if !(value?["replacement"] is NSNull) || value?["revision"] as? Int != 42 { status = 400 }
      data = Data(#"{"revision":43,"previous":null,"replacement":null}"#.utf8)
    }
    // Form-style query parsing on the server would read a bare '+' as a space.
    if request.url!.absoluteString.contains("+") { status = 400 }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type":"application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}
private final class MalformedDictionaryCatalogProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { request.url?.path.hasSuffix("/catalog") == true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
    let query = components?.queryItems?.first { $0.name == "q" }?.value ?? ""
    let entry: [String: Any]
    switch query {
    case "bad-code": entry = ["kind": "pinyin", "code": "", "word": "你", "weight": 1]
    case "bad-weight": entry = ["kind": "pinyin", "code": "ni", "word": "你", "weight": -1]
    default: entry = ["kind": "pinyin", "code": "ni", "word": "你", "weight": 1]
    }
    let object: [String: Any] = [
      "entries": [entry], "offset": query == "bad-offset" ? 100 : 0,
      "has_more": false, "revision": query == "bad-revision" ? -1 : 1,
      "normalized": query == "bad-normalized" ? "ni\u{0001}" : "ni",
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
final class BackendDictionaryCatalogTests: XCTestCase {
  func testBaseCatalogHasNoPersonalIDAndDeletionRequiresExplicitNull() async throws {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [DictionaryCatalogProtocol.self]
    let client = BackendAccountClient(configuration: config)
    let catalog = try await client.dictionaryCatalog(.pinyin, code: "nihc", scheme: "shuangpin", token: "session")
    XCTAssertEqual(catalog.revision, 42)
    XCTAssertEqual(catalog.normalized, "ni'hao")
    let entry = try XCTUnwrap(catalog.entries.first)
    XCTAssertEqual(entry.word, "你好")
    let deleted = try await client.editCatalog(entry, revision: catalog.revision, replacement: nil, token: "session")
    XCTAssertEqual(deleted.revision, 43)
  }

  func testQueryPlusIsSentEscaped() async throws {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [DictionaryCatalogProtocol.self]
    let client = BackendAccountClient(configuration: config)
    _ = try await client.dictionaryCatalog(.pinyin, code: "C++", token: "session")
    var components = URLComponents()
    components.path = "/v1/users/me/dictionaries/english"
    components.queryItems = [.init(name: "q", value: "C++ x"), .init(name: "context", value: "a+b")]
    let path = try XCTUnwrap(BackendAccountClient.encodedPath(components))
    XCTAssertTrue(path.contains("q=C%2B%2B%20x"), path)
    XCTAssertTrue(path.contains("context=a%2Bb"), path)
    let decoded = URLComponents(string: path)?.queryItems
    XCTAssertEqual(decoded?.first { $0.name == "q" }?.value, "C++ x")
  }
  func testDictionaryCatalogRejectsMalformedServerEntries() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedDictionaryCatalogProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    for code in ["bad-code", "bad-weight", "bad-offset", "bad-revision", "bad-normalized"] {
      do {
        _ = try await client.dictionaryCatalog(.pinyin, code: code, token: "session")
        XCTFail("malformed dictionary catalog accepted: \(code)")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 0)
      }
    }
  }
}

import Foundation
import XCTest
@testable import MSIMEBackend

private final class DictionaryFileProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let chunk = Data(repeating: 65, count: 65536)
    let headers = request.url?.query == "truncated" ? ["Content-Type": "text/plain", "Content-Length": "3000000"] : ["Content-Type": "text/plain"]
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: headers)!, cacheStoragePolicy: .notAllowed)
    for _ in 0..<32 { client?.urlProtocol(self, didLoad: chunk) }
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class DictionaryImportValidationProtocol: URLProtocol {
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
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"imported":1,"revision":1}"#.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendDictionaryFileTests: XCTestCase {
  private func client() -> BackendAccountClient {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [DictionaryFileProtocol.self]
    return BackendAccountClient(configuration: config)
  }
  private func exports() throws -> Set<String> {
    Set(try FileManager.default.contentsOfDirectory(atPath: FileManager.default.temporaryDirectory.path).filter { $0.hasPrefix("msime-export-") })
  }
  func testExportStreamsPastOrdinaryJSONLimitIntoPrivateFile() async throws {
    let file = try await client().download("/v1/users/me/dictionaries/quick/export", token: "session", filename: "test.tsv", maximumBytes: 3 * 1024 * 1024)
    defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
    let attributes = try FileManager.default.attributesOfItem(atPath: file.path)
    XCTAssertEqual(attributes[.size] as? Int, 2 * 1024 * 1024)
    XCTAssertEqual((attributes[.posixPermissions] as? Int).map { $0 & 0o777 }, 0o600)
  }
  func testOversizedAndTruncatedDownloadsLeaveNoPartialExport() async throws {
    for (suffix, bound) in [("", 1_000_000), ("?truncated", 4_000_000)] {
      let before = try exports()
      do {
        _ = try await client().download("/v1/users/me/dictionaries/quick/export" + suffix, token: "session", filename: "test.tsv", maximumBytes: bound)
        XCTFail("invalid download accepted")
      } catch { }
      XCTAssertEqual(try exports(), before)
    }
  }
  func testImportChecksRawPayloadAndHanKindBeforeSending() async throws {
    for (kind, text, format) in [(BackendAccountClient.DictionaryKind.quick, "你好", BackendAccountClient.DictionaryFileFormat.hans), (.quick, String(repeating: "a", count: 65537), .standard)] {
      do { _ = try await client().importDictionary(kind, text: text, format: format, token: "session"); XCTFail("invalid import sent") }
      catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 400) }
    }
  }

  func testImportMatchesRawTextValidationContract() async throws {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [DictionaryImportValidationProtocol.self]
    let client = BackendAccountClient(configuration: config)

    DictionaryImportValidationProtocol.reset()
    for text in ["safe\u{0000}text", "safe\u{0007}text", "safe\u{0085}text"] {
      do {
        _ = try await client.importDictionary(.quick, text: text, format: .standard, token: "session")
        XCTFail("disallowed control character sent")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, 400)
      }
    }
    XCTAssertEqual(DictionaryImportValidationProtocol.requests(), 0)

    let boundary = String(repeating: "a", count: 65536)
    let result = try await client.importDictionary(.quick, text: boundary, format: .standard, token: "session")
    XCTAssertEqual(result.imported, 1)
    XCTAssertEqual(DictionaryImportValidationProtocol.requests(), 1)

    let tsv = try await client.importDictionary(.quick, text: "你好\tnihao\n", format: .standard, token: "session")
    XCTAssertEqual(tsv.imported, 1)
    XCTAssertEqual(DictionaryImportValidationProtocol.requests(), 2)
  }
}

import Foundation
import CryptoKit
import XCTest
@testable import MSIMESnapshot

private final class RestoreProtocol: URLProtocol {
  static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    do {
      let (response, data) = try Self.handler!(request)
      client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
      client?.urlProtocol(self, didLoad: data)
      client?.urlProtocolDidFinishLoading(self)
    } catch { client?.urlProtocol(self, didFailWithError: error) }
  }
  override func stopLoading() {}
}

final class SnapshotRestoreTests: XCTestCase {
  func testRefusesToFollowAResponseRedirect() {
    let delegate = SnapshotRestoreClient.Redirects()
    let expectation = expectation(description: "redirect completion")
    delegate.urlSession(URLSession.shared, task: URLSession.shared.dataTask(with: URL(string: "https://api.msime.app")!),
                        willPerformHTTPRedirection: HTTPURLResponse(url: URL(string: "https://api.msime.app")!, statusCode: 307, httpVersion: nil, headerFields: nil)!,
                        newRequest: URLRequest(url: URL(string: "https://redirect.invalid")!)) { request in
      XCTAssertNil(request)
      expectation.fulfill()
    }
    wait(for: [expectation], timeout: 1)
  }

  private func fixture() throws -> (URL, Data, String) {
    let body = Data(#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":1}"#.utf8) + Data([10])
    let digest = SHA256.hash(data: body).map { String(format: "%02x", $0) }.joined()
    let footer = try JSONSerialization.data(withJSONObject: ["type": "footer", "records": 1, "sha256": digest])
    let data = body + footer + Data([10])
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try data.write(to: url)
    return (url, data, digest)
  }
  private func session() -> URLSession {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [RestoreProtocol.self]
    return URLSession(configuration: config)
  }
  private func response(_ request: URLRequest, status: Int = 200, mime: String = "application/json",
                        length: String? = nil) -> HTTPURLResponse {
    var headers = ["Content-Type": mime]
    if let length { headers["Content-Length"] = length }
    return HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!
  }

  func testUploadsFrozenValidatedCopyWithAppleContract() async throws {
    let (file, original, digest) = try fixture()
    defer { try? FileManager.default.removeItem(at: file) }
    let transport = session()
    defer { transport.invalidateAndCancel(); RestoreProtocol.handler = nil }
    RestoreProtocol.handler = { request in
      XCTAssertEqual(request.httpMethod, "PUT")
      XCTAssertEqual(request.url?.path, "/v1/users/me/dictionary/snapshot")
      XCTAssertEqual(request.url?.query, "revision=7")
      XCTAssertEqual(request.timeoutInterval, 130)
      XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-ndjson")
      XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
      XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Length"), String(original.count))
      try Data("synthetic changed source".utf8).write(to: file)
      let stream = try XCTUnwrap(request.httpBodyStream)
      stream.open()
      defer { stream.close() }
      var uploaded = Data()
      var buffer = [UInt8](repeating: 0, count: 64)
      while true {
        let count = stream.read(&buffer, maxLength: buffer.count)
        if count == 0 { break }
        guard count > 0 else { throw stream.streamError ?? URLError(.cannotOpenFile) }
        uploaded.append(contentsOf: buffer.prefix(count))
      }
      XCTAssertEqual(uploaded, original)
      return (self.response(request), Data(#"{"revision":8,"reset":true}"#.utf8))
    }
    let result = try await SnapshotRestoreClient(session: transport).restore(
      file: file, expectedSHA256: digest, revision: 7, token: "synthetic-test-token")
    XCTAssertEqual(result.revision, 8)
    XCTAssertTrue(result.reset)
  }

  func testInvalidInputsAndTamperedSnapshotNeverReachTransport() async throws {
    let (file, original, digest) = try fixture()
    defer { try? FileManager.default.removeItem(at: file) }
    let transport = session()
    defer { transport.invalidateAndCancel(); RestoreProtocol.handler = nil }
    RestoreProtocol.handler = { _ in
      XCTFail("Invalid snapshot reached network transport")
      throw URLError(.badURL)
    }
    let client = SnapshotRestoreClient(session: transport)
    for (hash, revision, token) in [
      (String(repeating: "0", count: 64), Int64(7), "synthetic"),
      (digest, -1, "synthetic"), (digest, 7, ""), (digest, 7, "synthetic\r\nvalue"),
      ("invalid", 7, "synthetic")
    ] {
      do {
        _ = try await client.restore(file: file, expectedSHA256: hash, revision: revision, token: token)
        XCTFail("Invalid input accepted")
      } catch { XCTAssertEqual((error as? SnapshotFailure)?.status, 400) }
    }
    try original.dropLast(30).write(to: file)
    do {
      _ = try await client.restore(file: file, expectedSHA256: digest, revision: 7, token: "synthetic")
      XCTFail("Truncated snapshot accepted")
    } catch { XCTAssertEqual((error as? SnapshotFailure)?.status, 400) }
  }

  func testResponseValidationAndAmbiguousFailureDoNotRetry() async throws {
    let (file, _, digest) = try fixture()
    defer { try? FileManager.default.removeItem(at: file) }
    let transport = session()
    defer { transport.invalidateAndCancel(); RestoreProtocol.handler = nil }
    let client = SnapshotRestoreClient(session: transport)
    let cases: [(Int, String, String?, Data)] = [
      (409, "application/json", nil, Data()),
      (200, "text/plain", nil, Data(#"{"revision":8,"reset":true}"#.utf8)),
      (200, "application/json", nil, Data(#"{"revision":7,"reset":true}"#.utf8)),
      (200, "application/json", nil, Data(#"{"revision":8,"reset":false}"#.utf8)),
      (200, "application/json", nil, Data(#"{"revision":true,"reset":true}"#.utf8)),
      (200, "application/json", nil, Data("not-json".utf8)),
      (200, "application/json", "1048577", Data()),
      (200, "application/json", nil, Data(repeating: 32, count: 1048577))
    ]
    for (status, mime, length, body) in cases {
      var calls = 0
      RestoreProtocol.handler = { request in
        calls += 1
        return (self.response(request, status: status, mime: mime, length: length), body)
      }
      do {
        _ = try await client.restore(file: file, expectedSHA256: digest, revision: 7, token: "synthetic")
        XCTFail("Invalid response accepted")
      } catch { XCTAssertEqual((error as? SnapshotFailure)?.status, status == 409 ? 409 : 0) }
      XCTAssertEqual(calls, 1)
    }
    var calls = 0
    RestoreProtocol.handler = { _ in calls += 1; throw URLError(.networkConnectionLost) }
    do {
      _ = try await client.restore(file: file, expectedSHA256: digest, revision: 7, token: "synthetic")
      XCTFail("Ambiguous failure accepted")
    } catch { XCTAssertEqual((error as? URLError)?.code, .networkConnectionLost) }
    XCTAssertEqual(calls, 1)
  }
}

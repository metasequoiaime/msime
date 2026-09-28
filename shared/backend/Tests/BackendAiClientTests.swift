import Foundation
import XCTest
@testable import MSIMEBackend

private final class AiProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let body = #"{"choices":[{"message":{"content":"{\"candidates\":[{\"text\":\"你好\"}]}"}}]}"#
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class OversizedAiProtocol: URLProtocol {
  static let stopped = DispatchSemaphore(value: 0)
  static let release = DispatchSemaphore(value: 0)

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let body = Data(repeating: 0x41, count: 1_048_577)
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    DispatchQueue.global().async {
      Self.release.wait()
      self.client?.urlProtocolDidFinishLoading(self)
    }
  }
  override func stopLoading() {
    Self.stopped.signal()
    Self.release.signal()
  }
}

final class BackendAiClientTests: XCTestCase {
  private func client() -> BackendAiClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AiProtocol.self]
    return BackendAiClient(configuration: configuration)
  }

  func testParsesOpenAICompatibleCandidateResponse() async throws {
    let result = try await client().suggest(endpoint: URL(string: "https://ai.invalid/v1/chat/completions")!, model: "model", token: "session", segmentedPinyin: ["ni", "hao"], context: "", candidateLimit: 3)
    XCTAssertEqual(result.candidates.map(\.text), ["你好"])
  }

  func testRejectsUnsafeEndpointAndInvalidLimit() async {
    do {
      _ = try await client().suggest(endpoint: URL(string: "http://ai.invalid")!, model: "model", token: "session", segmentedPinyin: ["ni"], context: "", candidateLimit: 1)
      XCTFail("unsafe endpoint")
    } catch { }
    do {
      _ = try await client().suggest(endpoint: URL(string: "https://ai.invalid")!, model: "model", token: "session", segmentedPinyin: ["ni"], context: "", candidateLimit: 0)
      XCTFail("invalid limit")
    } catch { }
  }

  func testRefusesToFollowAResponseRedirect() {
    let delegate = BackendAiClient.Redirects()
    let expectation = expectation(description: "redirect completion")
    delegate.urlSession(URLSession.shared, task: URLSession.shared.dataTask(with: URL(string: "https://ai.invalid")!),
                        willPerformHTTPRedirection: HTTPURLResponse(url: URL(string: "https://ai.invalid")!, statusCode: 307, httpVersion: nil, headerFields: nil)!,
                        newRequest: URLRequest(url: URL(string: "https://redirect.invalid")!)) { request in
      XCTAssertNil(request)
      expectation.fulfill()
    }
    wait(for: [expectation], timeout: 1)
  }

  func testOversizedResponseCancelsStreamingRequest() async {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [OversizedAiProtocol.self]
    let client = BackendAiClient(configuration: configuration)
    let request = Task {
      try? await client.suggest(endpoint: URL(string: "https://ai.invalid/v1/chat/completions")!, model: "model",
                                token: "session", segmentedPinyin: ["ni"], context: "", candidateLimit: 1)
    }

    XCTAssertEqual(OversizedAiProtocol.stopped.wait(timeout: .now() + 2), .success)
    OversizedAiProtocol.release.signal()
    _ = await request.value
  }
}

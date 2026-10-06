import Foundation
import XCTest
@testable import MSIMEBackend

private final class AiProtocol: URLProtocol {
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

private final class MalformedAiResponseProtocol: URLProtocol {
  static var candidateText = " \n"
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let inner = try! JSONSerialization.data(withJSONObject: ["candidates": [["text": Self.candidateText]]])
    let body = try! JSONSerialization.data(withJSONObject: ["choices": [["message": ["content": String(decoding: inner, as: UTF8.self)]]]])
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
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

  func testRejectsWhitespaceCandidateResponse() async {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedAiResponseProtocol.self]
    MalformedAiResponseProtocol.candidateText = " \n"
    do {
      _ = try await BackendAiClient(configuration: configuration).suggest(
        endpoint: URL(string: "https://ai.invalid/v1/chat/completions")!, model: "model",
        token: "session", segmentedPinyin: ["ni"], context: "", candidateLimit: 1)
      XCTFail("whitespace candidate accepted")
    } catch { }
  }

  func testRejectsControlCharacterCandidateResponse() async {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedAiResponseProtocol.self]
    MalformedAiResponseProtocol.candidateText = "bad\u{0001}text"
    do {
      _ = try await BackendAiClient(configuration: configuration).suggest(
        endpoint: URL(string: "https://ai.invalid/v1/chat/completions")!, model: "model",
        token: "session", segmentedPinyin: ["ni"], context: "", candidateLimit: 1)
      XCTFail("control character candidate accepted")
    } catch { }
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

  func testRejectsOversizedPinyinBeforeSending() async {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [AiProtocol.self]
    let client = BackendAiClient(configuration: configuration)
    AiProtocol.reset()
    do {
      _ = try await client.suggest(endpoint: URL(string: "https://ai.invalid/v1/chat/completions")!, model: "model",
        token: "session", segmentedPinyin: [String(repeating: "a", count: 33)], context: "", candidateLimit: 1)
      XCTFail("oversized pinyin segment accepted")
    } catch { }
    XCTAssertEqual(AiProtocol.requests(), 0)
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

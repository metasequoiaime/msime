import Foundation
import XCTest

private final class KeyboardFallbackFixture: URLProtocol, @unchecked Sendable {
  static var response = Data()
  static var requestedRedirect = false
  static var requestedSecret = false

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

  override func startLoading() {
    if request.url?.path == "/redirect" {
      Self.requestedRedirect = true
    }
    if request.url?.path == "/secret" {
      Self.requestedSecret = true
    }
    if request.url?.path == "/redirect" {
      let target = URL(string: "https://other.invalid/secret")!
      let response = HTTPURLResponse(url: request.url!, statusCode: 302, httpVersion: nil,
                                     headerFields: ["Location": target.absoluteString])!
      client?.urlProtocol(self, wasRedirectedTo: URLRequest(url: target), redirectResponse: response)
      return
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                   headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Self.response)
    client?.urlProtocolDidFinishLoading(self)
  }

  override func stopLoading() {}
}

final class KeyboardServiceFallbackTests: XCTestCase {
  private func configuration(endpoint: String) -> CustomServiceConfiguration {
    var value = CustomServiceConfiguration()
    value.endpoint = endpoint
    value.model = "fixture-model"
    return value
  }

  private func sessionConfiguration() -> URLSessionConfiguration {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [KeyboardFallbackFixture.self]
    return configuration
  }

  func testRejectsAnOversizedResponse() async throws {
    KeyboardFallbackFixture.response = Data(repeating: 0x20, count: 1024 * 1024 + 1)
    defer { KeyboardFallbackFixture.response = Data() }

    do {
      _ = try await CustomServiceClient.request(
        kind: .ai, configuration: configuration(endpoint: "https://fixture.invalid/oversized"),
        text: "fixture", token: "fixture-token", sessionConfiguration: sessionConfiguration())
      XCTFail("oversized response was accepted")
    } catch let error as ServiceFailure {
      XCTAssertEqual(error.message, "服务响应过大。")
    }
  }

  func testRefusesToFollowAResponseRedirect() async throws {
    KeyboardFallbackFixture.requestedSecret = false
    KeyboardFallbackFixture.requestedRedirect = false
    defer {
      KeyboardFallbackFixture.requestedSecret = false
      KeyboardFallbackFixture.requestedRedirect = false
    }

    do {
      _ = try await CustomServiceClient.request(
        kind: .ai, configuration: configuration(endpoint: "https://fixture.invalid/redirect"),
        text: "fixture", token: "fixture-token", sessionConfiguration: sessionConfiguration())
      XCTFail("redirect response was accepted")
    } catch {
      XCTAssertTrue(KeyboardFallbackFixture.requestedRedirect)
      XCTAssertFalse(KeyboardFallbackFixture.requestedSecret)
    }
  }
}

import Foundation
import XCTest
@testable import MSIMEBackend

private final class ChatProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var body = #"{"data":[{"id":"luna"},{"id":"sol"}],"default_model":"luna"}"#
    var status = 200
    if request.value(forHTTPHeaderField: "Authorization") != "Bearer session" { status = 401 }
    if request.url!.path == "/v1/chat/completions" {
      XCTAssertEqual(request.timeoutInterval, 125)
      let stream = request.httpBodyStream
      stream?.open(); defer { stream?.close() }
      var data = request.httpBody ?? Data()
      if data.isEmpty, let stream {
        var buffer = [UInt8](repeating: 0, count: 1024)
        while stream.hasBytesAvailable { let count = stream.read(&buffer, maxLength: buffer.count); if count <= 0 { break }; data.append(contentsOf: buffer.prefix(count)) }
      }
      let requestBody = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
      XCTAssertEqual(requestBody?["model"] as? String, "sol")
      XCTAssertEqual(requestBody?["stream"] as? Bool, false)
      let messages = requestBody?["messages"] as? [[String: String]]
      XCTAssertEqual(messages?.last?["content"], "你好")
      body = #"{"choices":[{"message":{"role":"assistant","content":"你好，收到消息。"}}]}"#
    }
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type":"application/json"])!, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8)); client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class MalformedChatResponseProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"choices":[{"message":{"role":"assistant","content":"bad\u0007reply"}}]}"#.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class MalformedChatModelsProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"data":[{"id":"bad\u0007model"}],"default_model":"bad\u0007model"}"#.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendChatClientTests: XCTestCase {
  func testModelsAndSelectedModelReachBackend() async throws {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [ChatProtocol.self]
    let api = BackendAccountClient(configuration: config)
    let models = try await api.chatModels(token: "session")
    XCTAssertEqual(models.data.map(\.id), ["luna", "sol"])
    let reply = try await api.chat(messages: [.init(role: "user", content: "你好")], model: "sol", token: "session")
    XCTAssertEqual(reply, "你好，收到消息。")
    do { _ = try await api.chatModels(token: "expired"); XCTFail("Accepted expired account") }
    catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 401) }
  }
  func testRejectsOverlongMessagesBeforeSending() async throws {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [ChatProtocol.self]
    let api = BackendAccountClient(configuration: config)
    do {
      _ = try await api.chat(messages: [.init(role: "user", content: String(repeating: "字", count: 6000))], model: "sol", token: "session")
      XCTFail("Oversized message sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
  }

  func testRejectsInvalidModelAndMessageTextBeforeSending() async throws {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [ChatProtocol.self]
    let api = BackendAccountClient(configuration: config)
    let invalidRequests: [(String, [BackendAccountClient.ChatMessage])] = [
      (String(repeating: "m", count: 201), [.init(role: "user", content: "valid")]),
      ("model\u{0007}", [.init(role: "user", content: "valid")]),
      ("sol", [.init(role: "user", content: " \n")]),
      ("sol", [.init(role: "user", content: "bad\u{0007}text")])
    ]
    for (model, messages) in invalidRequests {
      do {
        _ = try await api.chat(messages: messages, model: model, token: "session")
        XCTFail("invalid chat request sent")
      } catch let failure as BackendAccountClient.Failure {
        XCTAssertEqual(failure.status, 400)
      }
    }
  }

  func testRejectsControlCharactersInChatResponse() async throws {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [MalformedChatResponseProtocol.self]
    do {
      _ = try await BackendAccountClient(configuration: config).chat(
        messages: [.init(role: "user", content: "valid")], model: "sol", token: "session")
      XCTFail("invalid chat response accepted")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 502)
    }
  }

  func testRejectsControlCharactersInChatModelCatalog() async throws {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [MalformedChatModelsProtocol.self]
    do {
      _ = try await BackendAccountClient(configuration: config).chatModels(token: "session")
      XCTFail("invalid chat model catalog accepted")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 0)
    }
  }
}

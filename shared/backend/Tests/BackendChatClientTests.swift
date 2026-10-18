import Foundation
import XCTest
@testable import MSIMEBackend

private final class TranslationProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    var data = request.httpBody ?? Data()
    if data.isEmpty, let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var buffer = [UInt8](repeating: 0, count: 1024)
      while stream.hasBytesAvailable {
        let count = stream.read(&buffer, maxLength: buffer.count)
        if count <= 0 { break }
        data.append(contentsOf: buffer.prefix(count))
      }
    }
    let requestBody = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    XCTAssertEqual(requestBody?["source_lang"] as? String, "ZH")
    XCTAssertEqual(requestBody?["target_lang"] as? String, "EN")
    XCTAssertEqual(requestBody?["texts"] as? [String], ["你好"])
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type":"application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"code":200,"data":["hello"]}"#.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

/// 键盘扩展仍用 Swift 的账号释义传输（负责人决定：未测，扩展保留 Swift）；对话、模型目录已经在 Rust（`msime_client_account_chat_stream`、`chat_models`）。
final class BackendChatClientTests: XCTestCase {
  func testTranslationUsesUppercaseBackendLanguageCode() async throws {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [TranslationProtocol.self]
    let values = try await BackendAccountClient(configuration: config).translate(
      texts: ["你好"], target: "en", token: "session")
    XCTAssertEqual(values, ["hello"])
  }
}

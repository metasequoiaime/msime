import XCTest
import UIKit

private final class ThemeSessionStorage: BackendSessionStorage, @unchecked Sendable {
  func load() throws -> BackendSavedSession? {
    .init(tokens: .init(access_token: String(repeating: "a", count: 64), refresh_token: String(repeating: "b", count: 64),
      token_type: "Bearer", expires_in: 900, user: .init(id: "synthetic-theme-user", display_name: "测试", created_at: "")),
      expiresAt: Date().addingTimeInterval(900))
  }
  func save(_ session: BackendSavedSession) throws {}
  func clear() throws {}
}
private final class ThemeProtocol: URLProtocol, @unchecked Sendable {
  private final class State: @unchecked Sendable {
    let lock = NSLock()
    var image = Data()
    var calls = [String]()
  }
  private static let state = State()
  static func reset(image: Data) { state.lock.lock(); defer { state.lock.unlock() }; state.image = image; state.calls = [] }
  static var paths: [String] { state.lock.lock(); defer { state.lock.unlock() }; return state.calls }
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    Self.state.lock.lock(); Self.state.calls.append(request.url!.path); let image = Self.state.image; Self.state.lock.unlock()
    do {
      let body: Data
      switch request.url!.path {
      case "/v1/models":
        body = Data(#"{"object":"list","data":[{"id":"fixture-model","object":"model"}],"default_model":"fixture-model"}"#.utf8)
      case "/v1/chat/completions":
        let fixture = try XCTUnwrap(Bundle(for: AISkinGenerationTests.self).url(forResource: "AIThemeReference", withExtension: "json"))
        // Add synthetic scene prompts to the recorded legacy response to verify
        // that keycap descriptions never enter the image request.
        var recorded = try JSONSerialization.jsonObject(with: Data(contentsOf: fixture)) as! [String: Any]
        var plans = recorded["skins"] as! [[String: Any]]
        for index in plans.indices {
          plans[index]["artworkPrompt"] = "原创背景场景 \(index)：竹林里的动物茶会，柔和水彩描绘晨雾、溪流与花叶，角色位于画面边缘，中央保持安静留白，营造独特而温暖的故事氛围"
        }
        recorded["skins"] = plans
        let content = String(data: try JSONSerialization.data(withJSONObject: recorded), encoding: .utf8)!
        body = try JSONSerialization.data(withJSONObject: ["choices": [["message": ["role": "assistant", "content": content]]]])
      case "/v1/skins/jobs":
        var payload = request.httpBody ?? Data()
        if payload.isEmpty, let stream = request.httpBodyStream {
          stream.open(); defer { stream.close() }
          var buffer = [UInt8](repeating: 0, count: 1024)
          while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count <= 0 { break }; payload.append(contentsOf: buffer.prefix(count))
          }
        }
        let input = try JSONSerialization.jsonObject(with: payload) as! [String: String]
        XCTAssertTrue(input["prompt"]?.hasPrefix("原创背景场景 ") == true)
        XCTAssertFalse(input["prompt"]?.contains("键帽") == true)
        let id = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased() + String(repeating: "a", count: 16)
        body = try JSONSerialization.data(withJSONObject: ["id": id, "state": "running"])
      case let path where path.hasPrefix("/v1/skins/jobs/"):
        body = request.httpMethod == "DELETE" ? Data() : try JSONSerialization.data(withJSONObject: ["id": request.url!.lastPathComponent, "state": "succeeded", "artwork": ["b64_json": image.base64EncodedString(), "mime_type": "image/png", "width": 64, "height": 64]])
      default: throw URLError(.unsupportedURL)
      }
      client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
        headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
      client?.urlProtocol(self, didLoad: body)
      client?.urlProtocolDidFinishLoading(self)
    } catch { client?.urlProtocol(self, didFailWithError: error) }
  }
  override func stopLoading() {}
}
final class AISkinGenerationTests: XCTestCase {
  private func client() -> BackendAccountClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ThemeProtocol.self]
    return BackendAccountClient(configuration: configuration)
  }
  @MainActor
  func testThreeIllustrationsAreBoundedAndDoNotAutoSaveOrApply() async throws {
    let current = CustomKeyboardSkinStore.current, saved = CustomSkinLibrary.designs
    let image = UIGraphicsImageRenderer(size: CGSize(width: 64, height: 64)).image {
      UIColor.systemTeal.setFill(); $0.fill(CGRect(x: 0, y: 0, width: 64, height: 64))
    }
    ThemeProtocol.reset(image: try XCTUnwrap(image.pngData()))
    let api = client()
    var progress: [Int] = []
    let proposals = try await AISkinService.generate("合成主题验收", client: api,
      account: BackendAccountSession(api: api, storage: ThemeSessionStorage()), progress: { progress.append($0) })
    XCTAssertEqual(progress, [1, 2, 3])
    XCTAssertEqual(proposals.map(\.name), ["鼠尾草晨雾", "奶油月光", "香草信笺"])
    XCTAssertEqual(proposals.count, 3)
    XCTAssertEqual(Set(proposals.compactMap { $0.design.keyShape }).count, 3)
    XCTAssertEqual(Set(proposals.compactMap { $0.design.keyMaterial }).count, 3)
    for proposal in proposals {
      let data = try XCTUnwrap(proposal.design.photo)
      XCTAssertLessThanOrEqual(data.count, 512_000)
      XCTAssertEqual(Array(data.prefix(2)), [0xFF, 0xD8], "Native storage uses sanitized JPEG")
      XCTAssertNotNil(UIImage(data: data))
    }
    XCTAssertEqual(Array(ThemeProtocol.paths.prefix(2)), ["/v1/models", "/v1/chat/completions"])
    XCTAssertEqual(ThemeProtocol.paths.filter { $0 == "/v1/skins/jobs" }.count, 3)
    XCTAssertEqual(ThemeProtocol.paths.filter { $0.hasPrefix("/v1/skins/jobs/") }.count, 6)
    XCTAssertEqual(CustomKeyboardSkinStore.current, current)
    XCTAssertEqual(CustomSkinLibrary.designs, saved)
  }
  func testCorruptArtworkStopsBeforeReturningPartialPlans() async throws {
    ThemeProtocol.reset(image: Data("not an image".utf8))
    let api = client()
    do {
      _ = try await AISkinService.generate("合成主题验收", client: api,
        account: BackendAccountSession(api: api, storage: ThemeSessionStorage()))
      XCTFail("Corrupt artwork must fail generation")
    } catch { XCTAssertTrue(error.localizedDescription.contains("插画")) }
    XCTAssertTrue((1...3).contains(ThemeProtocol.paths.filter { $0 == "/v1/skins/jobs" }.count))
  }
}

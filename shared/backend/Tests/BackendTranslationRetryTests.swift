import Foundation
import XCTest
@testable import MSIMEBackend

private final class TranslationSessionStorage: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?

  init(_ value: BackendSavedSession) { self.value = value }
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
}

private final class TranslationRetryProtocol: URLProtocol {
  static let oldToken = String(repeating: "a", count: 64)
  static let newToken = String(repeating: "b", count: 64)
  private static let lock = NSLock()
  private static var attempts: [String] = []
  static var authorizations: [String] { lock.lock(); defer { lock.unlock() }; return attempts }
  static func reset() { lock.lock(); defer { lock.unlock() }; attempts = [] }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let path = request.url!.path
    let authorization = request.value(forHTTPHeaderField: "Authorization") ?? ""
    let status: Int
    let body: String
    if path == "/v1/auth/refresh" {
      status = 200
      body = "{\"access_token\":\"\(Self.newToken)\",\"refresh_token\":\"\(String(repeating: "d", count: 64))\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"synthetic-user\",\"display_name\":\"示例\",\"created_at\":\"2026-09-08\"}}"
    } else if path == "/v1/translate" {
      Self.lock.lock(); Self.attempts.append(authorization); Self.lock.unlock()
      status = authorization == "Bearer \(Self.newToken)" ? 200 : 401
      body = status == 200 ? #"{"code":200,"data":["hello"]}"# : #"{"error":{"code":"invalid_credentials"}}"#
    } else {
      status = 404
      body = #"{"error":{"code":"not_found"}}"#
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil,
                                   headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendTranslationRetryTests: XCTestCase {
  func testRejectedTranslationTokenRefreshesOnceAndRetries() async throws {
    TranslationRetryProtocol.reset()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [TranslationRetryProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let tokens = BackendAccountClient.Tokens(
      access_token: TranslationRetryProtocol.oldToken,
      refresh_token: String(repeating: "c", count: 64),
      token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-user", display_name: "示例", created_at: "2026-09-08"))
    let storage = TranslationSessionStorage(try BackendSavedSession.forTokens(tokens))
    let session = BackendAccountSession(api: client, storage: storage,
                                        refreshLock: BackendProcessRefreshLock())

    let result = try await client.translate(texts: ["你好"], target: "en", session: session)

    XCTAssertEqual(result, ["hello"])
    XCTAssertEqual(TranslationRetryProtocol.authorizations,
                   ["Bearer \(TranslationRetryProtocol.oldToken)",
                    "Bearer \(TranslationRetryProtocol.newToken)"])
  }
}

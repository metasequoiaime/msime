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

private final class ChatModelsSessionStorage: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?
  init(_ value: BackendSavedSession) { self.value = value }
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
}

private struct SharedChatTestLock: BackendRefreshLock {
  var sharedAcrossProcesses: Bool { true }
  func run<T: Sendable>(_ body: @Sendable () async throws -> T) async throws -> T { try await body() }
}

private final class ChatIdentityChangeProtocol: URLProtocol, @unchecked Sendable {
  static let oldToken = String(repeating: "a", count: 64)
  static let newToken = String(repeating: "b", count: 64)
  private static let lock = NSLock()
  private static var recorded: [String] = []
  private static var onOldRequest: (() -> Void)?
  private static var rejectOldChat = false
  static var authorizations: [String] { lock.withLock { recorded } }
  static func reset(rejectOldChat: Bool = false, onOldRequest: @escaping () -> Void) {
    lock.withLock { recorded = []; Self.rejectOldChat = rejectOldChat; Self.onOldRequest = onOldRequest }
  }
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let authorization = request.value(forHTTPHeaderField: "Authorization") ?? ""
    let (rejectOldChat, onOldRequest) = Self.lock.withLock { () -> (Bool, (() -> Void)?) in
      Self.recorded.append(authorization)
      defer { Self.onOldRequest = nil }
      return (Self.rejectOldChat, Self.onOldRequest)
    }
    let oldRequest = authorization == "Bearer \(Self.oldToken)"
    if oldRequest { onOldRequest?() }
    let chat = request.url?.path == "/v1/chat/completions"
    let rejected = chat && rejectOldChat && oldRequest
    let body: String
    if rejected { body = #"{"error":{"code":"invalid_credentials"}}"# }
    else if chat { body = #"{"choices":[{"message":{"role":"assistant","content":"合成回复"}}]}"# }
    else { body = #"{"data":[{"id":"fixture-model"}],"default_model":"fixture-model"}"# }
    let response = HTTPURLResponse(url: request.url!, statusCode: rejected ? 401 : 200, httpVersion: nil,
                                   headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class ChatModelsRetryProtocol: URLProtocol {
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
      body = "{\"access_token\":\"\(Self.newToken)\",\"refresh_token\":\"\(String(repeating: "d", count: 64))\",\"token_type\":\"Bearer\",\"expires_in\":900,\"user\":{\"id\":\"synthetic-chat-user\",\"display_name\":\"示例\",\"created_at\":\"2026-09-08\"}}"
    } else if path == "/v1/models" {
      Self.lock.lock(); Self.attempts.append(authorization); Self.lock.unlock()
      status = authorization == "Bearer \(Self.newToken)" ? 200 : 401
      body = status == 200
        ? #"{"data":[{"id":"fixture-model"}],"default_model":"fixture-model"}"#
        : #"{"error":{"code":"invalid_credentials"}}"#
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

final class BackendChatClientTests: XCTestCase {
  func testChatResponseFromPreviousAccountIsDiscarded() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ChatIdentityChangeProtocol.self]
    let api = BackendAccountClient(configuration: configuration)
    let old = BackendAccountClient.Tokens(access_token: ChatIdentityChangeProtocol.oldToken,
      refresh_token: String(repeating: "c", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-chat-user", display_name: "示例", created_at: "2026-09-08"))
    let replacement = BackendAccountClient.Tokens(access_token: ChatIdentityChangeProtocol.newToken,
      refresh_token: String(repeating: "d", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "replacement-chat-user", display_name: "新账号", created_at: "2026-09-08"))
    let storage = ChatModelsSessionStorage(try BackendSavedSession.forTokens(old))
    let replacementSession = try BackendSavedSession.forTokens(replacement)
    ChatIdentityChangeProtocol.reset(onOldRequest: { try? storage.save(replacementSession) })
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: SharedChatTestLock())

    do {
      _ = try await api.chat(messages: [.init(role: "user", content: "合成消息")],
                             model: "fixture-model", session: session,
                             matchingUserID: "synthetic-chat-user")
      XCTFail("old account's reply must be discarded after account replacement")
    } catch is CancellationError { }
    XCTAssertEqual(ChatIdentityChangeProtocol.authorizations,
                   ["Bearer \(ChatIdentityChangeProtocol.oldToken)"])
    XCTAssertEqual(try storage.load()?.tokens.user.id, "replacement-chat-user")
  }

  func testRejectedChatDoesNotRetryAsReplacementAccount() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ChatIdentityChangeProtocol.self]
    let api = BackendAccountClient(configuration: configuration)
    let old = BackendAccountClient.Tokens(access_token: ChatIdentityChangeProtocol.oldToken,
      refresh_token: String(repeating: "c", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-chat-user", display_name: "示例", created_at: "2026-09-08"))
    let replacement = BackendAccountClient.Tokens(access_token: ChatIdentityChangeProtocol.newToken,
      refresh_token: String(repeating: "d", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "replacement-chat-user", display_name: "新账号", created_at: "2026-09-08"))
    let storage = ChatModelsSessionStorage(try BackendSavedSession.forTokens(old))
    let replacementSession = try BackendSavedSession.forTokens(replacement)
    ChatIdentityChangeProtocol.reset(rejectOldChat: true,
                                     onOldRequest: { try? storage.save(replacementSession) })
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: SharedChatTestLock())

    do {
      _ = try await api.chat(messages: [.init(role: "user", content: "合成消息")],
                             model: "fixture-model", session: session,
                             matchingUserID: "synthetic-chat-user")
      XCTFail("old chat must not be retried with the replacement account's token")
    } catch is CancellationError { }
    XCTAssertEqual(ChatIdentityChangeProtocol.authorizations,
                   ["Bearer \(ChatIdentityChangeProtocol.oldToken)"])
    XCTAssertEqual(try storage.load()?.tokens.user.id, "replacement-chat-user")
  }

  func testModelsResponseFromPreviousAccountIsDiscarded() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ChatIdentityChangeProtocol.self]
    let api = BackendAccountClient(configuration: configuration)
    let old = BackendAccountClient.Tokens(access_token: ChatIdentityChangeProtocol.oldToken,
      refresh_token: String(repeating: "c", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-chat-user", display_name: "示例", created_at: "2026-09-08"))
    let replacement = BackendAccountClient.Tokens(access_token: ChatIdentityChangeProtocol.newToken,
      refresh_token: String(repeating: "d", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "replacement-chat-user", display_name: "新账号", created_at: "2026-09-08"))
    let storage = ChatModelsSessionStorage(try BackendSavedSession.forTokens(old))
    let replacementSession = try BackendSavedSession.forTokens(replacement)
    ChatIdentityChangeProtocol.reset(onOldRequest: { try? storage.save(replacementSession) })
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: SharedChatTestLock())

    do {
      _ = try await api.chatModels(session: session)
      XCTFail("old account's model catalog must be discarded after account replacement")
    } catch is CancellationError { }
    XCTAssertEqual(ChatIdentityChangeProtocol.authorizations,
                   ["Bearer \(ChatIdentityChangeProtocol.oldToken)"])
    XCTAssertEqual(try storage.load()?.tokens.user.id, "replacement-chat-user")
  }

  func testModelsRefreshRejectedSessionAndPreserveAccount() async throws {
    ChatModelsRetryProtocol.reset()
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ChatModelsRetryProtocol.self]
    let api = BackendAccountClient(configuration: configuration)
    let tokens = BackendAccountClient.Tokens(
      access_token: ChatModelsRetryProtocol.oldToken,
      refresh_token: String(repeating: "c", count: 64),
      token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-chat-user", display_name: "示例", created_at: "2026-09-08"))
    let storage = ChatModelsSessionStorage(try BackendSavedSession.forTokens(tokens))
    let session = BackendAccountSession(api: api, storage: storage,
                                        refreshLock: BackendProcessRefreshLock())

    let catalog = try await api.chatModels(session: session)

    XCTAssertEqual(catalog.default_model, "fixture-model")
    XCTAssertEqual(ChatModelsRetryProtocol.authorizations,
                   ["Bearer \(ChatModelsRetryProtocol.oldToken)",
                    "Bearer \(ChatModelsRetryProtocol.newToken)"])
    let user = try await session.user()
    XCTAssertEqual(user?.id, "synthetic-chat-user")
  }

  func testTranslationUsesUppercaseBackendLanguageCode() async throws {
    let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [TranslationProtocol.self]
    let values = try await BackendAccountClient(configuration: config).translate(
      texts: ["你好"], target: "en", token: "session")
    XCTAssertEqual(values, ["hello"])
  }

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

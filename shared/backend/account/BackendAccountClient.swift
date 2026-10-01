import Foundation

/// Shared account transport. Platform UI owns consent and Keychain persistence.
struct BackendAccountClient: Sendable {
  /// Backend bearer sessions are short-lived; reject responses that would create a practically permanent local session.
  static let maxSessionSeconds = 86_400 * 30

  struct User: Codable, Equatable, Sendable {
    let id: String
    let display_name: String
    let created_at: String
    var preferredDisplayName: String {
      display_name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        ? "水杉小鹿·" + id.prefix(6).uppercased() : display_name
    }
  }
  struct Tokens: Codable, Sendable {
    let access_token: String
    let refresh_token: String
    let token_type: String
    let expires_in: Int
    let user: User
  }
  struct Challenge: Decodable, Sendable {
    let challenge_id: String
    let expires_in: Int
    let nonce: String?
    let authorization_url: String?
  }
  struct Profile: Decodable, Sendable {
    struct Identity: Decodable, Sendable { let provider: String; let subject: String }
    let user: User
    let identities: [Identity]
  }
  struct Failure: Error, LocalizedError, Sendable {
    let status: Int
    /// The server's `error.code`, read from the body of a refused request; nil when there was none.
    var code: String? = nil

    /// The community moderation refusals, which the user has to be told apart from an outage.
    static let moderationMessages = [
      "blocked_content": "内容包含不允许发布的词语，请修改后再提交",
      "screening_unavailable": "审核服务暂时不可用，请稍后重试",
      "account_banned": "该账号已被封禁，暂时无法使用账号相关功能",
    ]
    static let moderationStatuses = ["blocked_content": 422, "screening_unavailable": 503, "account_banned": 403]

    /// The Chinese sentence for a moderation refusal, or nil when this failure is not one.
    var moderationMessage: String? {
      guard let code, Self.moderationStatuses[code] == status else { return nil }
      return Self.moderationMessages[code]
    }

    var errorDescription: String? {
      if let moderationMessage { return moderationMessage }
      switch status {
      case 400: return "请求内容无效或超出大小限制，请检查后重试。"
      case 401: return "登录已失效，请重新登录。"
      case 403: return "此操作需要重新登录或开启相应权限。"
      case 409: return "内容已在其他设备更新，请刷新后重试。"
      case 429: return "操作过于频繁，请稍后再试。"
      case 503: return "此服务暂不可用，请稍后再试。"
      default: return "请求未完成，请稍后重试。"
      }
    }
  }
  private final class Redirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask,
                    willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest,
                    completionHandler: @escaping (URLRequest?) -> Void) {
      completionHandler(nil)
    }
  }
  private let session: URLSession
  private let origin = URL(string: "https://api.msime.app")!

  init(configuration: URLSessionConfiguration = .ephemeral) {
    let configuration = configuration.copy() as! URLSessionConfiguration
    configuration.httpCookieStorage = nil
    configuration.urlCache = nil
    configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
    session = URLSession(configuration: configuration, delegate: Redirects(), delegateQueue: nil)
  }

  func providers() async throws -> [String: Bool] {
    struct Response: Decodable { let providers: [String: Bool] }
    let response: Response = try await json("GET", "/v1/auth/providers")
    return response.providers
  }
  func challenge(provider: String, target: String = "", linkToken: String? = nil) async throws -> Challenge {
    struct Body: Encodable { let provider: String; let target: String; let purpose: String }
    let value: Challenge = try await json("POST", "/v1/auth/challenges", token: linkToken,
                                          body: JSONEncoder().encode(Body(provider: provider, target: target,
                                                                        purpose: linkToken == nil ? "login" : "link")))
    guard Self.validSingleLine(value.challenge_id, maximumBytes: 256, empty: false), value.expires_in > 0,
          value.nonce.map({ Self.validSingleLine($0, maximumBytes: 4096) }) ?? true,
          value.authorization_url.map({ Self.validSingleLine($0, maximumBytes: 4096) }) ?? true
    else { throw Failure(status: 0) }
    return value
  }
  func login(challenge: String, credential: String, linkToken: String? = nil) async throws -> Tokens {
    struct Body: Encodable { let challenge_id: String; let credential: String }
    let tokens: Tokens = try await json("POST", "/v1/auth/login", token: linkToken,
      body: JSONEncoder().encode(Body(challenge_id: challenge, credential: credential)))
    return try validated(tokens)
  }
  func refresh(_ token: String) async throws -> Tokens {
    struct Body: Encodable { let refresh_token: String }
    let tokens: Tokens = try await json("POST", "/v1/auth/refresh",
      body: JSONEncoder().encode(Body(refresh_token: token)))
    return try validated(tokens)
  }
  func profile(token: String) async throws -> Profile {
    let value: Profile = try await json("GET", "/v1/users/me", token: token)
    guard Self.validUser(value.user), value.identities.count <= 16,
          value.identities.allSatisfy({ identity in
            !identity.provider.isEmpty && identity.provider.utf8.count <= 32
              && identity.provider.utf8.allSatisfy({ (97...122).contains($0) || $0 == 95 || $0 == 45 })
              && Self.validSingleLine(identity.subject, maximumBytes: 512)
          }) else { throw Failure(status: 0) }
    return value
  }
  func rename(_ name: String, token: String) async throws {
    struct Body: Encodable { let display_name: String }
    _ = try await request("PATCH", "/v1/users/me", token: token,
                         body: JSONEncoder().encode(Body(display_name: name)))
  }
  func logout(token: String, all: Bool = false) async throws {
    struct Body: Encodable { let all: Bool }
    _ = try await request("POST", "/v1/auth/logout", token: token,
                          body: JSONEncoder().encode(Body(all: all)))
  }
  func deleteAccount(token: String) async throws {
    _ = try await request("DELETE", "/v1/users/me", token: token)
  }

  // URLComponents leaves '+' bare, which the server decodes as a space; only a value can hold one.
  static func encodedPath(_ components: URLComponents) -> String? {
    var components = components
    components.percentEncodedQuery = components.percentEncodedQuery?.replacingOccurrences(of: "+", with: "%2B")
    return components.string
  }

  // Used by the explicit user-data screens as well as account operations. No redirects,
  // cookies, cached private data, arbitrary origins, or server error text are exposed.
  func request(_ method: String, _ path: String, token: String? = nil,
               body: Data? = nil, timeout: TimeInterval = 30, maximumResponseBytes: Int = 1024 * 1024) async throws -> Data {
    guard (1...48 * 1024 * 1024).contains(maximumResponseBytes) else { throw Failure(status: 0) }
    let request = try makeRequest(method, path, token: token, body: body, timeout: timeout)
    let (bytes, response) = try await session.bytes(for: request)
    guard let response = response as? HTTPURLResponse else { throw Failure(status: 0) }
    guard (200..<300).contains(response.statusCode) else {
      throw Failure(status: response.statusCode, code: try? await Self.errorCode(bytes))
    }
    guard response.expectedContentLength <= maximumResponseBytes else { throw Failure(status: 0) }
    var data = Data()
    for try await byte in bytes {
      guard data.count < maximumResponseBytes else { throw Failure(status: 0) }
      data.append(byte)
    }
    try Task.checkCancellation()
    return data
  }
  /// `error.code` of a refusal body `{"error":{"code":...}}`, reading at most 4 KiB. The server's message text is never shown.
  static func errorCode(_ bytes: URLSession.AsyncBytes) async throws -> String? {
    var data = Data()
    for try await byte in bytes {
      guard data.count < 4096 else { return nil }
      data.append(byte)
    }
    return errorCode(data)
  }

  static func errorCode(_ data: Data) -> String? {
    guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let error = root["error"] as? [String: Any],
          let code = error["code"] as? String,
          (1...64).contains(code.utf8.count),
          code.utf8.allSatisfy({ (97...122).contains($0) || (48...57).contains($0) || $0 == 95 })
    else { return nil }
    return code
  }

  private func makeRequest(_ method: String, _ path: String, token: String?, body: Data?, timeout: TimeInterval = 30) throws -> URLRequest {
    guard path.hasPrefix("/v1/"), !path.contains("\\"),
          let url = URL(string: path, relativeTo: origin)?.absoluteURL,
          url.scheme == "https", url.host == origin.host, url.port == nil,
          url.user == nil, url.password == nil, url.fragment == nil,
          body == nil || body!.count <= 1024 * 1024,
          token == nil || (!token!.isEmpty && !token!.contains(where: { $0.isWhitespace || $0.isNewline }))
    else { throw Failure(status: 0) }
    var request = URLRequest(url: url)
    request.httpMethod = method
    request.timeoutInterval = timeout
    request.httpBody = body
    request.setValue("MSIME/Apple", forHTTPHeaderField: "User-Agent")
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    if body != nil { request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
    if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
    return request
  }

  // Export directly to a private temporary file. Keep the ordinary JSON transport's
  // smaller bound; dictionary files may legitimately contain 100,000 entries.
  func download(_ path: String, token: String, filename: String, maximumBytes: Int, mediaType: String = "text/plain") async throws -> URL {
    guard maximumBytes > 0, maximumBytes <= 512 * 1024 * 1024,
          ["text/plain", "application/x-ndjson"].contains(mediaType),
          !filename.isEmpty, filename == URL(fileURLWithPath: filename).lastPathComponent else { throw Failure(status: 400) }
    var request = try makeRequest("GET", path, token: token, body: nil)
    request.timeoutInterval = 600
    request.setValue(mediaType, forHTTPHeaderField: "Accept")
    request.setValue("identity", forHTTPHeaderField: "Accept-Encoding")
    let (bytes, response) = try await session.bytes(for: request)
    guard let response = response as? HTTPURLResponse else { throw Failure(status: 0) }
    guard response.statusCode == 200 else { throw Failure(status: response.statusCode) }
    guard response.expectedContentLength <= maximumBytes,
          response.mimeType == mediaType else { throw Failure(status: 0) }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-export-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    var complete = false
    defer { if !complete { try? FileManager.default.removeItem(at: directory) } }
    let file = directory.appendingPathComponent(filename)
    var attributes: [FileAttributeKey: Any] = [.posixPermissions: 0o600]
    #if os(iOS)
    attributes[.protectionKey] = FileProtectionType.completeUntilFirstUserAuthentication
    #endif
    guard FileManager.default.createFile(atPath: file.path, contents: nil, attributes: attributes) else { throw Failure(status: 0) }
    let handle = try FileHandle(forWritingTo: file)
    defer { try? handle.close() }
    var buffer = Data(); buffer.reserveCapacity(65536)
    var count = 0
    for try await byte in bytes {
      guard count < maximumBytes else { throw Failure(status: 0) }
      buffer.append(byte); count += 1
      if buffer.count == 65536 {
        try Task.checkCancellation()
        try handle.write(contentsOf: buffer); buffer.removeAll(keepingCapacity: true)
      }
    }
    try Task.checkCancellation()
    if response.expectedContentLength >= 0 && response.expectedContentLength != count { throw Failure(status: 0) }
    try handle.write(contentsOf: buffer)
    try handle.synchronize()
    complete = true
    return file
  }
  // The caller owns a validated private copy for the entire request lifetime.
  // Stream both the large request and bounded JSON response; do not retry a
  // replacement automatically after an ambiguous network failure.
  func uploadSnapshot(_ file: URL, revision: Int64, token: String) async throws -> Data {
    guard revision >= 0, file.isFileURL,
          let stream = InputStream(url: file),
          let size = try FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber,
          size.int64Value > 0, size.int64Value <= 512 * 1024 * 1024 else { throw Failure(status: 400) }
    var request = try makeRequest("PUT", "/v1/users/me/dictionary/snapshot?revision=\(revision)", token: token, body: nil)
    request.timeoutInterval = 130
    request.setValue("application/x-ndjson", forHTTPHeaderField: "Content-Type")
    request.setValue(size.stringValue, forHTTPHeaderField: "Content-Length")
    request.httpBodyStream = stream
    let (bytes, response) = try await session.bytes(for: request)
    guard let response = response as? HTTPURLResponse else { throw Failure(status: 0) }
    guard response.statusCode == 200 else { throw Failure(status: response.statusCode) }
    guard response.mimeType == "application/json", response.expectedContentLength <= 1024 * 1024 else { throw Failure(status: 0) }
    var result = Data()
    for try await byte in bytes {
      guard result.count < 1024 * 1024 else { throw Failure(status: 0) }
      result.append(byte)
    }
    try Task.checkCancellation()
    return result
  }

  func json<T: Decodable>(_ method: String, _ path: String, token: String? = nil,
                                  body: Data? = nil, timeout: TimeInterval = 30, maximumResponseBytes: Int = 1024 * 1024) async throws -> T {
    let data = try await request(method, path, token: token, body: body, timeout: timeout, maximumResponseBytes: maximumResponseBytes)
    do { return try JSONDecoder().decode(T.self, from: data) }
    catch { throw Failure(status: 0) }
  }
  static func validate(_ tokens: Tokens) throws {
    let hex = CharacterSet(charactersIn: "0123456789abcdef")
    guard tokens.token_type == "Bearer", (1...maxSessionSeconds).contains(tokens.expires_in),
          validUser(tokens.user),
          [tokens.access_token, tokens.refresh_token].allSatisfy({ token in
            token.utf8.count == 64 && token.unicodeScalars.allSatisfy(hex.contains)
          }) else { throw Failure(status: 0) }
  }
  private static func validUser(_ user: User) -> Bool {
    validSingleLine(user.id, maximumBytes: 256, empty: false)
      && user.display_name.unicodeScalars.count <= 64
      && !user.display_name.unicodeScalars.contains { $0.properties.generalCategory == .control }
      && validSingleLine(user.created_at, maximumBytes: 128)
  }
  private static func validSingleLine(_ value: String, maximumBytes: Int, empty: Bool = true) -> Bool {
    (empty || !value.isEmpty) && value.utf8.count <= maximumBytes
      && !value.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }
  private func validated(_ tokens: Tokens) throws -> Tokens {
    try Self.validate(tokens)
    return tokens
  }
}

import Foundation

/// iOS App 和键盘扩展所属的产品版本（edition），以及两者共用的 App Group。
///
/// App Group 标识只写在这里一处，其他代码一律引用 `appGroupIdentifier`：`UserDefaults(suiteName:) ?? .standard` 和 `containerURL(...)` 在标识不对时不报错，漏改的那一处会悄悄读写另一份数据。放在这个文件里，是因为 iOS App、键盘扩展、各测试目标和 Tauri 公共组件的 iOS 工程都编译它。
///
/// 版本身份写在 App 和键盘扩展各自的 Info.plist 里，键名与 macOS 相同（platforms/macos/src/core/EditionIdentity.h）：`MSIMEEdition` 是版本 id，`MSIMEInputSchemes` 是本版本提供的方案，`MSIMEDefaultScheme` 是回退方案，`MSIMEWubiMixedPinyinDefault` 是五笔混拼的默认值。full 不带这些键，没有 `MSIMEEdition` 就是 full，每个值都取引入版本之前的那个；测试进程同样读到 full。其他版本的 App Group 是 full 的标识加 `.<版本 id>`，两个版本同时装在一台设备上也不会读写对方的数据。只有 iOS 读 Info.plist，macOS 等平台编译这个文件时始终是 full，它们的版本身份另有来源。
enum MSIMEAppEdition {
  static let fullIdentifier = "full"
  static let fullAppGroupIdentifier = "group.app.msime.ios"
  static let fullDefaultScheme = "quanpin"
  static let fullURLScheme = "msime"

  #if os(iOS)
  private static var info: [String: Any] { Bundle.main.infoDictionary ?? [:] }
  #else
  private static var info: [String: Any] { [:] }
  #endif

  /// 本进程的版本 id。
  static let identifier = identifier(in: info)
  /// App 与键盘扩展共用的 App Group，也是两者共用的钥匙串访问组。
  static let appGroupIdentifier = appGroupIdentifier(in: info)
  /// 键盘扩展拉起本版本 App 用的 URL scheme，App 在 Info.plist 的 `CFBundleURLTypes` 里注册它。多个 App 声明同一个自定义 scheme 时由系统任选一个打开，所以每个版本各用一个：full 是 `msime`，其他版本是 `msime-<版本 id>`。
  static let urlScheme = urlScheme(in: info)
  /// 本版本提供的方案（版本表里的方案名）；nil 表示 full，即全部方案。
  static let inputSchemes = inputSchemes(in: info)
  /// 本版本的默认方案，也是偏好里的方案本版本没有时的回退值。
  static let defaultScheme = defaultScheme(in: info)
  /// 五笔混拼开关没被用户动过时的值：full 是关，五笔版是开（版本表的 `preference_defaults`）。
  static let wubiMixedPinyinDefault = wubiMixedPinyinDefault(in: info)

  static var isFull: Bool { identifier == fullIdentifier }

  /// 本版本是否提供这个方案（`quanpin`、`wubi` 等偏好取值）。
  static func offers(_ scheme: String) -> Bool { inputSchemes?.contains(scheme) ?? true }

  static func identifier(in info: [String: Any]) -> String {
    guard let value = info["MSIMEEdition"] as? String, !value.isEmpty else { return fullIdentifier }
    return value
  }

  static func appGroupIdentifier(in info: [String: Any]) -> String {
    let edition = identifier(in: info)
    return edition == fullIdentifier ? fullAppGroupIdentifier : "\(fullAppGroupIdentifier).\(edition)"
  }

  static func urlScheme(in info: [String: Any]) -> String {
    let edition = identifier(in: info)
    return edition == fullIdentifier ? fullURLScheme : "\(fullURLScheme)-\(edition)"
  }

  static func inputSchemes(in info: [String: Any]) -> [String]? {
    guard identifier(in: info) != fullIdentifier,
          let schemes = info["MSIMEInputSchemes"] as? [String], !schemes.isEmpty else { return nil }
    return schemes
  }

  static func wubiMixedPinyinDefault(in info: [String: Any]) -> Bool {
    guard identifier(in: info) != fullIdentifier else { return false }
    return info["MSIMEWubiMixedPinyinDefault"] as? Bool ?? false
  }

  /// 声明的默认方案不在本版本的方案里时取第一个方案，保证回退到的方案本版本一定能跑。
  static func defaultScheme(in info: [String: Any]) -> String {
    guard let schemes = inputSchemes(in: info) else { return fullDefaultScheme }
    if let declared = info["MSIMEDefaultScheme"] as? String, schemes.contains(declared) { return declared }
    return schemes[0]
  }
}

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
    guard Self.validProviders(response.providers) else { throw Failure(status: 0) }
    return response.providers
  }
  func challenge(provider: String, target: String = "", linkToken: String? = nil) async throws -> Challenge {
    struct Body: Encodable { let provider: String; let target: String; let purpose: String }
    guard Self.validProviderTarget(provider, target: target) else { throw Failure(status: 400) }
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
    guard Self.validLoginRequest(challenge: challenge, credential: credential) else { throw Failure(status: 400) }
    let tokens: Tokens = try await json("POST", "/v1/auth/login", token: linkToken,
      body: JSONEncoder().encode(Body(challenge_id: challenge, credential: credential)))
    return try validated(tokens)
  }
  func refresh(_ token: String) async throws -> Tokens {
    struct Body: Encodable { let refresh_token: String }
    guard Self.validLowerHexToken(token) else { throw Failure(status: 400) }
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
    guard Self.validDisplayName(name) else { throw Failure(status: 400) }
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
          !Self.containsDotSegment(path),
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

  private static func containsDotSegment(_ path: String) -> Bool {
    guard let decoded = path.removingPercentEncoding else { return true }
    return decoded.split(separator: "/", omittingEmptySubsequences: false).contains { $0 == "." || $0 == ".." }
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
  private static func validProviders(_ providers: [String: Bool]) -> Bool {
    providers.count <= 16 && providers.keys.allSatisfy { key in
      !key.isEmpty && key.utf8.count <= 32
        && key.utf8.allSatisfy { (97...122).contains($0) || $0 == 95 || $0 == 45 }
    }
  }
  private static func validProviderTarget(_ provider: String, target: String) -> Bool {
    switch provider {
    case "apple":
      return target.isEmpty
    case "google":
      let prefix = target.hasPrefix("http://127.0.0.1:") ? "http://127.0.0.1:" :
        (target.hasPrefix("http://[::1]:") ? "http://[::1]:" : nil)
      guard let prefix, target.hasSuffix("/callback") else { return false }
      let portText = String(target.dropFirst(prefix.count).dropLast("/callback".count))
      guard (1...5).contains(portText.utf8.count), !portText.hasPrefix("0"),
        portText.utf8.allSatisfy({ (48...57).contains($0) }),
        let port = Int(portText), (1024...65535).contains(port) else { return false }
      return true
    case "anonymous":
      let prefix = "msime-"
      guard target.hasPrefix(prefix), target.dropFirst(prefix.count).count == 16 else { return false }
      return target.dropFirst(prefix.count).utf8.allSatisfy {
        (97...122).contains($0) || (48...57).contains($0)
      }
    case "email", "phone":
      return !target.isEmpty && target.utf8.count <= 320
        && target == target.trimmingCharacters(in: .whitespacesAndNewlines)
        && validSingleLine(target, maximumBytes: 320, empty: false)
    default:
      return false
    }
  }
  private static func validLoginRequest(challenge: String, credential: String) -> Bool {
    guard validSingleLine(challenge, maximumBytes: 256, empty: false) else { return false }
    let sixDigitCode = credential.utf8.count == 6 && credential.utf8.allSatisfy { (48...57).contains($0) }
    let appleToken = validSingleLine(credential, maximumBytes: 16 * 1024, empty: false)
    let googleCode = credential.utf8.count <= 2048 && !credential.isEmpty
      && credential.utf8.allSatisfy { (33...126).contains($0) }
    return sixDigitCode || appleToken || googleCode
  }
  private static func validLowerHexToken(_ value: String) -> Bool {
    value.utf8.count == 64 && value.utf8.allSatisfy {
      (48...57).contains($0) || (97...102).contains($0)
    }
  }
  private static func validDisplayName(_ value: String) -> Bool {
    !value.isEmpty && value == value.trimmingCharacters(in: .whitespacesAndNewlines)
      && value.unicodeScalars.count <= 64
      && !value.unicodeScalars.contains { $0.properties.generalCategory == .control }
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

/// 存储路径的符号链接策略，防止被人放进去的链接把客户端的写入重定向到别处。它是 `crates/path-trust/src/lib.rs` 中 `SYSTEM_ALIASES` 和 `reject_symlinked_components` 在 macOS 与 iOS 上的对应实现，必须与之保持一致：逐层检查路径时拒绝所有符号链接，只有至多一个受信任的系统别名例外，而且最后一级永远不能是链接。
///
/// 放在这个文件里，是因为它是 `shared/backend` 中唯一一个所有使用方（Swift package、所有 iOS target、macOS 的 CMake target 和 Tauri 的 Apple 工程）都会编译的源文件，这样 iOS `SharedUI` 的各个 store 和后端 store 可以共用它，而不必在每份构建清单里登记新文件。
enum SafePath {
  /// 存储路径可以经过的系统链接，以及每条链接唯一受信任的目标。macOS 和 iOS 上 `/tmp`、`/var` 是指向 `/private` 的链接，临时目录以及真机上的应用容器和 App Group 容器都在它们下面。必须与 `crates/path-trust/src/lib.rs` 里的 `SYSTEM_ALIASES` 保持一致。
  static let systemAliases: [(alias: String, target: String)] = [("/tmp", "/private/tmp"), ("/var", "/private/var")]

  /// 判断 `path` 是否是系统别名之一，并且从它读出的 `target`（可能是相对路径）相对链接所在目录按字面解析后，正好是该别名唯一受信任的指向。光凭名字不能证明链接归系统所有，所以目标也要核对。
  static func isTrustedSystemAliasTarget(_ path: String, target: String) -> Bool {
    guard let expected = systemAliases.first(where: { $0.alias == path })?.target else { return false }
    let parent = (path as NSString).deletingLastPathComponent
    let joined = target.hasPrefix("/") ? target : (parent.isEmpty ? "/" : parent) + "/" + target
    return normalizedLexically(joined) == expected
  }

  /// 判断 `path` 是否是系统放置的符号链接：属于系统别名之一，且链接目标与预期一致。
  static func isTrustedSystemAlias(_ path: String) -> Bool {
    guard let target = try? FileManager.default.destinationOfSymbolicLink(atPath: path) else { return false }
    return isTrustedSystemAliasTarget(path, target: target)
  }

  /// 判断 `url` 本身或其上层是否有应拒绝的符号链接：任何一级上的链接（包括最后一级）都算，唯一的例外是最后一级之上至多一个受信任的系统别名。不存在的层级可以接受，调用方正要创建它；其它 `lstat` 失败一律视为拒绝。
  static func hasRefusedSymbolicLink(_ url: URL) -> Bool {
    let components = url.standardizedFileURL.pathComponents
    guard components.first == "/" else { return true }
    var current = ""
    var sawSystemAlias = false
    for (index, component) in components.enumerated().dropFirst() {
      current += "/" + component
      var status = stat()
      if lstat(current, &status) != 0 {
        if errno == ENOENT { continue }
        return true
      }
      guard status.st_mode & S_IFMT == S_IFLNK else { continue }
      if index == components.count - 1 || sawSystemAlias || !isTrustedSystemAlias(current) { return true }
      sawSystemAlias = true
    }
    return false
  }

  private static func normalizedLexically(_ path: String) -> String {
    var parts: [Substring] = []
    for part in path.split(separator: "/") {
      if part == "." { continue }
      if part == ".." { _ = parts.popLast(); continue }
      parts.append(part)
    }
    return "/" + parts.joined(separator: "/")
  }
}

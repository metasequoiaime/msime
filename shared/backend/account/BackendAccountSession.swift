import Foundation
import Security

protocol BackendSessionAPI: Sendable {
  func login(challenge: String, credential: String, linkToken: String?) async throws -> BackendAccountClient.Tokens
  func refresh(_ token: String) async throws -> BackendAccountClient.Tokens
  func logout(token: String, all: Bool) async throws
}
extension BackendAccountClient: BackendSessionAPI {}

struct BackendSavedSession: Codable, Sendable {
  let tokens: BackendAccountClient.Tokens
  let expiresAt: Date

  static func validated(_ session: BackendSavedSession, now: Date = Date()) throws -> BackendSavedSession {
    try BackendAccountClient.validate(session.tokens)
    guard session.expiresAt.timeIntervalSinceReferenceDate.isFinite,
          session.expiresAt <= now.addingTimeInterval(TimeInterval(BackendAccountClient.maxSessionSeconds)) else {
      throw BackendAccountClient.Failure(status: 0)
    }
    return session
  }

  static func forTokens(_ tokens: BackendAccountClient.Tokens, now: Date = Date()) throws -> BackendSavedSession {
    try BackendAccountClient.validate(tokens)
    return try validated(.init(tokens: tokens,
      expiresAt: now.addingTimeInterval(TimeInterval(tokens.expires_in))), now: now)
  }
}
protocol BackendSessionStorage: Sendable {
  func load() throws -> BackendSavedSession?
  func save(_ session: BackendSavedSession) throws
  func clear() throws
}
struct BackendKeychain: BackendSessionStorage {
  private var query: [String: Any] {
    [kSecClass as String: kSecClassGenericPassword,
     kSecAttrService as String: "app.msime.backend.account",
     kSecAttrAccount as String: "https://api.msime.app"]
  }
  func load() throws -> BackendSavedSession? {
    var query = query
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return try migrateCommunitySession() }
    // A keychain we cannot read is not a session we have. An unsigned simulator build answers
    // -34018 (errSecMissingEntitlement) here, and a device can answer errSecInteractionNotAllowed
    // while locked; treating either as a hard failure took every screen that asks "am I signed in"
    // down with it, and the only thing the user saw was the status-0 fallback, 请求未完成. Absent
    // and unreadable are the same answer to that question. Data we *can* read but cannot decode is
    // a real fault and still throws.
    guard status == errSecSuccess, let data = result as? Data else { return nil }
    do { return try BackendSavedSession.validated(JSONDecoder().decode(BackendSavedSession.self, from: data)) }
    catch { throw BackendAccountClient.Failure(status: 0) }
  }
  private func migrateCommunitySession() throws -> BackendSavedSession? {
    let legacy: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "app.msime.ios.community", kSecAttrAccount as String: "api.msime.app"]
    var lookup = legacy
    lookup[kSecReturnData as String] = true
    var result: CFTypeRef?
    let status = SecItemCopyMatching(lookup as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
    // Same reasoning as above: an unreadable legacy item is nothing to migrate, not an error.
    guard status == errSecSuccess, let data = result as? Data else { return nil }
    struct Legacy: Decodable {
      struct User: Decodable { let id: String; let display_name: String; let created_at: String? }
      let access_token: String; let refresh_token: String; let user: User
      let saved_at: Date?; let expires_in: Int?
    }
    let old = try JSONDecoder().decode(Legacy.self, from: data)
    let seconds = old.expires_in ?? 900
    let tokens = BackendAccountClient.Tokens(access_token: old.access_token, refresh_token: old.refresh_token,
      token_type: "Bearer", expires_in: seconds,
      user: .init(id: old.user.id, display_name: old.user.display_name, created_at: old.user.created_at ?? ""))
    let value = try BackendSavedSession.validated(.init(tokens: tokens,
      expiresAt: (old.saved_at ?? .distantPast).addingTimeInterval(TimeInterval(seconds))))
    try save(value)
    SecItemDelete(legacy as CFDictionary)
    return value
  }
  func save(_ session: BackendSavedSession) throws {
    let validatedSession = try BackendSavedSession.validated(session)
    let data = try JSONEncoder().encode(validatedSession)
    let attributes: [String: Any] = [kSecValueData as String: data,
      kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
    var status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
    if status == errSecItemNotFound {
      status = SecItemAdd(query.merging(attributes) { _, new in new } as CFDictionary, nil)
    }
    guard status == errSecSuccess else { throw BackendAccountClient.Failure(status: 0) }
    try clearLegacy()
  }
  private func clearLegacy() throws {
    let status = SecItemDelete([kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "app.msime.ios.community", kSecAttrAccount as String: "api.msime.app"] as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else { throw BackendAccountClient.Failure(status: 0) }
  }
  func clear() throws {
    try clearLegacy()
    let status = SecItemDelete(query as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else { throw BackendAccountClient.Failure(status: 0) }
  }
}

/// Serializes rotating refresh tokens. A late login/refresh may never undo logout.
actor BackendAccountSession {
  static let shared = BackendAccountSession()
  private let api: any BackendSessionAPI
  private let storage: any BackendSessionStorage
  private var saved: BackendSavedSession?
  private var loaded = false
  private var generation = 0
  private var refreshing: Task<String, Error>?

  init(api: any BackendSessionAPI = BackendAccountClient(), storage: any BackendSessionStorage = BackendKeychain()) {
    self.api = api; self.storage = storage
  }
  func user() throws -> BackendAccountClient.User? {
    try load()
    return saved?.tokens.user
  }
  private func load() throws {
    if !loaded {
      saved = try storage.load().map { try BackendSavedSession.validated($0) }
      loaded = true
    }
  }
  func signIn(challenge: String, credential: String) async throws {
    generation += 1
    refreshing?.cancel(); refreshing = nil
    let version = generation
    let tokens = try await api.login(challenge: challenge, credential: credential, linkToken: nil)
    try Task.checkCancellation()
    guard version == generation else { throw CancellationError() }
    try install(tokens)
  }
  private func install(_ tokens: BackendAccountClient.Tokens) throws {
    let value = try BackendSavedSession.forTokens(tokens)
    try storage.save(value)
    saved = value; loaded = true
  }
  func accessToken(retrying rejectedToken: String? = nil) async throws -> String {
    try load()
    // A refresh may have been started because another caller received a 401
    // for the still-unexpired access token. Every concurrent caller must join
    // it before returning that rejected token from the fast path.
    if let refreshing { return try await refreshing.value }
    if let current = saved, current.expiresAt.timeIntervalSinceNow > 30 && rejectedToken != current.tokens.access_token {
      return current.tokens.access_token
    }
    // Other actors and processes share this storage and may already have rotated (or created) the session.
    if let stored = try? storage.load().map({ try BackendSavedSession.validated($0) }),
       stored.tokens.refresh_token != saved?.tokens.refresh_token { saved = stored }
    guard let current = saved else { throw BackendAccountClient.Failure(status: 401) }
    if current.expiresAt.timeIntervalSinceNow > 30 && rejectedToken != current.tokens.access_token { return current.tokens.access_token }
    let refreshToken = current.tokens.refresh_token
    let version = generation
    let task = Task<String, Error> {
      let tokens = try await self.api.refresh(refreshToken)
      guard self.generation == version else { throw CancellationError() }
      try self.install(tokens)
      return tokens.access_token
    }
    refreshing = task
    defer { if version == generation { refreshing = nil } }
    do { return try await task.value }
    catch {
      if version == generation, let failure = error as? BackendAccountClient.Failure, failure.status == 401 {
        // Clear only the session that was rejected; a rotation saved meanwhile elsewhere is adopted.
        // A nil read may be an unreadable keychain rather than an absent item, so it is not cleared.
        let stored = try? storage.load().map({ try BackendSavedSession.validated($0) })
        if let stored, stored.tokens.refresh_token != refreshToken {
          saved = stored
          if stored.expiresAt.timeIntervalSinceNow > 30 && rejectedToken != stored.tokens.access_token { return stored.tokens.access_token }
        } else {
          if stored != nil { try storage.clear() }
          saved = nil
        }
      }
      throw error
    }
  }
  func updateUser(_ user: BackendAccountClient.User, matching token: String) throws {
    try load()
    guard let current = saved, current.tokens.access_token == token, current.tokens.user.id == user.id else {
      throw CancellationError()
    }
    let old = current.tokens
    let tokens = BackendAccountClient.Tokens(access_token: old.access_token, refresh_token: old.refresh_token,
      token_type: old.token_type, expires_in: old.expires_in, user: user)
    let value = try BackendSavedSession.validated(.init(tokens: tokens, expiresAt: current.expiresAt))
    try storage.save(value); saved = value
  }
  func credentials(retrying rejectedToken: String? = nil, matchingUserID expected: String? = nil) async throws -> (userID: String, token: String) {
    try load()
    if let expected, saved?.tokens.user.id != expected { throw CancellationError() }
    let token = try await accessToken(retrying: rejectedToken)
    guard let saved, saved.tokens.access_token == token,
          expected == nil || saved.tokens.user.id == expected else { throw CancellationError() }
    return (saved.tokens.user.id, token)
  }
  func forget() throws {
    generation += 1
    refreshing?.cancel(); refreshing = nil
    saved = nil; loaded = true
    try storage.clear()
  }
  func logout(all: Bool = false) async throws {
    let token: String
    do { token = try await accessToken() }
    catch { try forget(); throw error }
    try forget()
    try await api.logout(token: token, all: all)
  }
}

import Foundation
import Security
#if os(macOS)
import Darwin
#endif

protocol BackendSessionAPI: Sendable {
  func login(challenge: String, credential: String, linkToken: String?) async throws -> BackendAccountClient.Tokens
  func refresh(_ token: String) async throws -> BackendAccountClient.Tokens
  func logout(token: String, all: Bool) async throws
}
extension BackendAccountClient: BackendSessionAPI {}

// 在任何操作创建或打开共享账号文件之前，检查每一级已存在的目录：除了 `SafePath` 放行的受信任系统别名外不能有符号链接，并且每一级已存在的路径都必须是目录。
private func backendDirectoryPathIsSafe(_ url: URL) -> Bool {
  guard url.isFileURL, url.path.hasPrefix("/"), !SafePath.hasRefusedSymbolicLink(url) else { return false }
  var current = URL(fileURLWithPath: "/")
  for component in url.standardizedFileURL.pathComponents.dropFirst() {
    current.appendPathComponent(component, isDirectory: true)
    var info = stat()
    if lstat(current.path, &info) != 0 {
      if errno == ENOENT { continue }
      return false
    }
    // 走到这里还可能出现的链接，只有 `SafePath` 已经信任的系统别名。
    let type = info.st_mode & S_IFMT
    guard type == S_IFLNK || type == S_IFDIR else { return false }
  }
  return true
}

struct BackendSavedSession: Codable, Sendable {
  let tokens: BackendAccountClient.Tokens
  let expiresAt: Date
  /// 令牌刷新时保持不变；同一用户重新登录也会取得新值。
  let sessionID: UUID?

  init(tokens: BackendAccountClient.Tokens, expiresAt: Date, sessionID: UUID? = UUID()) {
    self.tokens = tokens; self.expiresAt = expiresAt; self.sessionID = sessionID
  }

  static func validated(_ session: BackendSavedSession, now: Date = Date()) throws -> BackendSavedSession {
    try BackendAccountClient.validate(session.tokens)
    guard session.expiresAt.timeIntervalSinceReferenceDate.isFinite,
          session.expiresAt <= now.addingTimeInterval(TimeInterval(BackendAccountClient.maxSessionSeconds)) else {
      throw BackendAccountClient.Failure(status: 0)
    }
    return session
  }

  static func forTokens(_ tokens: BackendAccountClient.Tokens, now: Date = Date(),
                        sessionID: UUID? = UUID()) throws -> BackendSavedSession {
    try BackendAccountClient.validate(tokens)
    return try validated(.init(tokens: tokens,
      expiresAt: now.addingTimeInterval(TimeInterval(tokens.expires_in)), sessionID: sessionID), now: now)
  }
}
protocol BackendSessionStorage: Sendable {
  func load() throws -> BackendSavedSession?
  func save(_ session: BackendSavedSession) throws
  func clear() throws
}
struct BackendKeychain: BackendSessionStorage {
  /// On iOS the session lives in the App Group's keychain access group, which the app and the keyboard extension both already hold as an entitlement, so the keyboard can reach the signed-in account (cloud clipboard) without the token ever being written to a file. Other platforms keep the item in the process's default access group.
  #if os(iOS)
  static let defaultAccessGroup: String? = MSIMEAppEdition.appGroupIdentifier
  #else
  static let defaultAccessGroup: String? = nil
  #endif
  private let accessGroup: String?
  private let service: String

  init(accessGroup: String? = BackendKeychain.defaultAccessGroup, service: String = "app.msime.backend.account") {
    self.accessGroup = accessGroup
    self.service = service
  }
  /// Matches the item in every access group the process holds, which is what clearing needs.
  private var anyGroupQuery: [String: Any] {
    [kSecClass as String: kSecClassGenericPassword,
     kSecAttrService as String: service,
     kSecAttrAccount as String: "https://api.msime.app"]
  }
  private var query: [String: Any] {
    var query = anyGroupQuery
    if let accessGroup { query[kSecAttrAccessGroup as String] = accessGroup }
    return query
  }
  func load() throws -> BackendSavedSession? {
    var query = query
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
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
  }
  func clear() throws {
    let status = SecItemDelete(anyGroupQuery as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else { throw BackendAccountClient.Failure(status: 0) }
  }
}

#if os(macOS)
/// The macOS account session: `account-session.json` in the input method's Application Support directory, read and written by this process and by the settings app (`FileAccountSessionStorage` in `crates/client-core`). Both take `account-refresh.lock` beside it around every write, so neither refreshes from a refresh token the other has already spent; the server revokes the session when one comes back. Owner-only, and never followed through a symlink.
struct BackendDesktopSessionFile: BackendSessionStorage {
  static let fileName = "account-session.json"
  static let maximumBytes = 64 * 1024
  /// 状态目录随版本而变：输入法的 Info.plist 声明了版本（`MSIMEEdition`）时取它的 `MSIMESettingsBundleIdentifier`，否则是 full 的 `app.msime.macos`。与 platforms/macos/src/core/EditionIdentity.h 一致。
  static var standardDirectory: URL? {
    let edition = Bundle.main.object(forInfoDictionaryKey: "MSIMEEdition") as? String
    let declared = edition.map { !$0.isEmpty && $0 != "full" } ?? false
    let identifier = declared ? (Bundle.main.object(forInfoDictionaryKey: "MSIMESettingsBundleIdentifier") as? String ?? "app.msime.macos") : "app.msime.macos"
    return FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
      .appendingPathComponent(identifier, isDirectory: true)
  }
  static var refreshLock: BackendFileRefreshLock {
    BackendFileRefreshLock(url: standardDirectory?.appendingPathComponent("account-refresh.lock", isDirectory: false))
  }
  let directory: URL?
  init(directory: URL? = BackendDesktopSessionFile.standardDirectory) { self.directory = directory }
  private var url: URL? { directory?.appendingPathComponent(Self.fileName, isDirectory: false) }

  func load() throws -> BackendSavedSession? {
    guard let url else { return nil }
    var status = stat()
    if lstat(url.path, &status) != 0 {
      if errno == ENOENT { return nil }
      throw BackendAccountClient.Failure(status: 0)
    }
    // A symlink, a file another user can read, or one too large to be a session is not a store either process wrote.
    guard status.st_mode & S_IFMT == S_IFREG, status.st_mode & 0o077 == 0, status.st_uid == geteuid(),
          status.st_size <= Self.maximumBytes else { throw BackendAccountClient.Failure(status: 0) }
    let data: Data
    do { data = try Self.readBounded(url, maximumBytes: Self.maximumBytes) }
    catch { throw BackendAccountClient.Failure(status: 0) }
    do { return try BackendSavedSession.validated(JSONDecoder().decode(BackendSavedSession.self, from: data)) }
    catch { throw BackendAccountClient.Failure(status: 0) }
  }

  /// 上面的元数据检查只能快速拒绝超限文件。分块读取确保检查后被替换的文件不会让会话加载器无限分配内存。
  static func readBounded(_ url: URL, maximumBytes: Int) throws -> Data {
    guard maximumBytes >= 0 else { throw BackendAccountClient.Failure(status: 0) }
    // `load()` checks the path with `lstat`, but another process could replace it before a
    // path-based FileHandle opens it. Keep the final component pinned and reject symlinks at
    // the open itself.
    let descriptor = open(url.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
    guard descriptor >= 0 else { throw BackendAccountClient.Failure(status: 0) }
    defer { close(descriptor) }
    var data = Data()
    data.reserveCapacity(min(maximumBytes, 64 * 1024))
    var buffer = [UInt8](repeating: 0, count: maximumBytes < 64 * 1024 ? maximumBytes + 1 : 64 * 1024)
    while true {
      let count = buffer.withUnsafeMutableBytes { bytes in
        read(descriptor, bytes.baseAddress, bytes.count)
      }
      if count == 0 { return data }
      if count < 0 {
        if errno == EINTR { continue }
        throw BackendAccountClient.Failure(status: 0)
      }
      guard count <= maximumBytes - data.count else {
        throw BackendAccountClient.Failure(status: 0)
      }
      data.append(contentsOf: buffer[..<count])
    }
  }

  func save(_ session: BackendSavedSession) throws {
    guard let directory else { throw BackendAccountClient.Failure(status: 0) }
    let data = try JSONEncoder().encode(BackendSavedSession.validated(session))
    guard backendDirectoryPathIsSafe(directory) else { throw BackendAccountClient.Failure(status: 0) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700])
    guard backendDirectoryPathIsSafe(directory) else { throw BackendAccountClient.Failure(status: 0) }
    let attributes = try FileManager.default.attributesOfItem(atPath: directory.path)
    if let permissions = attributes[.posixPermissions] as? NSNumber, permissions.intValue & 0o077 != 0 {
      try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory.path)
    }
    // Pin the directory before creating, publishing, or cleaning the temporary file. A
    // path-based rename could otherwise follow a parent directory that was swapped after
    // the safety check. Created 0600 before any byte is written and published by renameat,
    // so the tokens are never readable by anyone else and no reader sees half a document.
    let directoryDescriptor = open(directory.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
    guard directoryDescriptor >= 0 else { throw BackendAccountClient.Failure(status: 0) }
    defer { close(directoryDescriptor) }
    let temporaryName = ".\(Self.fileName).\(UUID().uuidString)"
    let descriptor = openat(directoryDescriptor, temporaryName,
      O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw BackendAccountClient.Failure(status: 0) }
    var written = true
    data.withUnsafeBytes { bytes in
      var offset = 0
      while offset < bytes.count {
        let count = write(descriptor, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
        if count < 0 {
          if errno == EINTR { continue }
          written = false
          break
        }
        if count == 0 { written = false; break }
        offset += count
      }
    }
    let synced = written && fsync(descriptor) == 0
    let closed = close(descriptor) == 0
    guard written, synced, closed, renameat(directoryDescriptor, temporaryName, directoryDescriptor, Self.fileName) == 0 else {
      unlinkat(directoryDescriptor, temporaryName, 0)
      throw BackendAccountClient.Failure(status: 0)
    }
  }

  func clear() throws {
    guard directory != nil else { return }
    guard let directory, backendDirectoryPathIsSafe(directory) else {
      throw BackendAccountClient.Failure(status: 0)
    }
    let directoryDescriptor = open(directory.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
    if directoryDescriptor < 0 {
      guard errno == ENOENT else { throw BackendAccountClient.Failure(status: 0) }
      return
    }
    defer { close(directoryDescriptor) }
    guard unlinkat(directoryDescriptor, Self.fileName, 0) == 0 || errno == ENOENT else {
      throw BackendAccountClient.Failure(status: 0)
    }
  }
}
#endif

/// Serializes every write to a stored session that several processes share: sign-in, refresh, profile updates and sign-out. The server rotates the refresh token on every refresh and revokes the whole session when a used one is presented again, so two processes refreshing from the same stored token sign the user out, and a refresh that finishes after another process signed out or switched accounts must not write its tokens back. Whoever holds this lock re-reads the store before writing.
protocol BackendRefreshLock: Sendable {
  /// Whether other processes read and write the same store. Such a store, not this process's memory, says who is signed in.
  var sharedAcrossProcesses: Bool { get }
  func run<T: Sendable>(_ body: @Sendable () async throws -> T) async throws -> T
}

/// For a session no other process shares.
struct BackendProcessRefreshLock: BackendRefreshLock {
  var sharedAcrossProcesses: Bool { false }
  func run<T: Sendable>(_ body: @Sendable () async throws -> T) async throws -> T { try await body() }
}

/// An exclusive `flock` on a file in a directory every sharing process can open. It is waited for by polling, so a cooperative thread is never blocked, and it is held only for the one refresh request: iOS ends a suspended process that keeps a lock in an App Group container.
struct BackendFileRefreshLock: BackendRefreshLock {
  let url: URL?
  var timeout: TimeInterval = 20
  var sharedAcrossProcesses: Bool { true }

  /// iOS: the App Group container, opened by both the app and the keyboard extension.
  static var appGroup: BackendFileRefreshLock {
    BackendFileRefreshLock(url: FileManager.default
      .containerURL(forSecurityApplicationGroupIdentifier: MSIMEAppEdition.appGroupIdentifier)?
      .appendingPathComponent("backend-account-refresh.lock", isDirectory: false))
  }

  func run<T: Sendable>(_ body: @Sendable () async throws -> T) async throws -> T {
    // No shared directory means no way to keep another process out, and refreshing anyway risks the revocation this lock exists to prevent.
    guard let url else { throw BackendAccountClient.Failure(status: 0) }
    let directory = url.deletingLastPathComponent()
    guard backendDirectoryPathIsSafe(directory) else {
      throw BackendAccountClient.Failure(status: 0)
    }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700])
    guard backendDirectoryPathIsSafe(directory) else { throw BackendAccountClient.Failure(status: 0) }
    let descriptor = open(url.path, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw BackendAccountClient.Failure(status: 0) }
    defer { close(descriptor) }
    let deadline = Date().addingTimeInterval(timeout)
    while flock(descriptor, LOCK_EX | LOCK_NB) != 0 {
      guard errno == EWOULDBLOCK || errno == EINTR, Date() < deadline else { throw BackendAccountClient.Failure(status: 0) }
      try await Task.sleep(nanoseconds: 50_000_000)
    }
    defer { flock(descriptor, LOCK_UN) }
    return try await body()
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
  private let refreshLock: any BackendRefreshLock

  /// On iOS the app and the keyboard extension share the stored sessions, so refreshes take the App Group lock there.
  #if os(iOS)
  static var defaultRefreshLock: any BackendRefreshLock { BackendFileRefreshLock.appGroup }
  #else
  static var defaultRefreshLock: any BackendRefreshLock { BackendProcessRefreshLock() }
  #endif

  /// Without a `storage`, the account's own store: the keychain on iOS, and on macOS the session file the settings app shares, whose refreshes then take the lock beside it.
  init(api: any BackendSessionAPI = BackendAccountClient(), storage: (any BackendSessionStorage)? = nil,
       refreshLock: (any BackendRefreshLock)? = nil) {
    self.api = api
    #if os(macOS)
    self.storage = storage ?? BackendDesktopSessionFile()
    self.refreshLock = refreshLock
      ?? (storage == nil ? BackendDesktopSessionFile.refreshLock : BackendAccountSession.defaultRefreshLock)
    #else
    self.storage = storage ?? BackendKeychain()
    self.refreshLock = refreshLock ?? BackendAccountSession.defaultRefreshLock
    #endif
  }
  func user() throws -> BackendAccountClient.User? {
    try load()
    return saved?.tokens.user
  }
  private var sharesStore: Bool { refreshLock.sharedAcrossProcesses }
  /// A store other processes share is read every time: the other process may have signed out or switched accounts, and an empty store then means signed out here too rather than a cue to keep using the session held in memory.
  private func load() throws {
    if !loaded || sharesStore {
      saved = try storage.load().map { try BackendSavedSession.validated($0) }
      loaded = true
    }
  }
  func signIn(challenge: String, credential: String,
              replacingAccount cleanup: @Sendable (String) throws -> Void = { _ in }) async throws {
    generation += 1
    refreshing?.cancel(); refreshing = nil
    let version = generation
    let tokens = try await api.login(challenge: challenge, credential: credential, linkToken: nil)
    try Task.checkCancellation()
    try await refreshLock.run {
      try await self.installReplacingAccount(tokens, version: version, cleanup: cleanup)
    }
  }
  private func installReplacingAccount(_ tokens: BackendAccountClient.Tokens,
                                       version: Int, cleanup: @Sendable (String) throws -> Void) throws {
    guard generation == version else { throw CancellationError() }
    let previous = try? storage.load().map { try BackendSavedSession.validated($0) }
    // 切换账号必须先取消旧账号的待处理工作，取消失败就保留旧会话、不发布新身份，避免旧账号的快照落进新账号。
    try install(tokens) {
      if let accountID = previous?.tokens.user.id, accountID != tokens.user.id { try cleanup(accountID) }
    }
  }
  private func install(_ tokens: BackendAccountClient.Tokens, sessionID: UUID? = UUID(),
                       beforeSave: () throws -> Void = {}) throws {
    let value = try BackendSavedSession.forTokens(tokens, sessionID: sessionID)
    try beforeSave()
    try storage.save(value)
    saved = value; loaded = true
  }
  func accessToken(retrying rejectedToken: String? = nil) async throws -> String {
    try await ensureSessionID()
    try load()
    // A refresh may have been started because another caller received a 401
    // for the still-unexpired access token. Every concurrent caller must join
    // it before returning that rejected token from the fast path.
    if let refreshing {
      let sessionID = saved?.sessionID
      let token = try await refreshing.value
      try load()
      guard saved?.sessionID == sessionID, saved?.tokens.access_token == token else { throw CancellationError() }
      return token
    }
    if let current = saved, current.expiresAt.timeIntervalSinceNow > 30 && rejectedToken != current.tokens.access_token {
      return current.tokens.access_token
    }
    // Other actors and processes share this storage and may already have rotated (or created) the session.
    let sessionID = saved?.sessionID
    if let stored = try? storage.load().map({ try BackendSavedSession.validated($0) }),
       stored.tokens.refresh_token != saved?.tokens.refresh_token {
      guard sessionID == nil || stored.sessionID == sessionID else { saved = stored; throw CancellationError() }
      saved = stored
    }
    guard let current = saved else { throw BackendAccountClient.Failure(status: 401) }
    if current.expiresAt.timeIntervalSinceNow > 30 && rejectedToken != current.tokens.access_token { return current.tokens.access_token }
    let refreshToken = current.tokens.refresh_token
    let version = generation
    let task = Task<String, Error> {
      try await self.refreshLock.run {
        try await self.refreshHoldingLock(refreshToken, rejectedToken: rejectedToken,
                                          sessionID: sessionID, version: version)
      }
    }
    refreshing = task
    defer { if version == generation { refreshing = nil } }
    return try await task.value
  }
  /// Runs with the refresh lock held, so no other process can rotate the stored session between the read below and the save after the refresh.
  private func refreshHoldingLock(_ expected: String, rejectedToken: String?,
                                  sessionID: UUID?, version: Int) async throws -> String {
    var refreshToken = expected
    let before = try? storage.load().map({ try BackendSavedSession.validated($0) })
    // Another process signed out while this one waited for the lock.
    if sharesStore && before == nil {
      saved = nil
      throw BackendAccountClient.Failure(status: 401)
    }
    if let stored = before, stored.sessionID != sessionID {
      saved = stored
      throw CancellationError()
    }
    // Another process may have rotated the session while this one waited for the lock; refreshing from the token it already used would revoke the session.
    if let stored = before, stored.tokens.refresh_token != expected {
      guard generation == version else { throw CancellationError() }
      saved = stored
      guard stored.sessionID == sessionID else { throw CancellationError() }
      if stored.expiresAt.timeIntervalSinceNow > 30 && rejectedToken != stored.tokens.access_token { return stored.tokens.access_token }
      refreshToken = stored.tokens.refresh_token
    }
    do {
      let tokens = try await api.refresh(refreshToken)
      guard generation == version else { throw CancellationError() }
      // Writers that do not take the lock (another host's own keychain code) can still change the store; tokens for a session that is no longer the stored one are discarded instead of resurrecting it.
      let current = try? storage.load().map({ try BackendSavedSession.validated($0) })
      if let current, current.tokens.refresh_token != refreshToken ||
          current.tokens.user.id != tokens.user.id || current.sessionID != sessionID {
        saved = current
        throw CancellationError()
      }
      if sharesStore && current == nil {
        saved = nil
        throw BackendAccountClient.Failure(status: 401)
      }
      try install(tokens, sessionID: sessionID)
      return tokens.access_token
    } catch {
      if version == generation, let failure = error as? BackendAccountClient.Failure, failure.status == 401 {
        // Clear only the session that was rejected; a rotation saved meanwhile elsewhere is adopted.
        // A nil read may be an unreadable keychain rather than an absent item, so it is not cleared.
        let stored = try? storage.load().map({ try BackendSavedSession.validated($0) })
        if let stored, stored.sessionID != sessionID {
          saved = stored
          throw CancellationError()
        }
        if let stored, stored.tokens.refresh_token != refreshToken {
          saved = stored
          guard stored.sessionID == sessionID else { throw CancellationError() }
          if stored.expiresAt.timeIntervalSinceNow > 30 && rejectedToken != stored.tokens.access_token { return stored.tokens.access_token }
        } else {
          if stored != nil { try storage.clear() }
          saved = nil
        }
      }
      throw error
    }
  }
  func updateUser(_ user: BackendAccountClient.User, matching token: String,
                  matchingSessionID sessionID: UUID? = nil) async throws {
    try await refreshLock.run { try await self.updateUserHoldingLock(user, matching: token, matchingSessionID: sessionID) }
  }
  private func updateUserHoldingLock(_ user: BackendAccountClient.User, matching token: String,
                                     matchingSessionID sessionID: UUID?) throws {
    try load()
    guard let current = saved, current.tokens.access_token == token, current.tokens.user.id == user.id,
          sessionID == nil || current.sessionID == sessionID else {
      throw CancellationError()
    }
    let old = current.tokens
    let tokens = BackendAccountClient.Tokens(access_token: old.access_token, refresh_token: old.refresh_token,
      token_type: old.token_type, expires_in: old.expires_in, user: user)
    let value = try BackendSavedSession.validated(.init(tokens: tokens, expiresAt: current.expiresAt,
                                                        sessionID: current.sessionID))
    try storage.save(value); saved = value
  }
  /// 在登录和刷新共用的锁内为旧会话保存 ID，使两个进程在发请求前认定同一次登录。
  private func ensureSessionID() async throws {
    try load()
    guard let saved, saved.sessionID == nil else { return }
    try await refreshLock.run { try await self.installSessionIDHoldingLock() }
  }
  private func installSessionIDHoldingLock() throws {
    guard let current = try storage.load().map({ try BackendSavedSession.validated($0) }) else {
      saved = nil
      throw BackendAccountClient.Failure(status: 401)
    }
    if current.sessionID != nil { saved = current; return }
    let migrated = BackendSavedSession(tokens: current.tokens, expiresAt: current.expiresAt)
    try storage.save(migrated)
    saved = migrated
  }
  func credentials(retrying rejectedToken: String? = nil, matchingUserID expected: String? = nil,
                   matchingSessionID expectedSessionID: UUID? = nil) async throws -> (userID: String, token: String, sessionID: UUID) {
    try await ensureSessionID()
    try load()
    if let expected, saved?.tokens.user.id != expected { throw CancellationError() }
    if let expectedSessionID, saved?.sessionID != expectedSessionID { throw CancellationError() }
    let token = try await accessToken(retrying: rejectedToken)
    guard let saved, saved.tokens.access_token == token,
          let sessionID = saved.sessionID,
          expected == nil || saved.tokens.user.id == expected,
          expectedSessionID == nil || sessionID == expectedSessionID else { throw CancellationError() }
    return (saved.tokens.user.id, token, sessionID)
  }
  func requireSession(matchingUserID userID: String, matchingSessionID sessionID: UUID) throws {
    try load()
    guard saved?.tokens.user.id == userID, saved?.sessionID == sessionID else { throw CancellationError() }
  }
  /// Perform one authenticated request and retry it once when the backend rejects the
  /// still locally valid access token. The returned token is the one paired with the
  /// successful result, so callers that update cached account data cannot bind it to a
  /// token that was already rejected.
  func authenticated<T: Sendable>(matchingUserID expected: String, matchingSessionID expectedSessionID: UUID? = nil,
                                  _ operation: @Sendable (String) async throws -> T) async throws -> (value: T, token: String) {
    var identity = try await credentials(matchingUserID: expected, matchingSessionID: expectedSessionID)
    let sessionID = identity.sessionID
    let value: T
    do {
      value = try await operation(identity.token)
    } catch let failure as BackendAccountClient.Failure where failure.status == 401 {
      identity = try await credentials(retrying: identity.token, matchingUserID: expected,
                                       matchingSessionID: sessionID)
      value = try await operation(identity.token)
    }
    try requireSession(matchingUserID: expected, matchingSessionID: sessionID)
    try Task.checkCancellation()
    return (value, identity.token)
  }
  func forget(removingAccount cleanup: @Sendable (String) -> Void = { _ in }) async throws {
    try await forget(removingAccount: cleanup, fallbackAccountID: nil)
  }
  func forget(matchingUserID expected: String, matchingSessionID expectedSessionID: UUID? = nil,
              removingAccount cleanup: @Sendable (String) -> Void = { _ in }) async throws {
    try await forget(removingAccount: cleanup, fallbackAccountID: expected, matchingUserID: expected,
                     matchingSessionID: expectedSessionID)
  }
  private func forget(removingAccount cleanup: @Sendable (String) -> Void,
                      fallbackAccountID: String?, matchingUserID expectedUserID: String? = nil,
                      matchingSessionID expectedSessionID: UUID? = nil,
                      allowMissingExpected: Bool = false) async throws {
    // Keep the old identity before clearing memory. The shared lock below rechecks the account
    // so an older sign-out cannot clear another process's replacement session. A refresh by
    // that process rotates its token but must still allow this sign-out to finish.
    let stored = try storage.load()
    if let expectedUserID, stored?.tokens.user.id != expectedUserID,
       !(allowMissingExpected && stored == nil) { throw CancellationError() }
    if let expectedSessionID, stored?.sessionID != expectedSessionID,
       !(allowMissingExpected && stored == nil) { throw CancellationError() }
    let expected = stored ?? saved
    let sessionID = expectedSessionID ?? expected?.sessionID
    let accountID = expected?.tokens.user.id ?? fallbackAccountID
    generation += 1
    refreshing?.cancel(); refreshing = nil
    saved = nil; loaded = true
    let version = generation
    // 清理必须在共享锁内完成，避免另一个进程正在刷新的 token 在注销后写回。
    // 如果暂时拿不到锁就保留持久化会话；无锁清理会让进行中的刷新重新复活已注销的会话。
    try await refreshLock.run {
      try await self.clearStorage(at: version, accountID: accountID,
                                  matchingUserID: expectedUserID,
                                  matchingSessionID: sessionID,
                                  allowMissingExpected: allowMissingExpected, cleanup: cleanup)
    }
  }
  private func clearStorage(at version: Int, accountID: String?,
                            matchingUserID expectedUserID: String?,
                            matchingSessionID expectedSessionID: UUID?,
                            allowMissingExpected: Bool,
                            cleanup: @Sendable (String) -> Void) throws {
    guard generation == version else { throw CancellationError() }
    let current = try storage.load()
    if let expectedUserID, current?.tokens.user.id != expectedUserID,
       !(allowMissingExpected && current == nil) { throw CancellationError() }
    if let expectedSessionID, current?.sessionID != expectedSessionID,
       !(allowMissingExpected && current == nil) { throw CancellationError() }
    if let current, current.tokens.user.id != accountID { throw CancellationError() }
    // 先取消旧账号的待处理工作再清除身份，缩小键盘按旧身份领取的窗口。回调不能抛错：退出登录是用户明确要离开这个账号，注销账号时远端已删除，清除失效登录时身份已不可用，清理失败也必须让本地会话清掉，失败由调用方自行记录；切换账号的 `replacingAccount` 才允许失败并阻止切换。
    if let accountID = current?.tokens.user.id ?? accountID { cleanup(accountID) }
    try storage.clear()
  }
  func logout(all: Bool = false, removingAccount cleanup: @Sendable (String) -> Void = { _ in }) async throws {
    try await ensureSessionID()
    guard let owner = try user()?.id, let sessionID = saved?.sessionID else {
      throw BackendAccountClient.Failure(status: 401)
    }
    let version = generation
    let token: String
    do { token = try await credentials(matchingUserID: owner, matchingSessionID: sessionID).token }
    catch {
      guard generation == version else { throw CancellationError() }
      try await forget(removingAccount: cleanup, fallbackAccountID: owner,
                       matchingUserID: owner, matchingSessionID: sessionID, allowMissingExpected: true)
      throw error
    }
    guard generation == version else { throw CancellationError() }
    try await forget(matchingUserID: owner, matchingSessionID: sessionID, removingAccount: cleanup)
    try await api.logout(token: token, all: all)
  }
}

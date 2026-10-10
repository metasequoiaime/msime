import Foundation
import XCTest
@testable import MSIMEBackend

private final class MemorySessions: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?
  init(_ value: BackendSavedSession?) { self.value = value }
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
}
private actor RefreshAPI: BackendSessionAPI {
  var refreshCount = 0
  private var continuation: CheckedContinuation<BackendAccountClient.Tokens, Error>?
  private var started: CheckedContinuation<Void, Never>?
  static func tokens(_ character: String = "a") -> BackendAccountClient.Tokens {
    .init(access_token: String(repeating: character, count: 64), refresh_token: String(repeating: "f", count: 64),
          token_type: "Bearer", expires_in: 900,
          user: .init(id: "synthetic-user", display_name: "测试", created_at: "2026-09-08"))
  }
  func login(challenge: String, credential: String, linkToken: String?) async throws -> BackendAccountClient.Tokens { Self.tokens() }
  func logout(token: String, all: Bool) async throws { }
  func refresh(_ token: String) async throws -> BackendAccountClient.Tokens {
    refreshCount += 1
    return try await withCheckedThrowingContinuation { continuation in
      self.continuation = continuation; started?.resume(); started = nil
    }
  }
  func waitUntilRefreshing() async {
    if continuation != nil { return }
    await withCheckedContinuation { started = $0 }
  }
  func finish() { continuation?.resume(returning: Self.tokens("b")); continuation = nil }
}
// Several actors share one storage; the server revokes a refresh token once it has rotated.
private final class SharedStoreAPI: BackendSessionAPI, @unchecked Sendable {
  private let lock = NSLock()
  private var calls = 0
  private let onRefresh: @Sendable (String) throws -> BackendAccountClient.Tokens
  private let loginResult: BackendAccountClient.Tokens
  init(_ onRefresh: @escaping @Sendable (String) throws -> BackendAccountClient.Tokens,
       loginResult: BackendAccountClient.Tokens = SharedStoreAPI.tokens("a", "f")) {
    self.onRefresh = onRefresh; self.loginResult = loginResult
  }
  var refreshCount: Int { lock.lock(); defer { lock.unlock() }; return calls }
  static func tokens(_ access: String, _ refresh: String) -> BackendAccountClient.Tokens {
    .init(access_token: String(repeating: access, count: 64), refresh_token: String(repeating: refresh, count: 64),
          token_type: "Bearer", expires_in: 900,
          user: .init(id: "synthetic-user", display_name: "测试", created_at: "2026-09-08"))
  }
  func login(challenge: String, credential: String, linkToken: String?) async throws -> BackendAccountClient.Tokens { loginResult }
  func logout(token: String, all: Bool) async throws { }
  func refresh(_ token: String) async throws -> BackendAccountClient.Tokens {
    lock.lock(); calls += 1; lock.unlock()
    return try onRefresh(token)
  }
}

private struct FailingRefreshLock: BackendRefreshLock {
  var sharedAcrossProcesses: Bool { true }
  func run<T: Sendable>(_ body: @Sendable () async throws -> T) async throws -> T {
    throw BackendAccountClient.Failure(status: 0)
  }
}

private final class ReplacingSessionLock: BackendRefreshLock, @unchecked Sendable {
  let storage: MemorySessions
  let replacement: BackendSavedSession
  private let lock = NSLock()
  private var replaced = false
  var sharedAcrossProcesses: Bool { true }
  init(storage: MemorySessions, replacement: BackendSavedSession) {
    self.storage = storage; self.replacement = replacement
  }
  func run<T: Sendable>(_ body: @Sendable () async throws -> T) async throws -> T {
    let shouldReplace = lock.withLock { () -> Bool in
      if replaced { return false }
      replaced = true
      return true
    }
    if shouldReplace { try storage.save(replacement) }
    return try await body()
  }
}

private final class AttemptCounter: @unchecked Sendable {
  private let lock = NSLock()
  private var value = 0
  func increment() -> Int { lock.lock(); defer { lock.unlock() }; value += 1; return value }
  var count: Int { lock.lock(); defer { lock.unlock() }; return value }
}

final class BackendAccountSessionTests: XCTestCase {
  private enum CleanupRefused: Error { case busy }

  func testReplacingAccountCancelsOldWorkBeforePublishingNewIdentity() async throws {
    let old = try BackendSavedSession.forTokens(RefreshAPI.tokens())
    let storage = MemorySessions(old)
    let new = BackendAccountClient.Tokens(
      access_token: String(repeating: "b", count: 64), refresh_token: String(repeating: "c", count: 64),
      token_type: "Bearer", expires_in: 900,
      user: .init(id: "replacement-user", display_name: "Replacement", created_at: "2026-09-08"))
    let api = SharedStoreAPI({ _ in SharedStoreAPI.tokens("a", "f") }, loginResult: new)
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: BackendProcessRefreshLock())

    try await session.signIn(challenge: "challenge", credential: "synthetic", replacingAccount: { accountID in
      XCTAssertEqual(accountID, "synthetic-user")
      XCTAssertEqual(try? storage.load()?.tokens.user.id, "synthetic-user")
    })

    XCTAssertEqual(try storage.load()?.tokens.user.id, "replacement-user")
  }

  func testForgettingAccountCancelsOldWorkBeforeRemovingIdentity() async throws {
    let storage = MemorySessions(try BackendSavedSession.forTokens(RefreshAPI.tokens()))
    let session = BackendAccountSession(api: RefreshAPI(), storage: storage, refreshLock: BackendProcessRefreshLock())

    try await session.forget(removingAccount: { accountID in
      XCTAssertEqual(accountID, "synthetic-user")
      XCTAssertEqual(try? storage.load()?.tokens.user.id, "synthetic-user")
    })

    XCTAssertNil(try storage.load())
  }

  func testStaleForgetCannotRemoveReplacementAccount() async throws {
    let replacement = BackendAccountClient.Tokens(
      access_token: String(repeating: "b", count: 64), refresh_token: String(repeating: "c", count: 64),
      token_type: "Bearer", expires_in: 900,
      user: .init(id: "replacement-user", display_name: "Replacement", created_at: "2026-09-08"))
    let storage = MemorySessions(try BackendSavedSession.forTokens(replacement))
    let session = BackendAccountSession(api: RefreshAPI(), storage: storage, refreshLock: BackendProcessRefreshLock())
    do {
      try await session.forget(matchingUserID: "synthetic-user", removingAccount: { _ in
        XCTFail("stale cleanup must not run for the replacement account")
      })
      XCTFail("old account's completion must not clear the replacement account")
    } catch is CancellationError { }
    XCTAssertEqual(try storage.load()?.tokens.user.id, "replacement-user")
  }

  func testLogoutDoesNotClearSameUsersNewSessionWhileWaitingForLock() async throws {
    let old = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("a", "f"))
    let replacement = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("d", "e"))
    let storage = MemorySessions(old)
    let session = BackendAccountSession(api: SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") },
                                        storage: storage,
                                        refreshLock: ReplacingSessionLock(storage: storage, replacement: replacement))

    do {
      try await session.logout()
      XCTFail("the old logout must not clear the same user's new session")
    } catch is CancellationError { }
    catch { XCTFail("expected cancellation, got \(error)") }
    XCTAssertEqual(try storage.load()?.sessionID, replacement.sessionID)
  }

  func testRefreshDoesNotAdoptSameUsersNewLoginWhileWaitingForLock() async throws {
    let old = BackendSavedSession(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast)
    let replacement = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("d", "e"))
    let storage = MemorySessions(old)
    let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
    let session = BackendAccountSession(api: api, storage: storage,
                                        refreshLock: ReplacingSessionLock(storage: storage, replacement: replacement))

    do {
      _ = try await session.accessToken(retrying: old.tokens.access_token)
      XCTFail("旧登录不得收养同一用户的新会话")
    } catch is CancellationError { }
    XCTAssertEqual(api.refreshCount, 0)
    XCTAssertEqual(try storage.load()?.sessionID, replacement.sessionID)
  }

  func testBoundForgetDoesNotClearSameUsersNewSession() async throws {
    let old = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("a", "f"))
    let replacement = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("d", "e"))
    let storage = MemorySessions(old)
    let session = BackendAccountSession(api: SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") },
                                        storage: storage, refreshLock: try sharedLock())
    let identity = try await session.credentials(matchingUserID: "synthetic-user")
    try storage.save(replacement)

    do {
      try await session.forget(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
      XCTFail("the old account deletion must not clear the same user's new session")
    } catch is CancellationError { }
    XCTAssertEqual(try storage.load()?.sessionID, replacement.sessionID)
  }

  func testProfileUpdateDoesNotModifySameUsersNewLoginWithReusedToken() async throws {
    let original = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("a", "f"))
    let replacement = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("a", "f"))
    let storage = MemorySessions(original)
    let session = BackendAccountSession(api: SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") },
                                        storage: storage, refreshLock: try sharedLock())
    try storage.save(replacement)
    let renamed = BackendAccountClient.User(id: replacement.tokens.user.id, display_name: "新名称",
                                             created_at: replacement.tokens.user.created_at)

    do {
      try await session.updateUser(renamed, matching: original.tokens.access_token,
                                   matchingSessionID: original.sessionID)
      XCTFail("旧会话的资料不得写入新登录")
    } catch is CancellationError { }
    XCTAssertEqual(try storage.load()?.tokens.user.display_name, replacement.tokens.user.display_name)
  }

  func testLogoutDuringRefreshCannotClearReplacementAccount() async throws {
    let old = BackendSavedSession(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast)
    let storage = MemorySessions(old)
    let replacement = BackendAccountClient.Tokens(
      access_token: String(repeating: "b", count: 64), refresh_token: String(repeating: "c", count: 64),
      token_type: "Bearer", expires_in: 900,
      user: .init(id: "replacement-user", display_name: "Replacement", created_at: "2026-09-08"))
    let api = SharedStoreAPI { _ in
      try storage.save(BackendSavedSession.forTokens(replacement))
      return SharedStoreAPI.tokens("d", "e")
    }
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: BackendProcessRefreshLock())

    do {
      try await session.logout()
      XCTFail("old logout must stop after another account replaces the session")
    } catch is CancellationError { }
    XCTAssertEqual(try storage.load()?.tokens.user.id, "replacement-user")
  }

  func testLogoutStillCleansOldAccountAfterRejectedRefresh() async throws {
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let api = SharedStoreAPI { _ in throw BackendAccountClient.Failure(status: 401) }
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: BackendProcessRefreshLock())
    let cleanup = AttemptCounter()

    do {
      try await session.logout(removingAccount: { accountID in
        XCTAssertEqual(accountID, "synthetic-user")
        _ = cleanup.increment()
      })
      XCTFail("revoked refresh token must fail logout")
    } catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 401) }
    XCTAssertEqual(cleanup.count, 1)
    XCTAssertNil(try storage.load())
  }

  func testFailedReplacementCleanupKeepsOldIdentity() async throws {
    let storage = MemorySessions(try BackendSavedSession.forTokens(RefreshAPI.tokens()))
    let replacement = BackendAccountClient.Tokens(
      access_token: String(repeating: "b", count: 64), refresh_token: String(repeating: "c", count: 64),
      token_type: "Bearer", expires_in: 900,
      user: .init(id: "replacement-user", display_name: "Replacement", created_at: "2026-09-08"))
    let api = SharedStoreAPI({ _ in SharedStoreAPI.tokens("a", "f") }, loginResult: replacement)
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: BackendProcessRefreshLock())

    do {
      try await session.signIn(challenge: "challenge", credential: "synthetic", replacingAccount: { _ in
        throw CleanupRefused.busy
      })
      XCTFail("replacement must stop when old work cannot be cancelled")
    } catch CleanupRefused.busy { }

    XCTAssertEqual(try storage.load()?.tokens.user.id, "synthetic-user")
  }

  func testFailedForgetCleanupKeepsOldIdentity() async throws {
    let storage = MemorySessions(try BackendSavedSession.forTokens(RefreshAPI.tokens()))
    let session = BackendAccountSession(api: RefreshAPI(), storage: storage, refreshLock: BackendProcessRefreshLock())

    do {
      try await session.forget(removingAccount: { _ in throw CleanupRefused.busy })
      XCTFail("forget must stop when old work cannot be cancelled")
    } catch CleanupRefused.busy { }

    XCTAssertEqual(try storage.load()?.tokens.user.id, "synthetic-user")
  }

  func testSignInRejectsUnboundedExpiryFromAPI() async throws {
    let invalid = BackendAccountClient.Tokens(access_token: String(repeating: "a", count: 64),
      refresh_token: String(repeating: "b", count: 64), token_type: "Bearer", expires_in: 86_400 * 30 + 1,
      user: .init(id: "synthetic-user", display_name: "测试", created_at: "2026-09-08"))
    let storage = MemorySessions(nil)
    let session = BackendAccountSession(api: SharedStoreAPI({ _ in SharedStoreAPI.tokens("a", "f") }, loginResult: invalid), storage: storage)
    do { try await session.signIn(challenge: "challenge", credential: "synthetic"); XCTFail("must reject") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
    XCTAssertNil(try storage.load())
  }

  func testPersistedSessionExpiryBeyondThirtyDaysIsRejected() async throws {
    let tokens = RefreshAPI.tokens()
    let storage = MemorySessions(.init(tokens: tokens,
      expiresAt: Date().addingTimeInterval(TimeInterval(86_400 * 30 + 1))))
    let session = BackendAccountSession(api: RefreshAPI(), storage: storage)
    do { _ = try await session.accessToken(); XCTFail("must reject") }
    catch let error as BackendAccountClient.Failure { XCTAssertEqual(error.status, 0) }
  }

  func testConcurrentCallersShareOneRefreshAndPersistRotation() async throws {
    let storage = MemorySessions(.init(tokens: RefreshAPI.tokens(), expiresAt: .distantPast))
    let api = RefreshAPI()
    let session = BackendAccountSession(api: api, storage: storage)
    let first = Task { try await session.accessToken() }
    await api.waitUntilRefreshing()
    let second = Task { try await session.accessToken() }
    try await Task.sleep(nanoseconds: 20_000_000)
    await api.finish()
    let a = try await first.value, b = try await second.value
    XCTAssertEqual(a, b)
    let count = await api.refreshCount
    XCTAssertEqual(count, 1)
    XCTAssertEqual(try storage.load()?.tokens.access_token, a)
  }
  func testConcurrentCallerJoinsRefreshAfterAnotherCallerRejectsCurrentToken() async throws {
    let original = BackendSavedSession(tokens: RefreshAPI.tokens(), expiresAt: Date().addingTimeInterval(600))
    let storage = MemorySessions(original)
    let api = RefreshAPI()
    let session = BackendAccountSession(api: api, storage: storage)
    let refreshing = Task { try await session.accessToken(retrying: original.tokens.access_token) }
    await api.waitUntilRefreshing()
    let concurrent = Task { try await session.accessToken() }
    await api.finish()
    let rotated = try await refreshing.value
    let concurrentToken = try await concurrent.value
    XCTAssertEqual(concurrentToken, rotated)
  }
  func testProfileCacheUpdatePreservesSessionAndRejectsLateResult() async throws {
    let original = BackendSavedSession(tokens: RefreshAPI.tokens(), expiresAt: Date().addingTimeInterval(600))
    let storage = MemorySessions(original)
    let session = BackendAccountSession(api: RefreshAPI(), storage: storage)
    let renamed = BackendAccountClient.User(id: "synthetic-user", display_name: "新昵称", created_at: "2026-09-08")
    try await session.updateUser(renamed, matching: original.tokens.access_token)
    XCTAssertEqual(try storage.load()?.tokens.user, renamed)
    XCTAssertEqual(try storage.load()?.expiresAt, original.expiresAt)
    try await session.forget()
    do { try await session.updateUser(renamed, matching: original.tokens.access_token); XCTFail("late profile resurrected logout") }
    catch is CancellationError { }
    XCTAssertNil(try storage.load())
  }
  func testRetryForDifferentAccountNeverRefreshesOrReturnsCurrentToken() async throws {
    let storage = MemorySessions(.init(tokens: RefreshAPI.tokens(), expiresAt: .distantPast))
    let api = RefreshAPI()
    let session = BackendAccountSession(api: api, storage: storage)
    do {
      _ = try await session.credentials(retrying: "old-account-token", matchingUserID: "previous-account")
      XCTFail("must not retry an old account request as the current account")
    } catch is CancellationError { }
    let count = await api.refreshCount
    XCTAssertEqual(count, 0)
  }

  func testAuthenticatedRequestRefreshesRejectedAccessTokenOnce() async throws {
    let original = BackendSavedSession(tokens: SharedStoreAPI.tokens("a", "f"),
                                       expiresAt: Date().addingTimeInterval(600))
    let storage = MemorySessions(original)
    let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
    let session = BackendAccountSession(api: api, storage: storage)
    let attempts = AttemptCounter()

    let result = try await session.authenticated(matchingUserID: "synthetic-user") { token in
      if attempts.increment() == 1 {
        throw BackendAccountClient.Failure(status: 401)
      }
      return token
    }

    XCTAssertEqual(result.value, String(repeating: "b", count: 64))
    XCTAssertEqual(result.token, result.value)
    XCTAssertEqual(attempts.count, 2)
    XCTAssertEqual(api.refreshCount, 1)
    XCTAssertEqual(try storage.load()?.tokens.access_token, result.value)
  }
  func testAuthenticatedRequestPropagatesSecondRejectionAndOtherFailures() async throws {
    for status in [401, 403, 409, 503] {
      let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"),
                                         expiresAt: Date().addingTimeInterval(600)))
      let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
      let session = BackendAccountSession(api: api, storage: storage)
      let attempts = AttemptCounter()
      do {
        _ = try await session.authenticated(matchingUserID: "synthetic-user") { _ -> String in
          _ = attempts.increment()
          throw BackendAccountClient.Failure(status: status)
        }
        XCTFail("must propagate rejected request")
      } catch let error as BackendAccountClient.Failure {
        XCTAssertEqual(error.status, status)
      }
      XCTAssertEqual(attempts.count, status == 401 ? 2 : 1)
      XCTAssertEqual(api.refreshCount, status == 401 ? 1 : 0)
    }
  }
  func testAuthenticatedRequestNeverRetriesAfterAccountSwitch() async throws {
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"),
                                       expiresAt: Date().addingTimeInterval(600)))
    let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
    let session = BackendAccountSession(api: api, storage: storage)
    let attempts = AttemptCounter()
    let other = BackendAccountClient.Tokens(access_token: String(repeating: "d", count: 64),
      refresh_token: String(repeating: "e", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "other-synthetic-user", display_name: "另一个账号", created_at: "2026-09-08"))
    do {
      _ = try await session.authenticated(matchingUserID: "synthetic-user") { _ -> String in
        _ = attempts.increment()
        try storage.save(BackendSavedSession.forTokens(other))
        throw BackendAccountClient.Failure(status: 401)
      }
      XCTFail("must not retry as another account")
    } catch is CancellationError { }
    XCTAssertEqual(attempts.count, 1)
    XCTAssertEqual(api.refreshCount, 0)
    XCTAssertEqual(try storage.load()?.tokens.user.id, "other-synthetic-user")
  }
  func testAuthenticatedRequestNeverRetriesAfterSameUserSignsInAgain() async throws {
    let old = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("a", "f"))
    let replacement = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("d", "e"))
    let storage = MemorySessions(old)
    let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: try sharedLock())
    let attempts = AttemptCounter()

    do {
      _ = try await session.authenticated(matchingUserID: "synthetic-user") { _ -> String in
        _ = attempts.increment()
        try storage.save(replacement)
        throw BackendAccountClient.Failure(status: 401)
      }
      XCTFail("the old request must not retry after the same user signs in again")
    } catch is CancellationError { }
    catch { XCTFail("expected cancellation, got \(error)") }
    XCTAssertEqual(attempts.count, 1)
    XCTAssertEqual(api.refreshCount, 0)
    XCTAssertEqual(try storage.load()?.tokens.access_token, replacement.tokens.access_token)
  }

  func testAuthenticatedRequestDiscardsSuccessAfterSameUserSignsInAgain() async throws {
    let old = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("a", "f"))
    let replacement = try BackendSavedSession.forTokens(SharedStoreAPI.tokens("d", "e"))
    let storage = MemorySessions(old)
    let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: try sharedLock())

    do {
      _ = try await session.authenticated(matchingUserID: "synthetic-user") { token in
        try storage.save(replacement)
        return token
      }
      XCTFail("the old response must not be returned after the same user signs in again")
    } catch is CancellationError { }
    XCTAssertEqual(try storage.load()?.tokens.access_token, replacement.tokens.access_token)
  }
  func testBoundCredentialsRejectLogoutDuringRefresh() async throws {
    let storage = MemorySessions(.init(tokens: RefreshAPI.tokens(), expiresAt: .distantPast))
    let api = RefreshAPI()
    let session = BackendAccountSession(api: api, storage: storage)
    let pending = Task { try await session.credentials(matchingUserID: "synthetic-user") }
    await api.waitUntilRefreshing()
    try await session.forget()
    await api.finish()
    do { _ = try await pending.value; XCTFail("late credentials returned") }
    catch is CancellationError { }
  }
  func testLogoutGenerationRejectsLateRefresh() async throws {
    let storage = MemorySessions(.init(tokens: RefreshAPI.tokens(), expiresAt: .distantPast))
    let api = RefreshAPI()
    let session = BackendAccountSession(api: api, storage: storage)
    let pending = Task { try await session.accessToken() }
    await api.waitUntilRefreshing()
    try await session.forget()
    // Deliberately ignore task cancellation in the fake transport, as an already
    // delivered network response can race cancellation in a real application.
    await api.finish()
    do { _ = try await pending.value; XCTFail("must reject stale completion") }
    catch is CancellationError { }
    let user = try await session.user()
    XCTAssertNil(user)
    XCTAssertNil(try storage.load())
  }

  func testSecondActorAdoptsRotationInsteadOfRefreshingRevokedToken() async throws {
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let api = SharedStoreAPI { token in
      guard token == String(repeating: "f", count: 64) else { throw BackendAccountClient.Failure(status: 401) }
      return SharedStoreAPI.tokens("b", "c")
    }
    let first = BackendAccountSession(api: api, storage: storage)
    let second = BackendAccountSession(api: api, storage: storage)
    _ = try await second.user()
    let rotated = try await first.accessToken()
    let adopted = try await second.accessToken()
    XCTAssertEqual(adopted, rotated)
    XCTAssertEqual(api.refreshCount, 1)
    XCTAssertEqual(try storage.load()?.tokens.refresh_token, String(repeating: "c", count: 64))
  }
  func testUnauthorizedRefreshKeepsSessionRotatedMeanwhile() async throws {
    let original = BackendSavedSession(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast)
    let storage = MemorySessions(original)
    let winner = BackendSavedSession(tokens: SharedStoreAPI.tokens("b", "c"),
                                     expiresAt: Date().addingTimeInterval(600), sessionID: original.sessionID)
    let api = SharedStoreAPI { _ in
      // Another process rotates while this refresh is in flight, so ours is rejected.
      try storage.save(winner)
      throw BackendAccountClient.Failure(status: 401)
    }
    let session = BackendAccountSession(api: api, storage: storage)
    let token = try await session.accessToken()
    XCTAssertEqual(token, winner.tokens.access_token)
    XCTAssertEqual(try storage.load()?.tokens.refresh_token, winner.tokens.refresh_token)
  }
  func testUnauthorizedRefreshOfStoredTokenClearsIt() async throws {
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let api = SharedStoreAPI { _ in throw BackendAccountClient.Failure(status: 401) }
    let session = BackendAccountSession(api: api, storage: storage)
    do { _ = try await session.accessToken(); XCTFail("revoked session must not yield a token") }
    catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 401) }
    XCTAssertNil(try storage.load())
  }
  /// The app and the keyboard extension each hold their own session over one stored item. The server revokes the session when a used refresh token comes back, so the second process must wait for the first and adopt its rotation.
  func testProcessesSharingAStoreRefreshOneAtATimeUnderTheFileLock() async throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let lockURL = directory.appendingPathComponent("refresh.lock")
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let used = NSLock()
    var presented: Set<String> = []
    let api = SharedStoreAPI { token in
      used.lock(); defer { used.unlock() }
      // A refresh token works once; presenting it again revokes the session, as the server does.
      guard presented.insert(token).inserted else { throw BackendAccountClient.Failure(status: 401) }
      Thread.sleep(forTimeInterval: 0.1)
      return SharedStoreAPI.tokens("b", "c")
    }
    let app = BackendAccountSession(api: api, storage: storage, refreshLock: BackendFileRefreshLock(url: lockURL))
    let keyboard = BackendAccountSession(api: api, storage: storage, refreshLock: BackendFileRefreshLock(url: lockURL))
    _ = try await app.user(); _ = try await keyboard.user()
    async let first = app.accessToken()
    async let second = keyboard.accessToken()
    let tokens = try await [first, second]
    XCTAssertEqual(tokens[0], tokens[1])
    XCTAssertEqual(api.refreshCount, 1)
    XCTAssertEqual(try storage.load()?.tokens.refresh_token, String(repeating: "c", count: 64))
  }
  /// A refresh the keyboard has in flight when the app signs out or switches accounts. The refresh request is held open until the app's write has been attempted.
  private final class GatedRefreshAPI: BackendSessionAPI, @unchecked Sendable {
    private let lock = NSLock()
    private var waiting: CheckedContinuation<Void, Never>?
    private var started: CheckedContinuation<Void, Never>?
    private var isStarted = false
    let loginResult: BackendAccountClient.Tokens
    init(loginResult: BackendAccountClient.Tokens) { self.loginResult = loginResult }
    func login(challenge: String, credential: String, linkToken: String?) async throws -> BackendAccountClient.Tokens { loginResult }
    func logout(token: String, all: Bool) async throws { }
    func refresh(_ token: String) async throws -> BackendAccountClient.Tokens {
      await withCheckedContinuation { continuation in
        let notify = lock.withLock { () -> CheckedContinuation<Void, Never>? in
          waiting = continuation; isStarted = true
          defer { started = nil }
          return started
        }
        notify?.resume()
      }
      return SharedStoreAPI.tokens("b", "c")
    }
    func waitUntilRefreshing() async {
      await withCheckedContinuation { continuation in
        let already = lock.withLock { () -> Bool in
          if isStarted { return true }
          started = continuation
          return false
        }
        if already { continuation.resume() }
      }
    }
    func finish() { lock.withLock { () -> CheckedContinuation<Void, Never>? in defer { waiting = nil }; return waiting }?.resume() }
  }
  private func sharedLock() throws -> BackendFileRefreshLock {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    return BackendFileRefreshLock(url: directory.appendingPathComponent("refresh.lock"))
  }

  func testSignOutDuringAnotherProcessRefreshIsNotUndone() async throws {
    let lock = try sharedLock()
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let api = GatedRefreshAPI(loginResult: SharedStoreAPI.tokens("x", "y"))
    let app = BackendAccountSession(api: api, storage: storage, refreshLock: lock)
    let keyboard = BackendAccountSession(api: api, storage: storage, refreshLock: lock)
    let refresh = Task { try await keyboard.accessToken() }
    await api.waitUntilRefreshing()
    let signOut = Task { try await app.forget() }
    try await Task.sleep(nanoseconds: 100_000_000)
    api.finish()
    _ = try? await refresh.value
    try await signOut.value
    XCTAssertNil(try storage.load(), "the keyboard's refresh resurrected a signed-out session")
    let keyboardUser = try await keyboard.user()
    XCTAssertNil(keyboardUser)
  }

  func testAccountSwitchDuringAnotherProcessRefreshKeepsTheNewAccount() async throws {
    let lock = try sharedLock()
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let other = BackendAccountClient.Tokens(access_token: String(repeating: "d", count: 64),
      refresh_token: String(repeating: "e", count: 64), token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-other", display_name: "另一个", created_at: "2026-09-08"))
    let api = GatedRefreshAPI(loginResult: other)
    let app = BackendAccountSession(api: api, storage: storage, refreshLock: lock)
    let keyboard = BackendAccountSession(api: api, storage: storage, refreshLock: lock)
    let refresh = Task { try await keyboard.accessToken() }
    await api.waitUntilRefreshing()
    let switchAccount = Task { try await app.signIn(challenge: "challenge", credential: "synthetic") }
    try await Task.sleep(nanoseconds: 100_000_000)
    api.finish()
    _ = try? await refresh.value
    try await switchAccount.value
    XCTAssertEqual(try storage.load()?.tokens.user.id, "synthetic-other", "the keyboard wrote the old account over the new one")
    let token = try await keyboard.accessToken()
    XCTAssertEqual(token, other.access_token)
  }

  func testEmptySharedStoreSignsOutASessionHeldInMemory() async throws {
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: Date().addingTimeInterval(600)))
    let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
    let keyboard = BackendAccountSession(api: api, storage: storage, refreshLock: try sharedLock())
    let first = try await keyboard.accessToken()
    XCTAssertEqual(first, String(repeating: "a", count: 64))
    try storage.clear()
    let user = try await keyboard.user()
    XCTAssertNil(user)
    do { _ = try await keyboard.accessToken(); XCTFail("a session the app forgot must not be used") }
    catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 401) }
    XCTAssertEqual(api.refreshCount, 0)
  }
  func testRefreshWithoutASharedLockDirectoryIsRefused() async throws {
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let api = SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") }
    let session = BackendAccountSession(api: api, storage: storage, refreshLock: BackendFileRefreshLock(url: nil))
    do { _ = try await session.accessToken(); XCTFail("must not refresh unlocked") }
    catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }
    XCTAssertEqual(api.refreshCount, 0)
    XCTAssertEqual(try storage.load()?.tokens.refresh_token, String(repeating: "f", count: 64), "the stored session is kept")
  }

  func testSignOutDoesNotClearWhenTheSharedLockCannotBeTaken() async throws {
    let stored = BackendSavedSession(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: Date().addingTimeInterval(600))
    let storage = MemorySessions(stored)
    let session = BackendAccountSession(api: SharedStoreAPI { _ in SharedStoreAPI.tokens("b", "c") },
                                        storage: storage, refreshLock: FailingRefreshLock())
    do {
      try await session.forget()
      XCTFail("sign-out must report a lock failure")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 0)
    }
    XCTAssertEqual(try storage.load()?.tokens.refresh_token, stored.tokens.refresh_token,
                   "an unlocked clear could race an in-flight refresh and resurrect the session")
  }

  func testActorThatLoadedNothingSeesLaterSignIn() async throws {
    let storage = MemorySessions(nil)
    let api = SharedStoreAPI { _ in throw BackendAccountClient.Failure(status: 401) }
    let session = BackendAccountSession(api: api, storage: storage)
    let before = try await session.user()
    XCTAssertNil(before)
    let signedIn = BackendSavedSession(tokens: SharedStoreAPI.tokens("c", "d"), expiresAt: Date().addingTimeInterval(600))
    try storage.save(signedIn)
    let token = try await session.accessToken()
    XCTAssertEqual(token, signedIn.tokens.access_token)
    XCTAssertEqual(api.refreshCount, 0)
  }
}

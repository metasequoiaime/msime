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
final class BackendAccountSessionTests: XCTestCase {
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
    let storage = MemorySessions(.init(tokens: SharedStoreAPI.tokens("a", "f"), expiresAt: .distantPast))
    let winner = BackendSavedSession(tokens: SharedStoreAPI.tokens("b", "c"), expiresAt: Date().addingTimeInterval(600))
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

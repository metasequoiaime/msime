import Foundation
import XCTest
@testable import MSIMEBackend

private final class AnonymousMemoryStorage: BackendSessionStorage, @unchecked Sendable {
  private let lock = NSLock()
  private var value: BackendSavedSession?
  init(_ value: BackendSavedSession? = nil) { self.value = value }
  func load() throws -> BackendSavedSession? { lock.lock(); defer { lock.unlock() }; return value }
  func save(_ session: BackendSavedSession) throws { lock.lock(); defer { lock.unlock() }; value = session }
  func clear() throws { lock.lock(); defer { lock.unlock() }; value = nil }
}

private struct AnonymousNoopAPI: BackendSessionAPI {
  func login(challenge: String, credential: String, linkToken: String?) async throws -> BackendAccountClient.Tokens {
    fatalError("synthetic test does not log in")
  }
  func refresh(_ token: String) async throws -> BackendAccountClient.Tokens {
    fatalError("synthetic test does not refresh")
  }
  func logout(token: String, all: Bool) async throws { }
}

final class BackendAnonymousAccountTests: XCTestCase {
  func testDiscardClearsCachedSessionAndBothAnonymousStores() async throws {
    let tokens = BackendAccountClient.Tokens(
      access_token: String(repeating: "a", count: 64),
      refresh_token: String(repeating: "b", count: 64),
      token_type: "Bearer", expires_in: 900,
      user: .init(id: "synthetic-anonymous", display_name: "匿名", created_at: "2026-09-08"))
    let saved = try BackendSavedSession.forTokens(tokens)
    let identityStorage = AnonymousMemoryStorage(saved)
    let sessionStorage = AnonymousMemoryStorage(saved)
    let account = BackendAccountSession(api: AnonymousNoopAPI(), storage: sessionStorage)
    let before = try await account.user()?.id
    XCTAssertEqual(before, "synthetic-anonymous")

    try await BackendAnonymousAccount.discard(accountSession: account,
                                              identityStorage: identityStorage,
                                              sessionStorage: sessionStorage)

    XCTAssertNil(try identityStorage.load())
    XCTAssertNil(try sessionStorage.load())
    let after = try await account.user()
    XCTAssertNil(after)
  }
}

import Foundation
import Security

/// Lazily creates an anonymous backend account for keyboard-only services such as candidate glosses.
enum BackendAnonymousAccount {
  struct Credentials: Codable, Sendable { let subject: String; let secret: String }
  private static let fileName = "anonymous-account.json"
  private static func generated() -> Credentials {
    let alphabet = Array("abcdefghijklmnopqrstuvwxyz0123456789")
    func random(_ count: Int) -> String {
      var bytes = [UInt8](repeating: 0, count: count)
      _ = SecRandomCopyBytes(kSecRandomDefault, count, &bytes)
      return String(bytes.map { alphabet[Int($0) % alphabet.count] })
    }
    return Credentials(subject: "msime-" + random(16), secret: random(48))
  }
  static func stored() -> Credentials? {
    guard let data = BackendLocalStore.read(fileName) else { return nil }
    return try? JSONDecoder().decode(Credentials.self, from: data)
  }
  static func sessionStorage() -> any BackendSessionStorage {
    BackendLocalStore(fileName: "anonymous-session.json")
  }
  /// One in-process actor for the anonymous storage, so its rotating refresh token has one owner.
  static let session = BackendAccountSession(storage: sessionStorage())

  /// Discard the local-only identity and its session after it has been replaced
  /// by a real account or explicitly deleted by the user.
  static func discard() async throws {
    try await discard(accountSession: session,
                      identityStorage: BackendLocalStore(fileName: fileName),
                      sessionStorage: sessionStorage())
  }
  static func discard(accountSession: BackendAccountSession,
                      identityStorage: any BackendSessionStorage,
                      sessionStorage: any BackendSessionStorage) async throws {
    try await accountSession.forget()
    try identityStorage.clear()
    try sessionStorage.clear()
  }

  static func ensureSignedIn(session: BackendAccountSession,
                             client: BackendAccountClient) async throws -> Credentials {
    let credentials: Credentials
    if let existing = stored() {
      credentials = existing
    } else {
      let candidate = generated()
      let data = try JSONEncoder().encode(candidate)
      if BackendLocalStore.writeIfAbsent(data, to: fileName) {
        credentials = candidate
      } else {
        guard let existing = stored() else { throw BackendAccountClient.Failure(status: 0) }
        credentials = existing
      }
    }
    let challenge = try await client.challenge(provider: "anonymous", target: credentials.subject)
    try await session.signIn(challenge: challenge.challenge_id, credential: credentials.secret)
    return credentials
  }
}

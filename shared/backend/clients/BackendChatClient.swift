import Foundation

extension BackendAccountClient {
  /// Translate one visible candidate page in order. The backend owns the provider and credentials;
  /// the keyboard only receives bounded display strings and never sends the user's raw keystrokes.
  func translate(texts: [String], target: String, session: BackendAccountSession) async throws -> [String] {
    let identity = try await session.credentials()
    let result = try await session.authenticated(matchingUserID: identity.userID,
                                                 matchingSessionID: identity.sessionID) { token in
      try await translate(texts: texts, target: target, token: token)
    }.value
    try await session.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
    try Task.checkCancellation()
    return result
  }

  func translate(texts: [String], target: String, token: String) async throws -> [String] {
    struct Body: Encodable { let texts: [String]; let source_lang = "ZH"; let target_lang: String }
    struct Response: Decodable { let code: Int; let data: [String] }
    guard (1...32).contains(texts.count), !target.isEmpty, target.utf8.count <= 16,
          texts.allSatisfy({ !$0.isEmpty && $0.utf8.count <= 2048 }) else {
      throw Failure(status: 400)
    }
    let body = try JSONEncoder().encode(Body(texts: texts, target_lang: target.uppercased()))
    let response: Response = try await json("POST", "/v1/translate", token: token,
      body: body, timeout: 30)
    guard response.code == 200, response.data.count == texts.count,
          response.data.allSatisfy({ $0.utf8.count <= 4096 && !$0.unicodeScalars.contains(where: { $0.value == 0x0A || $0.value == 0x0D || ($0.value < 0x20 && $0.value != 0x09) }) })
    else { throw Failure(status: 502) }
    return response.data
  }
}

import Foundation

extension BackendAccountClient {
  struct SkinArtwork: Decodable, Sendable {
    let b64_json: String
    let mime_type: String
    let width: Int
    let height: Int
  }
  private struct SkinArtworkJob: Decodable {
    let id: String
    let state: String
    let artwork: SkinArtwork?
  }

  static func validSkinArtworkPrompt(_ prompt: String) -> Bool {
    !prompt.isEmpty && prompt.unicodeScalars.count <= 500
  }

  func skinArtwork(prompt: String, account: BackendAccountSession, userID: String) async throws -> SkinArtwork {
    guard Self.validSkinArtworkPrompt(prompt) else { throw Failure(status: 400) }
    struct Body: Encodable { let prompt: String }
    let credential = try await account.credentials(matchingUserID: userID)
    try Task.checkCancellation()
    let job: SkinArtworkJob = try await json("POST", "/v1/skins/jobs", token: credential.token,
      body: JSONEncoder().encode(Body(prompt: prompt)))
    guard job.id.utf8.count == 48, job.id.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else {
      throw Failure(status: 0)
    }
    let path = "/v1/skins/jobs/" + job.id
    // Cancellation must also release the upstream job. A detached cleanup is
    // bounded and awaited, so parent cancellation cannot skip the DELETE.
    func release() async {
      await Task.detached { [self] in
        _ = try? await request("DELETE", path, token: credential.token, timeout: 5)
      }.value
    }
    do {
      let deadline = ProcessInfo.processInfo.systemUptime + 200
      while ProcessInfo.processInfo.systemUptime < deadline {
        let fresh = try await account.credentials(matchingUserID: userID)
        try Task.checkCancellation()
        let result: SkinArtworkJob = try await json("GET", path, token: fresh.token,
          timeout: min(30, max(1, deadline - ProcessInfo.processInfo.systemUptime)), maximumResponseBytes: 12 * 1024 * 1024)
        guard result.id == job.id else { throw Failure(status: 0) }
        switch result.state {
        case "succeeded":
          guard let artwork = result.artwork else { throw Failure(status: 0) }
          _ = try await account.credentials(matchingUserID: userID)
          try Task.checkCancellation()
          await release()
          return artwork
        case "running": try await Task.sleep(nanoseconds: 5_000_000_000)
        case "failed": throw Failure(status: 502)
        default: throw Failure(status: 0)
        }
      }
      throw Failure(status: 504)
    } catch {
      await release()
      throw error
    }
  }
}

import Foundation

// Candidate glosses use the account's bounded translation endpoint. The native input method
// already loads this Swift backend as a private dylib, so the Objective-C++ controller only needs a
// C entry point and a notification carrying display-safe strings back to the main thread.
@MainActor
enum BackendCandidateGloss {
  struct Request: Sendable {
    let words: [String]
    let primary: String
    let secondary: String
    let generation: UInt64
  }

  nonisolated static let notification = Notification.Name("MSIMEBackendCandidateTranslationsDidArrive")
  private nonisolated static let account = BackendAccountSession.shared
  private nonisolated static let anonymous = BackendAnonymousAccount.session
  private nonisolated static let client = BackendAccountClient()
  // One page in flight and one waiting, as Windows' single cloud translation worker keeps them. On a slow network every page the user typed past used to start its own pair of requests, and the page on screen queued behind all of them at the service's rate limit. A newer page now replaces the waiting one, and the one in flight runs to the end rather than being cancelled, because its answers still fill the controller's cache.
  private static var pending: Request?
  private static var inFlight: Task<Void, Never>?
  // The network round trip for one page. A variable so BackendAccountTests can count what is sent without a server or an account.
  static var perform: @MainActor (Request) async -> Void = { await translate($0) }

  private nonisolated static func translationSession() async throws -> BackendAccountSession {
    if (try? await account.accessToken()) != nil { return account }
    if (try? await anonymous.accessToken()) != nil { return anonymous }
    _ = try await BackendAnonymousAccount.ensureSignedIn(session: anonymous, client: client)
    return anonymous
  }

  static func fetch(words: [String], primary: String, secondary: String, generation: UInt64) {
    guard !words.isEmpty, !primary.isEmpty else { return }
    pending = Request(words: words, primary: primary, secondary: secondary, generation: generation)
    pump()
  }

  // Drops the waiting page, for when the input method stops using the account (the user turned it off, chose a service of their own, or left the field). The request in flight is left to finish, since its answers only fill the cache, but a page that had not gone out yet must not be sent after the user opted out.
  static func cancelPending() {
    pending = nil
  }

  private static func pump() {
    guard inFlight == nil, let request = pending else { return }
    pending = nil
    inFlight = Task { @MainActor in
      await perform(request)
      inFlight = nil
      pump()
    }
  }

  private nonisolated static func translate(_ request: Request) async {
    let words = request.words, generation = request.generation
    guard let session = try? await translationSession() else { return }
    await withTaskGroup(of: Void.self) { group in
      for code in [request.primary, request.secondary] where !code.isEmpty {
        group.addTask {
          // A request that failed (offline, rate limited) posts nothing, so the words stay unknown and are asked about again.
          guard let values = try? await client.translate(texts: words, target: code, session: session) else { return }
          await MainActor.run {
            NotificationCenter.default.post(name: notification, object: nil,
                                            userInfo: payload(words: words, values: values, code: code, generation: generation))
          }
        }
      }
    }
  }

  // The notification's userInfo for one language's reply, apart from the network so BackendAccountTests can check what the input method receives.
  nonisolated static func payload(words: [String], values: [String], code: String, generation: UInt64) -> [String: Any] {
    // Every word asked about gets an answer, so the input method can remember the ones the account had nothing for and stop asking about them for a while, as Windows does. An empty or unchanged value is sent as "", and a duplicate candidate keeps whichever of its answers is not empty.
    let table = Dictionary(zip(words, values).map { ($0.0, $0.1 == $0.0 ? "" : $0.1) },
                           uniquingKeysWith: { first, second in first.isEmpty ? second : first })
    return [
      // Each reply names its own language, so the input method files it under that target without depending on the order in which the two requests finish, and saves only the English one to the learned glossary.
      "generation": generation,
      "target": code,
      "translations": table,
    ]
  }
}

// Called on the main thread by InputController.mm whenever it stops asking the account.
@_cdecl("MSIMECancelAccountCandidateGlosses")
public func msimeCancelAccountCandidateGlosses() {
  MainActor.assumeIsolated { BackendCandidateGloss.cancelPending() }
}

// Called on the main thread by InputController.mm, from the idle timer that follows a page change.
@_cdecl("MSIMEFetchAccountCandidateGlosses")
public func msimeFetchAccountCandidateGlosses(_ wordsJSON: UnsafePointer<CChar>, _ primary: UnsafePointer<CChar>,
                                               _ secondary: UnsafePointer<CChar>, _ generation: UInt64) {
  guard let data = String(cString: wordsJSON).data(using: .utf8),
        let words = try? JSONDecoder().decode([String].self, from: data),
        words.count <= 32 else { return }
  let primary = String(cString: primary), secondary = String(cString: secondary)
  MainActor.assumeIsolated {
    BackendCandidateGloss.fetch(words: words, primary: primary, secondary: secondary, generation: generation)
  }
}

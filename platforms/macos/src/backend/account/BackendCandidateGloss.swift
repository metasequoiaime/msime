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
  // One page in flight and one waiting, as Windows' single cloud translation worker keeps them. On a slow network every page the user typed past used to start its own pair of requests, and the page on screen queued behind all of them at the service's rate limit. A newer page now replaces the waiting one, and the one in flight runs to the end rather than being cancelled, because its answers still fill the controller's cache.
  private static var pending: Request?
  private static var inFlight: Task<Void, Never>?
  // The network round trip for one page. A variable so BackendAccountTests can count what is sent without a server or an account.
  static var perform: @MainActor (Request) async -> Void = { request in
    guard let rust = BackendRustAccount.shared else { return }
    await translate(request, rust: rust)
  }

  /// 本进程补注册匿名账号的间隔：Rust 的 `translate` 登录账号优先、没有登录时用本机匿名账号，两个都没有时是 `account_unauthorized`，这时补注册一次再问；一个进程 600 秒内只补一次，按尝试计，与 Linux、Windows 相同（迁移差异 M37）。注册计入服务端每个出口地址每天 5 次的额度，原来每一页都会再注册一次。
  private actor AnonymousRegistration {
    static let shared = AnonymousRegistration()
    static let interval: TimeInterval = 600
    private var last: Date?
    func attempt() -> Bool {
      let now = Date()
      if let last, now.timeIntervalSince(last) < Self.interval { return false }
      last = now
      return true
    }
  }

  /// 一页候选的释义经 Rust 的 `translate`（#8201 迁移计划 4e）：令牌、刷新和会话选择都在 Rust 里。回答标记的是问之前登录的那个账号（`BackendAccountState`）；回来以后再读一次登录状态，换了号就丢掉，输入法也会按标记丢掉不属于当前账号的回答。
  nonisolated static func translate(_ request: Request, rust: BackendRustAccount) async {
    let words = request.words, generation = request.generation
    guard let marker = try? await BackendAccountState.refresh(rust) else { return }
    await withTaskGroup(of: Void.self) { group in
      for code in [request.primary, request.secondary] where !code.isEmpty {
        group.addTask {
          // 失败的请求（断网、限流）什么也不发，这些词仍算未知，下次再问。
          guard let values = await translations(words, target: code, rust: rust),
                (try? await BackendAccountState.refresh(rust)) == marker else { return }
          await MainActor.run {
            NotificationCenter.default.post(name: notification, object: nil,
                                            userInfo: payload(words: words, values: values, code: code, generation: generation, marker: marker))
          }
        }
      }
    }
  }

  /// 一种语言的释义，与 `words` 等长：Rust 把不合格的那一项给成 null，这里当作没有释义（""）。失败时为 nil。
  private nonisolated static func translations(_ words: [String], target: String, rust: BackendRustAccount) async -> [String]? {
    let action: [String: Any] = ["operation": "translate", "texts": words, "source_lang": "ZH", "target_lang": target]
    func decode(_ data: Data) -> [String]? {
      guard let object = try? BackendRustAccount.object(data), let values = object["translations"] as? [Any],
            values.count == words.count else { return nil }
      return values.map { $0 as? String ?? "" }
    }
    do { return decode(try await rust.account(action)) }
    catch let failure as BackendRustClient.Failure where failure.code == "account_unauthorized" {
      guard await AnonymousRegistration.shared.attempt(),
            (try? await rust.account(["operation": "ensure_anonymous"])) != nil,
            let data = try? await rust.account(action) else { return nil }
      return decode(data)
    } catch { return nil }
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

  // The notification's userInfo for one language's reply, apart from the network so BackendAccountTests can check what the input method receives.
  nonisolated static func payload(words: [String], values: [String], code: String, generation: UInt64,
                                  marker: UInt64 = 0) -> [String: Any] {
    // Every word asked about gets an answer, so the input method can remember the ones the account had nothing for and stop asking about them for a while, as Windows does. An empty or unchanged value is sent as "", and a duplicate candidate keeps whichever of its answers is not empty.
    let table = Dictionary(zip(words, values).map { ($0.0, $0.1 == $0.0 ? "" : $0.1) },
                           uniquingKeysWith: { first, second in first.isEmpty ? second : first })
    return [
      // Each reply names its own language, so the input method files it under that target without depending on the order in which the two requests finish, and saves only the English one to the learned glossary.
      "generation": generation,
      "marker": marker,
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

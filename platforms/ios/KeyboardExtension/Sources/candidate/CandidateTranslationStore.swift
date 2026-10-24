import Foundation

protocol CandidateTranslationService: Sendable {
  func translate(words: [String], target: String) async throws -> [String]
}

/// 账号释义：登录账号优先，没有登录时用本机匿名账号（没有可用的匿名会话时先注册它）。请求怎么拼、回答怎么读在 Rust（`AccountGloss`，`msime_client_account_gloss`），HTTP 和令牌在 Swift 的会话里：键盘扩展不注册 Rust 的会话存储（负责人决定：未测，扩展保留 Swift），与 App 里的 Rust 共用 App Group 的刷新锁。服务端以 401 拒绝令牌时刷新一次、在同一个账号里重试。
struct BackendCandidateTranslationService: CandidateTranslationService {
  private let client: BackendAccountClient
  private let account: BackendAccountSession
  private let anonymous: BackendAccountSession

  init(client: BackendAccountClient = BackendAccountClient(),
       account: BackendAccountSession = .shared,
       anonymous: BackendAccountSession = BackendAnonymousAccount.session) {
    self.client = client
    self.account = account
    self.anonymous = anonymous
  }

  func translate(words: [String], target: String) async throws -> [String] {
    let session: BackendAccountSession
    if (try? await account.user()) != nil {
      session = account
    } else {
      _ = try await token()
      session = anonymous
    }
    let request = try AccountGloss.request(texts: words, target: target)
    let identity = try await session.credentials()
    let client = client
    let response = try await session.authenticated(matchingUserID: identity.userID,
                                                   matchingSessionID: identity.sessionID) { token in
      let answer = try await client.exchange(request.method, request.path, token: token, body: request.body,
                                             timeout: 30, maximumResponseBytes: 1024 * 1024)
      // 被拒的令牌交给会话刷新一次再重试；其余状态码交给 Rust 读。
      if answer.status == 401 { throw BackendAccountClient.Failure(status: 401) }
      return answer
    }.value
    try await session.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
    try Task.checkCancellation()
    return try AccountGloss.parse(texts: words, status: response.status, body: response.body).map { $0 ?? "" }
  }

  private func token() async throws -> String {
    if let value = try? await anonymous.accessToken() { return value }
    _ = try await BackendAnonymousAccount.ensureSignedIn(session: anonymous, client: client)
    return try await anonymous.accessToken()
  }
}

/// A user-chosen provider (NiuTrans, Tencent TMT or a custom endpoint) signed and parsed by the shared host; words it cannot translate come back empty and are skipped.
struct ProviderCandidateTranslationService: CandidateTranslationService {
  let route: TranslationRoute
  var client = TranslationProviderClient()
  func translate(words: [String], target: String) async throws -> [String] {
    await client.translate(words: words, target: target, route: route).map { $0 ?? "" }
  }
}

@MainActor
final class CandidateTranslationStore {
  var onArrival: (() -> Void)?
  static let quietInterval: TimeInterval = 0.35
  private var service: any CandidateTranslationService
  /// Which provider and credentials the cached glosses came from.
  private(set) var scope: String
  private var cache: [String: String] = [:]
  private var signature: String?
  private var debounce: Timer?
  /// Requests belong to the visible candidate strip; cancel them when the keyboard leaves it.
  private var tasks: [UUID: Task<Void, Never>] = [:]
  /// The keyboard process lives long; only the page on screen needs its glosses kept.
  private let cacheLimit: Int
  init(service: any CandidateTranslationService = ProviderCandidateTranslationService(route: .none), scope: String = "none",
       cacheLimit: Int = 2_000) {
    self.service = service
    self.scope = scope
    self.cacheLimit = cacheLimit
  }
  /// Switch provider; glosses from another provider or other credentials are dropped, including replies still in flight.
  func use(_ service: any CandidateTranslationService, scope: String) {
    self.service = service
    guard scope != self.scope else { return }
    self.scope = scope
    cancel()
    cache.removeAll()
    signature = nil
  }
  static func translatable(_ word: String) -> Bool {
    word.unicodeScalars.contains { scalar in
      (0x4E00...0x9FFF).contains(scalar.value) || (0x3400...0x4DBF).contains(scalar.value)
        || (0xF900...0xFAFF).contains(scalar.value) || (0x20000...0x3FFFF).contains(scalar.value)
    }
  }
  static func sourceGetsNoGloss(_ source: Int) -> Bool {
    [6, 7, 13, 14, 18].contains(source)
  }
  static func allowsGloss(sources: [Int], at index: Int) -> Bool {
    sources.indices.contains(index) && !sourceGetsNoGloss(sources[index])
  }
  func gloss(word: String, code: String) -> String? { cache["\(code)|\(word)"] }
  func refresh(words: [String], codes: [String]) {
    refresh(words: words, sources: Array(repeating: 0, count: words.count), codes: codes)
  }
  func refresh(words: [String], sources: [Int], codes: [String]) {
    debounce?.invalidate(); debounce = nil
    let wanted: [String] = words.indices.compactMap { index -> String? in
      let word = words[index]
      guard Self.allowsGloss(sources: sources, at: index),
            Self.translatable(word) else { return nil }
      return word
    }
    guard !wanted.isEmpty, !codes.isEmpty else { return }
    let timer = Timer(timeInterval: Self.quietInterval, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated { self?.send(words: wanted, codes: codes) }
    }
    RunLoop.main.add(timer, forMode: .common); debounce = timer
  }
  func cancel() {
    debounce?.invalidate(); debounce = nil
    for task in tasks.values { task.cancel() }
    tasks.removeAll()
    signature = nil
  }

  /// Release candidate glosses when the keyboard extension is under memory pressure. The visible
  /// page will request them again after the warning has passed.
  func clearCacheForMemoryPressure() {
    cancel()
    cache.removeAll(keepingCapacity: false)
  }
  private static func signature(_ values: [String]) -> String {
     values.reduce(into: "\(values.count):") { result, value in
       result += "\(value.utf8.count):\(value)"
     }
   }
  private func send(words: [String], codes: [String]) {
    debounce = nil
    let stamp = "codes=" + Self.signature(codes) + "|words=" + Self.signature(words)
    // Evict before `pending` so the page on screen is asked again for every code.
    if cache.count > cacheLimit { cache.removeAll(); signature = nil }
    var pending: [String: [String]] = [:]
    for code in codes {
      let missing = words.filter { cache["\(code)|\($0)"] == nil }
      if !missing.isEmpty { pending[code] = missing }
    }
    guard !pending.isEmpty, stamp != signature else { signature = stamp; return }
    signature = stamp
    for (code, missing) in pending {
      let service = service
      let scope = scope
      let id = UUID()
      let task = Task { [weak self] in
        defer { self?.tasks[id] = nil }
        guard let glosses = try? await service.translate(words: missing, target: code) else {
          // 请求失败时释放当前签名，让同一候选页在下一次刷新时可以重试；较新的请求已经换代时不能误清它的签名。
          if self?.signature == stamp { self?.signature = nil }
          return
        }
        guard !Task.isCancelled, let self, self.scope == scope else { return }
        if !self.absorb(code: code, words: missing, glosses: glosses), self.signature == stamp {
          self.signature = nil
        }
      }
      tasks[id] = task
    }
  }
  private func absorb(code: String, words: [String], glosses: [String]) -> Bool {
    guard words.count == glosses.count else { return false }
    var arrived = false
    for (word, gloss) in zip(words, glosses) {
      let text = gloss.trimmingCharacters(in: .whitespacesAndNewlines)
      guard !text.isEmpty, text != word, text.utf8.count <= 4096 else { continue }
      cache["\(code)|\(word)"] = text; arrived = true
    }
    if arrived { onArrival?() }
    return true
  }
}

@_silgen_name("msime_client_account_gloss")
private func msimeClientAccountGloss(_ request: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_string_free")
private func msimeAccountGlossStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 账号释义的 sans-IO 一半（Rust 的 `msime_client_account_gloss`，#8201 迁移计划 4f）：请求体怎么拼、回答怎么读、单条不合格时怎么办都在 Rust，键盘扩展只负责带着 Swift 会话的令牌把请求发出去。扩展不注册 Rust 的会话存储（负责人决定：没有真机量内存，未测，扩展保留 Swift），所以 HTTP 和令牌仍在 Swift，与 Windows、Linux 在按键路径上自己发 HTTP 的做法相同。
enum AccountGloss {
  /// Rust 给出的失败码，例如 `account_unauthorized`、`account_rate_limited`。
  struct Failure: Error, Equatable {
    let code: String
  }

  /// 要发出去的那一次请求。
  struct Request: Equatable, Sendable {
    let method: String
    let path: String
    let body: Data
  }

  /// 一页候选的请求：原文按顺序交给 Rust 校验（1–32 条，每条非空白、不超过 8192 字节），语言代码由 Rust 转成大写。
  static func request(texts: [String], target: String) throws -> Request {
    let value = try call(["operation": "request", "texts": texts, "source_lang": "ZH", "target_lang": target])
    guard let object = value as? [String: Any], let method = object["method"] as? String,
          let path = object["path"] as? String, let body = object["body"] as? String else { throw Failure(code: "account_unavailable") }
    return Request(method: method, path: path, body: Data(body.utf8))
  }

  /// 读回答：2xx 时每一项要么是释义（制表符和换行已折成空格），要么是 nil（为空、与原文相同、含控制字符或超过 4096 字节），不让一条坏值作废整页；其余状态码换成账号错误码。
  static func parse(texts: [String], status: Int, body: Data) throws -> [String?] {
    let value = try call(["operation": "parse", "texts": texts, "status": status,
                          "body": String(decoding: body, as: UTF8.self)])
    guard let object = value as? [String: Any], let translations = object["translations"] as? [Any],
          translations.count == texts.count else { throw Failure(code: "account_unavailable") }
    return translations.map { $0 as? String }
  }

  /// 同步调用：不联网、不读会话，可以在任意线程上调。
  private static func call(_ request: [String: Any]) throws -> Any {
    guard JSONSerialization.isValidJSONObject(request), let data = try? JSONSerialization.data(withJSONObject: request) else {
      throw Failure(code: "account_invalid")
    }
    guard let raw = data.withUnsafeBytes({ bytes in
      msimeClientAccountGloss(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
    }) else { throw Failure(code: "account_unavailable") }
    let answer = Data(bytes: raw, count: strlen(raw))
    msimeAccountGlossStringFree(raw)
    guard let envelope = try? JSONSerialization.jsonObject(with: answer) as? [String: Any],
          let ok = envelope["ok"] as? Bool else { throw Failure(code: "account_unavailable") }
    guard ok else { throw Failure(code: envelope["error"] as? String ?? "account_unavailable") }
    return envelope["value"] ?? NSNull()
  }
}

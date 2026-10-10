import Foundation

protocol CandidateTranslationService: Sendable {
  func translate(words: [String], target: String) async throws -> [String]
}

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
    if (try? await account.user()) != nil {
      return try await client.translate(texts: words, target: target, session: account)
    }
    _ = try await token()
    return try await client.translate(texts: words, target: target, session: anonymous)
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
  func gloss(word: String, code: String) -> String? { cache["\(code)|\(word)"] }
  func refresh(words: [String], codes: [String]) {
    debounce?.invalidate(); debounce = nil
    let wanted = words.filter(Self.translatable)
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

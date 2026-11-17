/// 本地按下日的聚合快照；来源只按键位计数，不保留切换顺序或时间。
struct TypingKeyBatch: Equatable {
  let day: String
  let keys: [String: Int]
  let sources: [String: [String: Int]]

  /// 旧调用没有来源时保留未分类，不能用写出时的当前方案补齐。
  init(day: String, keys: [String: Int], sources: [String: [String: Int]]? = nil) {
    self.day = day
    self.keys = keys
    self.sources = sources ?? ["unknown": keys]
  }

  /// 共享层负责来源校验与语言分类；直接调用存储的旧入口可继续省略来源。
  static func request(keys: [String: Int], day: String, sources: [String: [String: Int]]?) -> [String: Any] {
    var request: [String: Any] = ["operation": "record_keys", "day": day, "keys": keys]
    if let sources { request["sources"] = sources }
    return request
  }
}

/// 日切换、累计 256 次、定时器或键盘收起时写出；切换语言不增加写盘。
struct TypingKeyCounter {
  static let flushThreshold = 256

  private(set) var day: String?
  private(set) var counts: [String: Int] = [:]
  private(set) var presses = 0
  private var sources: [String: [String: Int]] = [:]

  /// 调用者在按下时传已有来源 ID；无效键单独丢弃，避免整批被共享层拒绝。
  mutating func record(_ id: String, day: String, source: String = "unknown") -> [TypingKeyBatch] {
    guard TypingKeyID.known.contains(id) else { return [] }
    var ready: [TypingKeyBatch] = []
    if self.day != day, let previous = drain() { ready.append(previous) }
    self.day = day
    counts[id, default: 0] += 1
    sources[source, default: [:]][id, default: 0] += 1
    presses += 1
    if presses >= Self.flushThreshold, let full = drain() { ready.append(full) }
    return ready
  }

  /// Swift 字典的值语义使交出的嵌套快照不受下一批或后续语言切换影响。
  mutating func drain() -> TypingKeyBatch? {
    defer {
      day = nil
      counts = [:]
      sources = [:]
      presses = 0
    }
    guard let day, !counts.isEmpty else { return nil }
    return TypingKeyBatch(day: day, keys: counts, sources: sources)
  }
}

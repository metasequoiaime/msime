import Foundation

@main
struct TypingKeyBatchTests {
  static func main() throws {
    let firstDay = "2026-11-01"
    let secondDay = "2026-11-02"
    var counter = TypingKeyCounter()
    precondition(counter.record("KeyA", day: firstDay, source: "quanpin").isEmpty)
    precondition(counter.record("KeyA", day: firstDay, source: "english").isEmpty)
    precondition(counter.record("Nine2", day: firstDay, source: "japanese").isEmpty)
    precondition(counter.record("Space", day: firstDay, source: "japanese").isEmpty)
    precondition(counter.record("KeyA", day: firstDay).isEmpty)
    // 无效键不能引入空来源，连跨日也不提前封批。
    precondition(counter.record("NotAKey", day: secondDay, source: "korean").isEmpty)
    precondition(counter.day == firstDay && counter.presses == 5)
    let first = counter.drain()!
    precondition(first.keys == ["KeyA": 3, "Nine2": 1, "Space": 1])
    precondition(first.sources == ["quanpin": ["KeyA": 1], "english": ["KeyA": 1],
                                  "japanese": ["Nine2": 1, "Space": 1], "unknown": ["KeyA": 1]])
    checkTotals(first)
    precondition(counter.drain() == nil && counter.day == nil && counter.presses == 0)

    _ = counter.record("KeyA", day: firstDay, source: "english")
    let beforeMidnight = counter.record("Nine2", day: secondDay, source: "japanese")
    precondition(beforeMidnight == [TypingKeyBatch(day: firstDay, keys: ["KeyA": 1],
                                                  sources: ["english": ["KeyA": 1]])])
    var ready: [TypingKeyBatch] = []
    for _ in 1..<TypingKeyCounter.flushThreshold {
      ready += counter.record("KeyA", day: secondDay, source: "quanpin")
    }
    precondition(ready.count == 1)
    precondition(ready[0].day == secondDay && ready[0].keys == ["Nine2": 1, "KeyA": 255])
    precondition(ready[0].sources == ["japanese": ["Nine2": 1], "quanpin": ["KeyA": 255]])
    checkTotals(ready[0])
    precondition(counter.drain() == nil && counter.presses == 0)
    // 同一键和同一来源的下一批继续增加，不改写交给后台队列的快照。
    _ = counter.record("KeyA", day: secondDay, source: "english")
    _ = counter.record("Nine2", day: secondDay, source: "japanese")
    let later = counter.drain()!
    precondition(later.sources == ["english": ["KeyA": 1], "japanese": ["Nine2": 1]])
    precondition(first.sources["english"] == ["KeyA": 1])
    precondition(ready[0].sources["quanpin"] == ["KeyA": 255])

    // 旧无来源构造和旧计数入口保持一致；历史仅标成 unknown。
    _ = counter.record("Backspace", day: firstDay)
    precondition(counter.drain() == TypingKeyBatch(day: firstDay, keys: ["Backspace": 1]))
    let legacy = TypingKeyBatch.request(keys: ["KeyA": 1], day: firstDay, sources: nil)
    precondition(legacy["sources"] == nil)
    let request = TypingKeyBatch.request(keys: first.keys, day: first.day, sources: first.sources)
    let encoded = try JSONSerialization.data(withJSONObject: request)
    let decoded = try JSONSerialization.jsonObject(with: encoded) as! [String: Any]
    precondition(Set(decoded.keys) == ["operation", "day", "keys", "sources"])
    precondition(decoded["operation"] as? String == "record_keys")
    precondition(decoded["day"] as? String == firstDay)
    precondition(decoded["keys"] as? [String: Int] == first.keys)
    precondition(decoded["sources"] as? [String: [String: Int]] == first.sources)
    print("iOS key batches: mixed sources, unknown compatibility, midnight, threshold, immutable snapshots and JSON passed")
  }

  private static func checkTotals(_ batch: TypingKeyBatch) {
    var total: [String: Int] = [:]
    for keys in batch.sources.values {
      for (key, count) in keys { total[key, default: 0] += count }
    }
    precondition(total == batch.keys)
  }
}

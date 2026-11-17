import Foundation

@main
struct TypingKeyLanguageTests {
  static func main() {
    let first = "2026-11-10", second = "2026-11-11", old = "2026-10-01", future = "2026-11-12"
    var totals = [first: ["KeyA": 8, "Space": 3, "Digit1": 2], second: ["KeyA": 5, "Nine2": 4],
                  old: ["KeyA": 900], future: ["KeyA": 800]]
    var groups = [first: ["chinese": ["KeyA": 4], "english": ["KeyA": 2, "Digit1": 2],
                          "common": ["Space": 3], "unknown": ["KeyA": 2]],
                  second: ["english": ["KeyA": 5], "japanese": ["Nine2": 4]],
                  old: ["korean": ["KeyA": 900]], future: ["tibetan": ["KeyA": 800]]]
    let view = TypingKeyLanguages(dailyKeys: totals, dailyLanguages: groups, days: [first, second])
    precondition(view.available == [.all, .chinese, .english, .japanese, .common, .unknown])
    precondition(view.keys(for: .all) == ["KeyA": 13, "Space": 3, "Digit1": 2, "Nine2": 4])
    precondition(view.keys(for: .english) == ["KeyA": 7, "Digit1": 2])
    precondition(view.keys(for: .common) == ["Space": 3])
    precondition(view.keys(for: .unknown) == ["KeyA": 2])
    precondition(view.resolved(.tibetan) == .all)
    precondition(view.keys(for: .korean) == view.keys(for: .all))
    // 值快照不能受后续加载影响。
    totals[first] = ["KeyA": 1]
    groups[first] = ["english": ["KeyA": 1]]
    precondition(view.keys(for: .english) == ["KeyA": 7, "Digit1": 2])
    let changed = TypingKeyLanguages(dailyKeys: totals, dailyLanguages: groups, days: [first])
    precondition(changed.resolved(.japanese) == .all)
    precondition(changed.keys(for: .japanese) == ["KeyA": 1])
    let legacy = TypingKeyLanguages(dailyKeys: totals, dailyLanguages: [:], days: [first])
    precondition(legacy.available == [.all] && legacy.keys(for: .all) == ["KeyA": 1])
    precondition(legacy.resolved(.english) == .all)
    let empty = TypingKeyLanguages(dailyKeys: totals, dailyLanguages: groups, days: [])
    precondition(empty.available == [.all] && empty.keys(for: .all).isEmpty)
    let bad = TypingKeyLanguages(dailyKeys: [first: ["KeyA": Int.max], second: ["KeyA": 1]],
      dailyLanguages: [first: ["english": ["KeyA": Int.max], "korean": ["KeyA": 0, "Nine2": -1]],
                       second: ["english": ["KeyA": 1]]], days: [first, first, second])
    precondition(bad.keys(for: .all) == ["KeyA": Int.max])
    precondition(bad.keys(for: .english) == ["KeyA": Int.max])
    precondition(bad.available == [.all, .english])
    // 未认识的未来语言仍在完整总计里，不被误标为已知语言。
    let newer = TypingKeyLanguages(dailyKeys: [first: ["KeyA": 7]],
      dailyLanguages: [first: ["future_language": ["KeyA": 7]]], days: [first])
    precondition(newer.available == [.all] && newer.keys(for: .all) == ["KeyA": 7])
    let duplicate = TypingKeyLanguages(dailyKeys: totals, dailyLanguages: groups, days: [first, first])
    precondition(duplicate.keys(for: .all) == ["KeyA": 1])
    print("iOS 按键语言：七天窗口、旧历史、通用/未知分组、重载回退、值快照和溢出检查通过")
  }
}

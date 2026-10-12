import Foundation

struct WubiLookupRequestState {
  private(set) var text = ""
  private(set) var result: WubiCodeLookup?
  private(set) var busy = false
  private(set) var failed = false
  private var generation = UUID()

  /// 输入变化后，旧回答和错误都不再对应当前文字，允许立即发起新查询。
  mutating func edit(_ next: String) {
    guard text != next else { return }
    text = next
    result = nil
    failed = false
    busy = false
    generation = UUID()
  }

  /// 当前文字可查且没有同文字的未完成请求时，返回新请求的身份。
  mutating func begin() -> UUID? {
    guard !busy, WubiCodeLookup.searchable(text) else { return nil }
    busy = true
    generation = UUID()
    return generation
  }

  /// 只有当前请求能更新结果；编辑前的回答不能覆盖新文字。
  mutating func complete(_ request: UUID, answer: WubiCodeLookup?) -> Bool {
    guard busy, request == generation else { return false }
    result = answer
    failed = answer == nil
    busy = false
    return true
  }
}

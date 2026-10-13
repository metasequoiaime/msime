struct CommonPhrasesRequestState {
  private(set) var busy = false
  private var generation = 0

  /// 发起一次读取；新的读取会让较早读取的结果失效。
  mutating func beginReload() -> Int? {
    guard !busy else { return nil }
    generation += 1
    return generation
  }

  /// 只有最后一次读取仍然有效时，才允许它更新界面。
  func acceptReload(_ request: Int) -> Bool {
    request == generation
  }

  /// 开始写入，令所有在途读取失效，并阻止生命周期事件再发起读取。
  mutating func beginMutation() -> Bool {
    guard !busy else { return false }
    generation += 1
    busy = true
    return true
  }

  /// 写入完成后再次令旧读取失效，下一次读取必须使用新的代次。
  mutating func finishMutation() {
    generation += 1
    busy = false
  }

  /// 页面消失时令所有在途操作失效，并清掉阻止下一次出现时读取的忙碌状态。
  mutating func invalidate() {
    generation += 1
    busy = false
  }
}

import Foundation

/// 一次删除按压的身份与连删状态；滑出只停连删，松手或取消才结束按压。
struct BackspacePressState<Owner: Equatable> {
  private var owner: Owner?
  private var repeating = false
  private var repeated = false

  mutating func begin(_ owner: Owner) {
    self.owner = owner
    repeating = true
    repeated = false
  }

  func owns(_ owner: Owner) -> Bool { self.owner == owner }

  /// 返回是否为第一次连删；按压结束或滑出后的迟到回调不再有效。
  mutating func nextRepeat() -> Bool? {
    guard owner != nil, repeating else { return nil }
    let first = !repeated
    repeated = true
    return first
  }

  mutating func stopRepeating() { repeating = false }

  /// 滑回后仍可完成短按，但已连删的按压不能重新变成一次短按。
  mutating func pause(_ owner: Owner) -> Bool {
    guard owns(owner) else { return false }
    stopRepeating()
    return true
  }

  /// 触摸松手须匹配按压；没有触摸事件的程序化激活仍能独立单删。
  mutating func finish(_ owner: Owner, programmaticActivation: Bool = false) -> Bool? {
    guard owns(owner) else {
      guard programmaticActivation else { return nil }
      cancel()
      return true
    }
    let delete = !repeated
    cancel()
    return delete
  }

  mutating func cancel() {
    owner = nil
    repeating = false
    repeated = false
  }
}

/// 长按更早开始，连续删除仍保持原来的节奏。
enum BackspaceRepeatTiming {
  static let initialDelay: TimeInterval = 0.3
  static let interval: TimeInterval = 0.075
}

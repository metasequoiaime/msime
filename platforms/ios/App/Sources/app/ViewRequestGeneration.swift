import Foundation

/// 页面可见期间只接受当前代次的异步请求结果。
@MainActor struct ViewRequestGeneration {
  private(set) var isVisible = false
  private var currentID = UUID()

  mutating func appear() { isVisible = true }

  @discardableResult mutating func begin() -> UUID {
    currentID = UUID()
    return currentID
  }

  mutating func disappear() {
    isVisible = false
    currentID = UUID()
  }

  func isCurrent(_ id: UUID) -> Bool { isVisible && currentID == id }

  func checkCurrent(_ id: UUID) throws {
    try Task.checkCancellation()
    guard isCurrent(id) else { throw CancellationError() }
  }
}

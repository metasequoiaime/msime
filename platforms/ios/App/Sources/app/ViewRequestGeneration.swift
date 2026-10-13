import Foundation

/// 与页面生命周期无关的异步操作代次；取消或替换后旧结果不得写回。
@MainActor struct OperationGeneration {
  private var currentID = UUID()

  @discardableResult mutating func begin() -> UUID {
    invalidate()
    return currentID
  }

  mutating func invalidate() { currentID = UUID() }

  func current() -> UUID { currentID }

  func isCurrent(_ id: UUID) -> Bool { currentID == id }

  func checkCurrent(_ id: UUID) throws {
    try Task.checkCancellation()
    guard isCurrent(id) else { throw CancellationError() }
  }
}

/// 页面可见期间只接受当前代次的异步请求结果。
@MainActor struct ViewRequestGeneration {
  private(set) var isVisible = false
  private var operation = OperationGeneration()

  mutating func appear() { isVisible = true }

  @discardableResult mutating func begin() -> UUID {
    operation.begin()
  }

  /// 页面仍可见时让旧工作失效，供筛选条件变化后重新请求。
  mutating func invalidate() { operation.invalidate() }

  mutating func disappear() {
    isVisible = false
    invalidate()
  }

  /// 直接刷新沿用当前代次，不打断由页面操作入口启动的请求。
  func current() -> UUID? { isVisible ? operation.current() : nil }

  func isCurrent(_ id: UUID) -> Bool { isVisible && operation.isCurrent(id) }

  func checkCurrent(_ id: UUID) throws {
    try operation.checkCurrent(id)
    guard isCurrent(id) else { throw CancellationError() }
  }
}

import Foundation

/// 页面可见期间只接受当前代次的异步请求结果。
@MainActor struct ViewRequestGeneration {
  private(set) var isVisible = false
  private var currentID = UUID()

  mutating func appear() { isVisible = true }

  @discardableResult mutating func begin() -> UUID {
    invalidate()
    return currentID
  }

  /// 页面仍可见时让旧工作失效，供筛选条件变化后重新请求。
  mutating func invalidate() { currentID = UUID() }

  mutating func disappear() {
    isVisible = false
    invalidate()
  }

  /// 直接刷新沿用当前代次，不打断由页面操作入口启动的请求。
  func current() -> UUID? { isVisible ? currentID : nil }

  func isCurrent(_ id: UUID) -> Bool { isVisible && currentID == id }

  func checkCurrent(_ id: UUID) throws {
    try Task.checkCancellation()
    guard isCurrent(id) else { throw CancellationError() }
  }
}

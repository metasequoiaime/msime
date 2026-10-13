import Foundation

struct AboutUpdateRequestState {
  private(set) var inProgress = false
  private var generation = UUID()
  var currentID: UUID { generation }

  mutating func begin() -> UUID? {
    guard !inProgress else { return nil }
    inProgress = true
    generation = UUID()
    return generation
  }

  mutating func invalidate() {
    inProgress = false
    generation = UUID()
  }

  func isCurrent(_ request: UUID) -> Bool { request == generation }

  mutating func finish(_ request: UUID) -> Bool {
    guard inProgress, isCurrent(request) else { return false }
    inProgress = false
    return true
  }
}

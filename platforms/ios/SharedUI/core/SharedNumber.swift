import Foundation

enum SharedNumber {
  static func clamped<T: Comparable>(_ value: T, to range: ClosedRange<T>) -> T {
    min(max(value, range.lowerBound), range.upperBound)
  }
}

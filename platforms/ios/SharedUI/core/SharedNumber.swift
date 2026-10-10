import Foundation

enum SharedNumber {
  /// Parses an integral NSNumber without accepting bridged booleans or truncating fractions.
  static func strictInt(_ value: Any?) -> Int? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          let integer = Int(number.stringValue),
          NSNumber(value: integer).compare(number) == .orderedSame else { return nil }
    return integer
  }

  static func nonnegativeInt(_ value: Any?) -> Int? {
    guard let integer = strictInt(value), integer >= 0 else { return nil }
    return integer
  }

  static func clamped<T: Comparable>(_ value: T, to range: ClosedRange<T>) -> T {
    min(max(value, range.lowerBound), range.upperBound)
  }
}

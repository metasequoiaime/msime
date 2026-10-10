import Foundation

enum SharedNumber {
  /// Parses an integral NSNumber without accepting bridged booleans or truncating fractions.
  static func strictInt(_ value: Any?) -> Int? {
    guard let number = numeric(value),
          let integer = Int(number.stringValue),
          NSNumber(value: integer).compare(number) == .orderedSame else { return nil }
    return integer
  }

  static func strictInt64(_ value: Any?) -> Int64? {
    guard let number = numeric(value),
          let integer = Int64(number.stringValue),
          NSNumber(value: integer).compare(number) == .orderedSame else { return nil }
    return integer
  }

  static func strictUInt64(_ value: Any?) -> UInt64? {
    guard let number = numeric(value),
          let integer = UInt64(number.stringValue),
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

  private static func numeric(_ value: Any?) -> NSNumber? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID() else { return nil }
    return number
  }
}

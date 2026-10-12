import Foundation

/// Shared parsing for cloud clipboard timestamps returned to the app and keyboard extension.
enum CloudClipboardTimePolicy {
  private static let fractional: ISO8601DateFormatter = {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter
  }()
  private static let wholeSecond: ISO8601DateFormatter = {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime]
    return formatter
  }()

  static func date(_ value: String) -> Date? {
    if let date = fractional.date(from: value) { return date }
    return wholeSecond.date(from: value)
  }
}

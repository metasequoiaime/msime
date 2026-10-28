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

  /// `date` 距 `now` 多久，用 `formatter` 写成「30分钟前」「昨天」这样。不到一分钟的，以及晚于 `now` 的（另一台设备的时钟、毫秒取整），一律是「刚刚」，与 Android、鸿蒙和 Rust 状态行的说法相同：`RelativeDateTimeFormatter` 的数字样式把零间隔和不到一秒的间隔说成「0秒钟后」，具名样式说成「现在」，不到一分钟的说成「N秒钟前」。
  static func relative(_ date: Date, now: Date = Date(), formatter: RelativeDateTimeFormatter) -> String {
    guard now.timeIntervalSince(date) >= 60 else { return "刚刚" }
    return formatter.localizedString(for: date, relativeTo: now)
  }
}

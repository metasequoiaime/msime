import XCTest

final class CloudClipboardTimePolicyTests: XCTestCase {
  func testParsesFractionalAndWholeSecondTimestamps() {
    XCTAssertNotNil(CloudClipboardTimePolicy.date("2026-10-12T14:00:00.123Z"))
    XCTAssertNotNil(CloudClipboardTimePolicy.date("2026-10-12T14:00:00Z"))
  }

  func testRejectsMalformedTimestamp() {
    XCTAssertNil(CloudClipboardTimePolicy.date("not-a-timestamp"))
  }

  /// 不到一分钟的和晚于现在的时间都是「刚刚」：`RelativeDateTimeFormatter` 的数字样式会把零间隔和不到一秒的间隔写成「0秒钟后」。
  func testRecentAndFutureTimesReadAsJustNow() {
    let formatter = RelativeDateTimeFormatter()
    formatter.locale = Locale(identifier: "zh-Hans")
    formatter.unitsStyle = .full
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    for offset: TimeInterval in [0, -0.4, -59, 3, 3600] {
      XCTAssertEqual(CloudClipboardTimePolicy.relative(now.addingTimeInterval(offset), now: now, formatter: formatter), "刚刚", "\(offset)")
    }
    XCTAssertEqual(CloudClipboardTimePolicy.relative(now.addingTimeInterval(-1800), now: now, formatter: formatter), "30分钟前")
  }
}

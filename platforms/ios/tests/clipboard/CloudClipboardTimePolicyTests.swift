import XCTest

final class CloudClipboardTimePolicyTests: XCTestCase {
  func testParsesFractionalAndWholeSecondTimestamps() {
    XCTAssertNotNil(CloudClipboardTimePolicy.date("2026-10-12T14:00:00.123Z"))
    XCTAssertNotNil(CloudClipboardTimePolicy.date("2026-10-12T14:00:00Z"))
  }

  func testRejectsMalformedTimestamp() {
    XCTAssertNil(CloudClipboardTimePolicy.date("not-a-timestamp"))
  }
}

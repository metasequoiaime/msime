import Foundation
import XCTest

final class VerificationCodePolicyTests: XCTestCase {
  func testRemainingSecondsRoundsUpAndClampsAtZero() {
    let now = Date(timeIntervalSince1970: 1_000)
    XCTAssertEqual(VerificationCodePolicy.remainingSeconds(until: now.addingTimeInterval(0.01), now: now), 1)
    XCTAssertEqual(VerificationCodePolicy.remainingSeconds(until: now.addingTimeInterval(60), now: now), 60)
    XCTAssertEqual(VerificationCodePolicy.remainingSeconds(until: now.addingTimeInterval(-1), now: now), 0)
  }
}

import Foundation
import XCTest

final class SharedNumberTests: XCTestCase {
  func testStrictIntRejectsBooleansFractionsAndOverflow() {
    XCTAssertEqual(SharedNumber.strictInt(NSNumber(value: -2)), -2)
    XCTAssertEqual(SharedNumber.strictInt(NSNumber(value: 0)), 0)
    XCTAssertEqual(SharedNumber.strictInt(NSNumber(value: 3)), 3)
    XCTAssertNil(SharedNumber.strictInt(NSNumber(value: true)))
    XCTAssertNil(SharedNumber.strictInt(NSNumber(value: 1.5)))
    XCTAssertNil(SharedNumber.strictInt(NSNumber(value: UInt64.max)))
    XCTAssertNil(SharedNumber.strictInt("3"))
  }

  func testNonnegativeIntRetainsRangeConstraint() {
    XCTAssertNil(SharedNumber.nonnegativeInt(NSNumber(value: -1)))
    XCTAssertEqual(SharedNumber.nonnegativeInt(NSNumber(value: 0)), 0)
    XCTAssertEqual(SharedNumber.nonnegativeInt(NSNumber(value: 3)), 3)
  }
}

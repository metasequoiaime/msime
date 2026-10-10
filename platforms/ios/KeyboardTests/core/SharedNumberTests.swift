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

  func testStrictInt64PreservesSignedBounds() {
    XCTAssertEqual(SharedNumber.strictInt64(NSNumber(value: Int64.min)), Int64.min)
    XCTAssertEqual(SharedNumber.strictInt64(NSNumber(value: Int64.max)), Int64.max)
    XCTAssertNil(SharedNumber.strictInt64(NSNumber(value: true)))
    XCTAssertNil(SharedNumber.strictInt64(NSNumber(value: 2.5)))
    XCTAssertNil(SharedNumber.strictInt64(NSNumber(value: UInt64.max)))
  }

  func testStrictUInt64PreservesUnsignedBounds() {
    XCTAssertEqual(SharedNumber.strictUInt64(NSNumber(value: UInt64.max)), UInt64.max)
    XCTAssertEqual(SharedNumber.strictUInt64(NSNumber(value: 0)), 0)
    XCTAssertNil(SharedNumber.strictUInt64(NSNumber(value: true)))
    XCTAssertNil(SharedNumber.strictUInt64(NSNumber(value: -1)))
    XCTAssertNil(SharedNumber.strictUInt64(NSNumber(value: 2.5)))
    XCTAssertNil(SharedNumber.strictUInt64(NSDecimalNumber(string: "18446744073709551616")))
  }
}

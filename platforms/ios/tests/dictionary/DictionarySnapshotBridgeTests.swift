import Foundation
import XCTest

final class DictionarySnapshotBridgeTests: XCTestCase {
  func testHandleMetadataRequiresAnUnsignedInteger() {
    XCTAssertEqual(DictionarySnapshotBridge.unsignedIntegerValue(NSNumber(value: 17)), 17)
    XCTAssertEqual(DictionarySnapshotBridge.unsignedIntegerValue(NSNumber(value: 17.0)), 17)
    XCTAssertNil(DictionarySnapshotBridge.unsignedIntegerValue(NSNumber(value: 17.5)))
    XCTAssertNil(DictionarySnapshotBridge.unsignedIntegerValue(NSNumber(value: true)))
    XCTAssertNil(DictionarySnapshotBridge.unsignedIntegerValue(NSNumber(value: -1)))
  }
}

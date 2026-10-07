import XCTest

final class InputBridgeResponseTests: XCTestCase {
  func testMalformedEnvelopeReleasesNativeResponseOnce() throws {
    let pointer = strdup("[]")!
    var releases = 0
    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.decodeResponse(pointer) { pointer in
      releases += 1
      free(pointer)
    })
    XCTAssertEqual(releases, 1)
  }
}

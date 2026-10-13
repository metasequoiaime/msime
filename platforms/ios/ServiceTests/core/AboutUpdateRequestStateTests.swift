import XCTest

final class AboutUpdateRequestStateTests: XCTestCase {
  func testLeavingPageDiscardsOldResultAndAllowsNewCheck() throws {
    var state = AboutUpdateRequestState()
    let prefetch = state.currentID
    let oldRequest = try XCTUnwrap(state.begin())
    XCTAssertFalse(state.isCurrent(prefetch))
    XCTAssertTrue(state.inProgress)
    XCTAssertNil(state.begin())

    state.invalidate()
    XCTAssertFalse(state.inProgress)
    XCTAssertFalse(state.isCurrent(oldRequest))
    XCTAssertFalse(state.finish(oldRequest))

    let newRequest = try XCTUnwrap(state.begin())
    XCTAssertNotEqual(oldRequest, newRequest)
    XCTAssertTrue(state.isCurrent(newRequest))
    XCTAssertFalse(state.finish(oldRequest))
    XCTAssertTrue(state.finish(newRequest))
    XCTAssertFalse(state.inProgress)
  }
}

import XCTest

final class CommonPhrasesRequestStateTests: XCTestCase {
  func testMutationInvalidatesOlderReload() {
    var state = CommonPhrasesRequestState()
    guard let oldRead = state.beginReload() else { return XCTFail("首次读取未启动") }
    XCTAssertTrue(state.beginMutation())
    XCTAssertTrue(state.busy)
    XCTAssertNil(state.beginReload())
    state.finishMutation()
    XCTAssertFalse(state.busy)
    XCTAssertFalse(state.acceptReload(oldRead))
    guard let currentRead = state.beginReload() else { return XCTFail("写入后读取未启动") }
    XCTAssertTrue(state.acceptReload(currentRead))
  }

  func testLaterReloadWinsWhenAnswersArriveOutOfOrder() {
    var state = CommonPhrasesRequestState()
    guard let first = state.beginReload(), let second = state.beginReload() else {
      return XCTFail("连续读取未启动")
    }
    XCTAssertFalse(state.acceptReload(first))
    XCTAssertTrue(state.acceptReload(second))
  }

  func testInvalidationClearsBusyAndRejectsInFlightRequests() {
    var state = CommonPhrasesRequestState()
    guard let oldRead = state.beginReload() else { return XCTFail("读取未启动") }
    XCTAssertTrue(state.beginMutation())

    state.invalidate()

    XCTAssertFalse(state.busy)
    XCTAssertFalse(state.acceptReload(oldRead))
    guard let currentRead = state.beginReload() else { return XCTFail("失效后读取未启动") }
    XCTAssertTrue(state.acceptReload(currentRead))
  }
}

import XCTest

final class WubiLookupRequestStateTests: XCTestCase {
  private func answer(_ text: String) -> WubiCodeLookup {
    WubiCodeLookup(phrase: nil, characters: [.init(text: text, wubi86: ["ab"], wubi98: ["cd"])],
                   truncated: false)
  }

  @objc func testEditingClearsResultAndLetsNewRequestWin() {
    var state = WubiLookupRequestState()
    state.edit("甲")
    guard let first = state.begin() else { return XCTFail("首次查询未启动") }
    XCTAssertTrue(state.busy)

    state.edit("乙")
    XCTAssertNil(state.result)
    XCTAssertFalse(state.busy)
    guard let second = state.begin() else { return XCTFail("编辑后新查询未启动") }
    XCTAssertNotEqual(first, second)
    XCTAssertTrue(state.complete(second, answer: answer("乙")))
    XCTAssertFalse(state.complete(first, answer: answer("甲")))
    XCTAssertEqual(state.result?.characters.first?.text, "乙")
  }

  @objc func testEditingClearsErrorAndRejectsEarlierFailure() {
    var state = WubiLookupRequestState()
    state.edit("甲")
    guard let first = state.begin() else { return XCTFail("首次查询未启动") }
    XCTAssertTrue(state.complete(first, answer: nil))
    XCTAssertTrue(state.failed)

    state.edit("乙")
    XCTAssertFalse(state.failed)
    guard let second = state.begin() else { return XCTFail("编辑后新查询未启动") }
    XCTAssertFalse(state.complete(first, answer: nil))
    XCTAssertTrue(state.complete(second, answer: answer("乙")))
    XCTAssertFalse(state.failed)
  }
}

import XCTest

final class KeyboardChatHistoryPolicyTests: XCTestCase {
  func testRetainsSharedChatContractWindow() {
    let messages = (0..<16).map { index in (role: "user", text: "消息\(index)") }
    let result = KeyboardChatHistoryPolicy.bounded(messages)
    XCTAssertEqual(result.count, 16)
    XCTAssertEqual(result.first?.text, "消息0")
    XCTAssertEqual(result.last?.text, "消息15")
  }

  func testDropsOldestCompleteMessagesWhenByteBudgetIsReached() {
    let messages = (0..<16).map { _ in (role: "user", text: String(repeating: "字", count: 4_000)) }
    let result = KeyboardChatHistoryPolicy.bounded(messages)
    XCTAssertEqual(result.count, 5)
  }
}

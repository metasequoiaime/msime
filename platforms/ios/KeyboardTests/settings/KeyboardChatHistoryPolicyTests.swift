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

  func testRequestLeadsWithSystemPromptAndLeavesRoomForIt() {
    let messages = (0..<20).map { index in (role: "user", text: "消息\(index)") }
    let result = KeyboardChatHistoryPolicy.request(messages)
    XCTAssertEqual(result.count, KeyboardChatHistoryPolicy.maxMessages)
    XCTAssertEqual(result.first?.role, "system")
    XCTAssertEqual(result.first?.text, KeyboardChatHistoryPolicy.systemPrompt)
    XCTAssertEqual(result.last?.text, "消息19")
  }

  func testRequestByteBudgetCountsTheSystemPrompt() {
    let messages = (0..<16).map { _ in (role: "user", text: String(repeating: "字", count: 4_000)) }
    let result = KeyboardChatHistoryPolicy.request(messages)
    XCTAssertLessThanOrEqual(result.reduce(0) { $0 + $1.text.utf8.count }, KeyboardChatHistoryPolicy.maxContentBytes)
    // The system prompt plus the five newest 12 KB messages that still fit beside it.
    XCTAssertEqual(result.count, 6)
  }
}

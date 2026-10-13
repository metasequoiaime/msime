import XCTest

final class CommunityUnseenItemsTests: XCTestCase {
  private struct Item: Identifiable {
    let id: String
  }

  func testExcludesExistingIDsAndKeepsIncomingOrder() {
    let existing = [Item(id: "a"), Item(id: "c")]
    let incoming = [Item(id: "b"), Item(id: "a"), Item(id: "b"), Item(id: "d")]

    let result = CommunityUnseenItems.filter(existing: existing, incoming: incoming)

    XCTAssertEqual(result.map(\.id), ["b", "b", "d"])
  }
}

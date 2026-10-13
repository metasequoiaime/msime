import XCTest

final class ViewRequestGenerationTests: XCTestCase {
  @MainActor func testOldRequestCannotWriteAfterReplacementOrDisappearance() throws {
    var generation = ViewRequestGeneration()
    XCTAssertFalse(generation.isVisible)
    generation.appear()
    let first = generation.begin()
    XCTAssertTrue(generation.isCurrent(first))

    let second = generation.begin()
    XCTAssertFalse(generation.isCurrent(first))
    XCTAssertThrowsError(try generation.checkCurrent(first))
    XCTAssertNoThrow(try generation.checkCurrent(second))

    generation.disappear()
    XCTAssertFalse(generation.isCurrent(second))
    generation.appear()
    XCTAssertFalse(generation.isCurrent(second))
    XCTAssertThrowsError(try generation.checkCurrent(second))
    let third = generation.begin()
    XCTAssertTrue(generation.isCurrent(third))
  }
}

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

  @MainActor func testInvalidationKeepsPageVisibleForTheNextRequest() throws {
    var generation = ViewRequestGeneration()
    XCTAssertNil(generation.current())
    generation.appear()
    let first = generation.begin()
    XCTAssertEqual(generation.current(), first)

    generation.invalidate()
    XCTAssertTrue(generation.isVisible)
    XCTAssertFalse(generation.isCurrent(first))
    XCTAssertThrowsError(try generation.checkCurrent(first))
    let replacement = try XCTUnwrap(generation.current())
    XCTAssertNoThrow(try generation.checkCurrent(replacement))

    generation.disappear()
    XCTAssertNil(generation.current())
  }
}

import XCTest

final class OperationGenerationTests: XCTestCase {
  @MainActor func testCancelInvalidatesOperationAndReleasesTask() async throws {
    var generation = OperationGeneration()
    let id = generation.begin()
    let task = Task<Void, Never> { try? await Task.sleep(nanoseconds: 3_600_000_000_000) }
    var pending: Task<Void, Never>? = task

    generation.cancel(task: &pending)

    XCTAssertFalse(generation.isCurrent(id))
    XCTAssertNil(pending)
    XCTAssertTrue(task.isCancelled)
    await task.value
  }

  @MainActor func testReplacementAndInvalidationRejectOldOperations() throws {
    var generation = OperationGeneration()
    let initial = generation.current()
    XCTAssertTrue(generation.isCurrent(initial))

    let first = generation.begin()
    XCTAssertFalse(generation.isCurrent(initial))
    XCTAssertNoThrow(try generation.checkCurrent(first))

    let second = generation.begin()
    XCTAssertFalse(generation.isCurrent(first))
    XCTAssertThrowsError(try generation.checkCurrent(first))
    XCTAssertTrue(generation.isCurrent(second))

    generation.invalidate()
    XCTAssertFalse(generation.isCurrent(second))
    XCTAssertThrowsError(try generation.checkCurrent(second))
    XCTAssertNoThrow(try generation.checkCurrent(generation.current()))
  }
}

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

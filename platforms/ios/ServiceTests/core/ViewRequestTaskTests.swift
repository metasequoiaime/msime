import SwiftUI
import XCTest

@MainActor private final class RequestTaskState {
  var request = ViewRequestGeneration()
  var busy = false
  var message: String?
  var pending: Task<Void, Never>?

  func run(_ action: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(
      request: Binding(get: { self.request }, set: { self.request = $0 }),
      busy: Binding(get: { self.busy }, set: { self.busy = $0 }),
      message: Binding(get: { self.message }, set: { self.message = $0 }),
      pending: Binding(get: { self.pending }, set: { self.pending = $0 }),
      action: action)
  }

  func disappear() {
    ViewRequestTask.disappear(
      request: Binding(get: { self.request }, set: { self.request = $0 }),
      pending: Binding(get: { self.pending }, set: { self.pending = $0 }))
  }
}

private enum RequestTaskFailure: Error { case expected }

final class ViewRequestTaskTests: XCTestCase {
  @MainActor func testPollRefreshesImmediatelyAndStopsWhenCancelled() async {
    let refreshed = expectation(description: "initial refresh")
    var count = 0
    let polling = Task {
      await ViewRequestTask.poll(every: .seconds(60)) {
        count += 1
        refreshed.fulfill()
      }
    }

    await fulfillment(of: [refreshed], timeout: 2)
    polling.cancel()
    await polling.value
    XCTAssertEqual(count, 1)
  }

  @MainActor func testPollRepeatsAfterTheInterval() async {
    let refreshedTwice = expectation(description: "second refresh")
    var count = 0
    let polling = Task {
      await ViewRequestTask.poll(every: .milliseconds(10)) {
        count += 1
        if count == 2 { refreshedTwice.fulfill() }
      }
    }

    await fulfillment(of: [refreshedTwice], timeout: 2)
    polling.cancel()
    await polling.value
    XCTAssertGreaterThanOrEqual(count, 2)
  }

  @MainActor func testCurrentFailureReportsAndClearsBusyState() async throws {
    let state = RequestTaskState()
    state.request.appear()
    state.run { _ in throw RequestTaskFailure.expected }
    XCTAssertTrue(state.busy)
    let task = try XCTUnwrap(state.pending)
    await task.value
    XCTAssertFalse(state.busy)
    XCTAssertNil(state.pending)
    XCTAssertNotNil(state.message)
  }

  @MainActor func testDisappearedPageRejectsLateFailure() async throws {
    let state = RequestTaskState()
    state.request.appear()
    state.run { _ in throw RequestTaskFailure.expected }
    let task = try XCTUnwrap(state.pending)
    state.disappear()
    state.busy = false
    XCTAssertFalse(state.request.isVisible)
    XCTAssertNil(state.pending)
    XCTAssertTrue(task.isCancelled)
    await task.value
    XCTAssertNil(state.message)
    XCTAssertFalse(state.busy)
    XCTAssertNil(state.pending)
  }
}

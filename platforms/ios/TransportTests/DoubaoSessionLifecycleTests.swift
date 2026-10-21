import Foundation
import XCTest
@testable import MSIMEDoubaoTransport

final class DoubaoSessionLifecycleTests: XCTestCase {
  func testRemoteCloseInvalidatesSessionWithoutWaitingForCoordinatorCleanup() {
    checkRemoteClose(.normalClosure, cleanup: { _ in })
  }

  func testRemoteCloseStillInvalidatesSessionBeforeFinish() {
    checkRemoteClose(.goingAway, cleanup: { $0.finish() })
  }

  func testRemoteCloseStillInvalidatesSessionBeforeCancel() {
    checkRemoteClose(.policyViolation, cleanup: { $0.cancel() })
  }

  func testStaleCloseDoesNotDisconnectReplacementSession() {
    let transport = DoubaoWebSocketTransport()
    let oldInvalidated = expectation(description: "旧 session 已失效")
    let newInvalidated = expectation(description: "新 session 在显式完成后失效")
    let old = makeSession(oldInvalidated)
    let next = makeSession(newInvalidated)
    defer {
      old.session.invalidateAndCancel()
      next.session.invalidateAndCancel()
    }
    transport.urlSession(old.session, webSocketTask: old.task, didOpenWithProtocol: nil)
    transport.finish()
    transport.urlSession(next.session, webSocketTask: next.task, didOpenWithProtocol: nil)
    transport.urlSession(old.session, webSocketTask: old.task, didCloseWith: .goingAway, reason: nil)
    XCTAssertTrue(transport.isConnected, "旧连接关闭不能清除替代连接")
    transport.finish()
    wait(for: [oldInvalidated, newInvalidated], timeout: 2)
  }

  private func checkRemoteClose(_ code: URLSessionWebSocketTask.CloseCode,
                               cleanup: (DoubaoWebSocketTransport) -> Void) {
    let transport = DoubaoWebSocketTransport()
    let invalidated = expectation(description: "收到真实 URLSession 失效通知")
    let connection = makeSession(invalidated)
    defer { connection.session.invalidateAndCancel() }
    // 沿用传输层既有测试的回调入口；任务保持暂停，不连接合成地址。
    transport.urlSession(connection.session, webSocketTask: connection.task, didOpenWithProtocol: nil)
    transport.urlSession(connection.session, webSocketTask: connection.task,
      didCloseWith: code, reason: nil)
    XCTAssertFalse(transport.isConnected)
    cleanup(transport)
    wait(for: [invalidated], timeout: 2)
  }

  private func makeSession(_ invalidated: XCTestExpectation)
      -> (session: URLSession, task: URLSessionWebSocketTask) {
    let observer = InvalidationObserver(invalidated)
    let session = URLSession(configuration: .ephemeral, delegate: observer, delegateQueue: nil)
    let task = session.webSocketTask(with: URL(string: "wss://synthetic.invalid/asr")!)
    return (session, task)
  }

  private final class InvalidationObserver: NSObject, URLSessionDelegate {
    let invalidated: XCTestExpectation

    init(_ invalidated: XCTestExpectation) { self.invalidated = invalidated }

    func urlSession(_ session: URLSession, didBecomeInvalidWithError error: Error?) {
      invalidated.fulfill()
    }
  }
}

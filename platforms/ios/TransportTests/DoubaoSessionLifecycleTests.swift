import Foundation
import Network
import XCTest
@testable import MSIMEDoubaoTransport

final class DoubaoSessionLifecycleTests: XCTestCase {
  func testRemoteCloseReleasesTransportWithoutWaitingForCoordinatorCleanup() async throws {
    try await checkRemoteClose(.protocolCode(.normalClosure), cleanup: { _ in })
  }

  func testRemoteCloseStillReleasesTransportBeforeFinish() async throws {
    try await checkRemoteClose(.protocolCode(.goingAway), cleanup: { $0.finish() })
  }

  func testRemoteCloseStillReleasesTransportBeforeCancel() async throws {
    try await checkRemoteClose(.protocolCode(.policyViolation), cleanup: { $0.cancel() })
  }

  func testUnknownCloseDoesNotDisconnectReplacementSession() async throws {
    let first = try await makeLoopbackServer()
    let next = try await makeLoopbackServer()
    let transport = DoubaoWebSocketTransport()
    let oldSession = URLSession(configuration: .ephemeral)
    let oldTask = oldSession.webSocketTask(with: URL(string: "wss://synthetic.invalid/asr")!)
    defer {
      transport.finish()
      first.stop()
      next.stop()
      oldSession.invalidateAndCancel()
    }
    try await connect(transport, to: first)
    transport.finish()
    try await connect(transport, to: next)
    transport.urlSession(oldSession, webSocketTask: oldTask, didCloseWith: .goingAway, reason: nil)
    XCTAssertTrue(transport.isConnected, "未知连接关闭不能清除替代连接")
    let frame = Data("replacement close guard".utf8)
    try await transport.send(binary: frame)
    let reply = try await transport.receive()
    XCTAssertEqual(reply, frame)
  }

  private func checkRemoteClose(_ code: NWProtocolWebSocket.CloseCode,
                               cleanup: (DoubaoWebSocketTransport) -> Void) async throws {
    let server = try await makeLoopbackServer()
    var transport: DoubaoWebSocketTransport? = DoubaoWebSocketTransport()
    defer { transport?.finish(); server.stop() }
    try await connect(transport!, to: server)
    weak var released = transport
    await closeRemotely(transport!, server: server, code: code)
    cleanup(transport!)
    transport = nil
    // Foundation 失效通知之后仍可被内部收尾保留约 5 秒；继续按 ARC 条件等待，给调度留出余量。
    await waitForTransport("远端关闭后 transport 的 ARC 引用归零", timeout: 15) { released == nil }
  }
}

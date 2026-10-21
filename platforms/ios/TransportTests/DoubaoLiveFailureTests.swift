import Foundation
import XCTest
@testable import MSIMEDoubaoTransport

final class DoubaoLiveFailureTests: XCTestCase {
  func testReceiveFailureCancelsOpenRecordingStream() async throws {
    let received = expectation(description: "接收任务开始")
    let cancelled = expectation(description: "停止消费尚未结束的录音流")
    let transport = ControlledTransport(mode: .receiveFailure, received: received)
    var continuation: AsyncStream<Data>.Continuation!
    let pcm = AsyncStream<Data> { continuation = $0 }
    continuation.onTermination = { if case .cancelled = $0 { cancelled.fulfill() } }
    await checkFailure(transport, pcm: pcm, expected: .receiveFailure, afterStart: {
      await self.fulfillment(of: [received], timeout: 2)
    }, cleanup: { continuation.finish() })
    await fulfillment(of: [cancelled], timeout: 2)
  }

  func testAudioSendFailureClosesPendingReceiveBeforeDrainingGroup() async throws {
    let received = expectation(description: "接收正在等待外部传输回调")
    let transport = ControlledTransport(mode: .sendFailure, received: received)
    var continuation: AsyncStream<Data>.Continuation!
    let pcm = AsyncStream<Data> { continuation = $0 }
    await checkFailure(transport, pcm: pcm, expected: .sendFailure, afterStart: {
      await self.fulfillment(of: [received], timeout: 2)
      continuation.yield(Data(repeating: 0x2A, count: 6401))
    }, cleanup: { continuation.finish() })
  }

  func testSuccessfulLiveRunSendsFinalAudioAndAppliesText() async throws {
    let received = expectation(description: "接收任务开始")
    let transport = ControlledTransport(mode: .success, received: received)
    let pcm = AsyncStream<Data> {
      $0.yield(Data(repeating: 0x2A, count: 6401))
      $0.finish()
    }
    var applied: [(String, UInt64)] = []
    let coordinator = DoubaoVoiceCoordinator(transport: transport, applyText: { applied.append(($0, $1)) }, decodeFrame: { _ in nil })
    try await coordinator.runLive(endpoint: endpoint, handshake: handshake(), generation: 42, pcm: pcm, codec: codec)
    XCTAssertEqual(transport.sentFrames, [Data([1]), Data([2]), Data([0xFD])])
    XCTAssertEqual(applied.map(\.0), ["合成识别文本"])
    XCTAssertEqual(applied.map(\.1), [42])
    XCTAssertTrue(transport.didFinish)
    await fulfillment(of: [received], timeout: 2)
  }

  private func checkFailure(_ transport: ControlledTransport, pcm: AsyncStream<Data>, expected: TestFailure,
                            afterStart: () async -> Void, cleanup: () -> Void) async {
    let returned = expectation(description: "无需外部停止录音或接收即可返回首个错误")
    let coordinator = DoubaoVoiceCoordinator(transport: transport, applyText: { _, _ in XCTFail("失败不应上屏") }, decodeFrame: { _ in nil })
    let worker = Task {
      do {
        try await coordinator.runLive(endpoint: endpoint, handshake: handshake(), generation: 7, pcm: pcm, codec: codec)
        XCTFail("失败应向上传递")
      } catch {
        XCTAssertEqual(error as? TestFailure, expected)
      }
      returned.fulfill()
    }
    await afterStart()
    await fulfillment(of: [returned], timeout: 2)
    XCTAssertTrue(transport.didFinish, "组内任务排空前必须关闭传输")
    // 失败断言后仍解除所有等待，避免旧实现在测试进程里留下任务。
    worker.cancel()
    transport.finish()
    cleanup()
    await worker.value
  }

  private var endpoint: URL { URL(string: "wss://synthetic.invalid/asr")! }
  private func handshake() throws -> DoubaoHandshake {
    try DoubaoHandshake(appKey: "fixture-app", accessKey: "fixture-key", resourceID: "fixture-resource")
  }
  private var codec: DoubaoVoiceCoordinator.FrameCodec {
    .init(startFrame: { Data([1]) }, audioFrame: { sequence, _, _ in Data([UInt8(truncatingIfNeeded: sequence)]) },
          decodeFrame: { _ in (true, "合成识别文本") })
  }

  private enum TestFailure: Error { case receiveFailure, sendFailure, closed }
  private final class ControlledTransport: DoubaoVoiceTransport {
    enum Mode { case receiveFailure, sendFailure, success }
    private let mode: Mode
    private let received: XCTestExpectation
    private let lock = NSLock()
    private var pendingReceive: CheckedContinuation<Data, Error>?
    private var finalSent = false
    private var frames: [Data] = []
    private var finished = false
    var sentFrames: [Data] { lock.withLock { frames } }
    var didFinish: Bool { lock.withLock { finished } }

    init(mode: Mode, received: XCTestExpectation) { self.mode = mode; self.received = received }
    func start(endpoint: URL) async throws {}
    func start(endpoint: URL, handshake: DoubaoHandshake) async throws {}
    func send(binary frame: Data) async throws {
      let pending = try lock.withLock { () throws -> CheckedContinuation<Data, Error>? in
        frames.append(frame)
        if mode == .sendFailure && frames.count > 1 { throw TestFailure.sendFailure }
        guard frame.first == 0xFD else { return nil }
        finalSent = true
        let pending = pendingReceive
        pendingReceive = nil
        return pending
      }
      pending?.resume(returning: Data([0xFF]))
    }
    func receive() async throws -> Data {
      if mode == .receiveFailure { received.fulfill(); throw TestFailure.receiveFailure }
      // 模拟由外部回调恢复的传输；Task 取消标记本身不能恢复 continuation。
      return try await withCheckedThrowingContinuation { continuation in
        let ready = lock.withLock {
          if finalSent { return true }
          pendingReceive = continuation
          return false
        }
        received.fulfill()
        if ready { continuation.resume(returning: Data([0xFF])) }
      }
    }
    func finish() {
      let pending = lock.withLock {
        finished = true
        let pending = pendingReceive
        pendingReceive = nil
        return pending
      }
      pending?.resume(throwing: TestFailure.closed)
    }
  }
}

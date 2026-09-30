import Foundation
import XCTest
@testable import MSIMEDoubaoTransport

final class DoubaoVoiceCoordinatorTests: XCTestCase {
  func testTransportRefusesWebSocketRedirects() {
    let transport = DoubaoWebSocketTransport()
    let response = HTTPURLResponse(
      url: URL(string: "https://openspeech.example.invalid")!,
      statusCode: 302,
      httpVersion: "HTTP/1.1",
      headerFields: ["Location": "wss://untrusted.example.invalid"]
    )!
    var redirected: URLRequest?
    let session = URLSession(configuration: .ephemeral)
    let task = session.dataTask(with: URL(string: "wss://openspeech.example.invalid")!)
    transport.urlSession(session, task: task,
      willPerformHTTPRedirection: response,
      newRequest: URLRequest(url: URL(string: "wss://untrusted.example.invalid")!),
      completionHandler: { redirected = $0 })
    XCTAssertNil(redirected)
  }

  func testTransportCanStartAgainAfterRemoteClose() async throws {
    let transport = DoubaoWebSocketTransport()
    let session = URLSession(configuration: .ephemeral)
    let task = session.webSocketTask(with: URL(string: "ws://127.0.0.1:1")!)
    transport.urlSession(session, webSocketTask: task, didOpenWithProtocol: nil)
    transport.urlSession(session, webSocketTask: task, didCloseWith: .goingAway, reason: nil)

    do {
      try await transport.start(endpoint: URL(string: "ws://127.0.0.1:1")!)
      XCTFail("a new connection should be attempted after the old socket closes")
    } catch {
      XCTAssertFalse(transport.isConnected)
    }
  }

  func testCodecRunSendsWindowsSizedAudioAndAppliesFinalTextGeneration() async throws {
    let transport = FakeTransport(incoming: [Data([0xFF])])
    let packets = PacketRecorder()
    let coordinator = DoubaoVoiceCoordinator(
      transport: transport,
      applyText: { packets.applied.append(($0, $1)) },
      decodeFrame: { _ in nil }
    )
    let codec = DoubaoVoiceCoordinator.FrameCodec(
      startFrame: { Data([0x01]) },
      audioFrame: { sequence, pcm, final in
        packets.frames.append((sequence, Data(pcm), final))
        return Data([UInt8(truncatingIfNeeded: sequence)])
      },
      decodeFrame: { frame in frame == Data([0xFF]) ? (true, "识别结果") : nil }
    )

    try await coordinator.run(
      endpoint: URL(string: "wss://example.invalid/asr")!,
      handshake: try DoubaoHandshake(appKey: "fixture-app", accessKey: "fixture-access", resourceID: "fixture-resource"),
      generation: 42,
      pcm: Data(repeating: 0x2A, count: 12_801),
      codec: codec
    )

    XCTAssertEqual(transport.handshakeStarts, 1)
    XCTAssertEqual(transport.sent, [Data([0x01]), Data([2]), Data([3]), Data([0xFC])])
    XCTAssertEqual(packets.frames.map(\.0), [2, 3, -4])
    XCTAssertEqual(packets.frames.map { $0.1.count }, [6400, 6400, 1])
    XCTAssertEqual(packets.frames.map(\.2), [false, false, true])
    XCTAssertEqual(packets.applied.map(\.0), ["识别结果"])
    XCTAssertEqual(packets.applied.map(\.1), [42])
    XCTAssertTrue(transport.didFinish)
  }

  func testExactChunkUsesNegativeFinalSequence() async throws {
    let transport = FakeTransport(incoming: [Data([0xFF])])
    let packets = PacketRecorder()
    let coordinator = DoubaoVoiceCoordinator(
      transport: transport,
      applyText: { _, _ in },
      decodeFrame: { frame in frame == Data([0xFF]) ? (true, nil) : nil }
    )
    let codec = DoubaoVoiceCoordinator.FrameCodec(
      startFrame: { Data([0x01]) },
      audioFrame: { sequence, pcm, final in
        packets.frames.append((sequence, Data(pcm), final))
        return Data()
      },
      decodeFrame: { frame in frame == Data([0xFF]) ? (true, nil) : nil }
    )

    try await coordinator.run(
      endpoint: URL(string: "wss://example.invalid/asr")!,
      handshake: try DoubaoHandshake(appKey: "fixture-app", accessKey: "fixture-access", resourceID: "fixture-resource"),
      generation: 7,
      pcm: Data(repeating: 0, count: DoubaoVoiceCoordinator.pcmChunkBytes),
      codec: codec
    )

    XCTAssertEqual(packets.frames.map(\.0), [-2])
    XCTAssertEqual(packets.frames.map(\.2), [true])
  }

  func testClientReturnsLatestTextThroughInjectedCodec() async throws {
    let transport = FakeTransport(incoming: [Data([0xFF])])
    let codec = DoubaoVoiceCoordinator.FrameCodec(
      startFrame: { Data([0x01]) },
      audioFrame: { _, _, _ in Data([0x02]) },
      decodeFrame: { frame in frame == Data([0xFF]) ? (true, "fixture transcript") : nil }
    )
    let client = DoubaoVoiceClient(transport: transport, codec: codec)
    let result = try await client.transcribe(
      endpoint: URL(string: "wss://example.invalid/asr")!,
      handshake: try DoubaoHandshake(appKey: "fixture-app", accessKey: "fixture-access", resourceID: "fixture-resource"),
      generation: 9,
      pcm: Data([0x2A])
    )

    XCTAssertEqual(result, "fixture transcript")
    XCTAssertEqual(transport.sent, [Data([0x01]), Data([0x02])])
    XCTAssertTrue(transport.didFinish)
  }

  private final class PacketRecorder {
    var frames: [(Int32, Data, Bool)] = []
    var applied: [(String, UInt64)] = []
  }

  private final class FakeTransport: DoubaoVoiceTransport {
    let incoming: [Data]
    var sent: [Data] = []
    var handshakeStarts = 0
    var didFinish = false

    init(incoming: [Data]) { self.incoming = incoming }

    func start(endpoint: URL) async throws {}

    func start(endpoint: URL, handshake: DoubaoHandshake) async throws {
      handshakeStarts += 1
    }

    func send(binary frame: Data) async throws { sent.append(frame) }

    func receive() async throws -> Data {
      guard !incoming.isEmpty else { throw FakeFailure.noIncomingFrame }
      return incoming[0]
    }

    func finish() { didFinish = true }
  }

  private enum FakeFailure: Error { case noIncomingFrame }
}

import Foundation

protocol DoubaoVoiceTransport: AnyObject {
  func start(endpoint: URL) async throws
  func start(endpoint: URL, handshake: DoubaoHandshake) async throws
  func send(binary frame: Data) async throws
  func receive() async throws -> Data
  func finish()
}

/// Coordinates a host transport with generation-checked voice text application.
final class DoubaoVoiceCoordinator {
  static let pcmChunkBytes = 6400
  static let maximumResponseFrameBytes = 1_048_576
  enum Failure: Error { case responseTooLarge }
  typealias ApplyText = (_ text: String, _ generation: UInt64) -> Void
  typealias DecodeFrame = (_ frame: Data) -> (isFinal: Bool, text: String?)?
  typealias AudioFrameBuilder = (_ sequence: Int32, _ pcm: Data, _ final: Bool) -> Data

  struct FrameCodec {
    let startFrame: () throws -> Data
    let audioFrame: (_ sequence: Int32, _ pcm: Data, _ final: Bool) throws -> Data
    let decodeFrame: DecodeFrame

    init(startFrame: @escaping () throws -> Data,
         audioFrame: @escaping (_ sequence: Int32, _ pcm: Data, _ final: Bool) throws -> Data,
         decodeFrame: @escaping DecodeFrame) {
      self.startFrame = startFrame
      self.audioFrame = audioFrame
      self.decodeFrame = decodeFrame
    }
  }

  private let transport: DoubaoVoiceTransport
  private let applyText: ApplyText
  private let decodeFrame: DecodeFrame

  init(transport: DoubaoVoiceTransport, applyText: @escaping ApplyText,
       decodeFrame: @escaping DecodeFrame) {
    self.transport = transport
    self.applyText = applyText
    self.decodeFrame = decodeFrame
  }

  func run(endpoint: URL, generation: UInt64, audioFrames: [Data]) async throws {
    try await transport.start(endpoint: endpoint)
    try await receiveAndApply(generation: generation, audioFrames: audioFrames)
  }

  func run(endpoint: URL, handshake: DoubaoHandshake, generation: UInt64,
           audioFrames: [Data]) async throws {
    try await transport.start(endpoint: endpoint, handshake: handshake)
    try await receiveAndApply(generation: generation, audioFrames: audioFrames)
  }

  /// Send one complete PCM recording using the Windows 200 ms packet cadence.
  /// The caller supplies the start frame and the platform bridge's frame builder.
  func run(endpoint: URL, handshake: DoubaoHandshake, generation: UInt64,
           startFrame: Data, pcm: Data, buildAudioFrame: @escaping AudioFrameBuilder) async throws {
    try await transport.start(endpoint: endpoint, handshake: handshake)
    defer { transport.finish() }
    try await transport.send(binary: startFrame)
    try await sendAudio(pcm: pcm, buildAudioFrame: buildAudioFrame)
    try await receiveUntilFinal(generation: generation)
  }

  /// Runs a complete PCM recording using a host-injected codec. The codec is
  /// normally backed by `MSIMEClientSession` and therefore keeps wire layout
  /// ownership in client-core instead of duplicating it in Swift.
  func run(endpoint: URL, handshake: DoubaoHandshake, generation: UInt64,
           pcm: Data, codec: FrameCodec) async throws {
    try await transport.start(endpoint: endpoint, handshake: handshake)
    defer { transport.finish() }
    try await transport.send(binary: codec.startFrame())
    try await sendAudio(pcm: pcm) { sequence, chunk, final in
      try codec.audioFrame(sequence, chunk, final)
    }
    try await receiveUntilFinal(generation: generation, decode: codec.decodeFrame)
  }

  /// Streams a recording while it is still being made, as the Windows host does with `stream_inline_preedit`: each 200 ms of PCM goes out as it is captured, and every partial result reaches `applyText` while the user is still speaking. The stream ending sends the last packet, and this returns once the final result is in.
  func runLive(endpoint: URL, handshake: DoubaoHandshake, generation: UInt64,
               pcm: AsyncStream<Data>, codec: FrameCodec) async throws {
    try await transport.start(endpoint: endpoint, handshake: handshake)
    defer { transport.finish() }
    try await transport.send(binary: codec.startFrame())
    try await withThrowingTaskGroup(of: Void.self) { group in
      group.addTask { try await self.sendLive(pcm, buildAudioFrame: codec.audioFrame) }
      group.addTask { try await self.receiveUntilFinal(generation: generation, decode: codec.decodeFrame) }
      try await group.waitForAll()
    }
  }

  private func sendLive(_ pcm: AsyncStream<Data>, buildAudioFrame: (_ sequence: Int32, _ pcm: Data, _ final: Bool) throws -> Data) async throws {
    var pending = Data()
    var sequence: Int32 = 2
    for await chunk in pcm {
      pending.append(chunk)
      while pending.count > Self.pcmChunkBytes {
        try await transport.send(binary: try buildAudioFrame(sequence, Data(pending.prefix(Self.pcmChunkBytes)), false))
        pending = Data(pending.dropFirst(Self.pcmChunkBytes))
        sequence += 1
      }
    }
    // A cancelled stream also ends the loop; only a finished recording gets the last packet.
    try Task.checkCancellation()
    try await transport.send(binary: try buildAudioFrame(-sequence, pending, true))
  }

  private func receiveAndApply(generation: UInt64, audioFrames: [Data]) async throws {
    defer { transport.finish() }
    for frame in audioFrames { try await transport.send(binary: frame) }
    try await receiveUntilFinal(generation: generation)
  }

  private func sendAudio(pcm: Data, buildAudioFrame: (_ sequence: Int32, _ pcm: Data, _ final: Bool) throws -> Data) async throws {
    var pending = pcm
    var sequence: Int32 = 2
    while pending.count > Self.pcmChunkBytes {
      try await transport.send(binary: try buildAudioFrame(sequence, pending.prefix(Self.pcmChunkBytes), false))
      pending.removeFirst(Self.pcmChunkBytes)
      sequence += 1
    }
    try await transport.send(binary: try buildAudioFrame(-sequence, pending, true))
  }

  private func receiveUntilFinal(generation: UInt64) async throws {
    try await receiveUntilFinal(generation: generation, decode: decodeFrame)
  }

  private func receiveUntilFinal(generation: UInt64, decode: @escaping DecodeFrame) async throws {
    while true {
      let frame = try await transport.receive()
      guard frame.count <= Self.maximumResponseFrameBytes else { throw Failure.responseTooLarge }
      guard let response = decode(frame) else { continue }
      if let text = response.text, !text.isEmpty { applyText(text, generation) }
      if response.isFinal { return }
    }
  }
}

/// Host-injected Doubao transport and frame codec. The wire layout remains in
/// client-core; the app only supplies the platform transport and codec bridge.
final class DoubaoVoiceClient: @unchecked Sendable {
  enum Failure: Error { case emptyTranscript }

  private let transport: DoubaoVoiceTransport
  private let codec: DoubaoVoiceCoordinator.FrameCodec

  init(transport: DoubaoVoiceTransport, codec: DoubaoVoiceCoordinator.FrameCodec) {
    self.transport = transport
    self.codec = codec
  }

  func transcribe(endpoint: URL, handshake: DoubaoHandshake, generation: UInt64,
                  pcm: Data) async throws -> String {
    var result = ""
    let coordinator = DoubaoVoiceCoordinator(
      transport: transport,
      applyText: { text, appliedGeneration in
        guard appliedGeneration == generation else { return }
        result = text
      },
      decodeFrame: codec.decodeFrame
    )
    try await coordinator.run(endpoint: endpoint, handshake: handshake,
                              generation: generation, pcm: pcm, codec: codec)
    guard !result.isEmpty else { throw Failure.emptyTranscript }
    return result
  }

  /// `transcribe` for a recording still in progress; `partial` sees each result as it arrives, on the socket's thread.
  func transcribeLive(endpoint: URL, handshake: DoubaoHandshake, generation: UInt64,
                      pcm: AsyncStream<Data>, partial: @escaping (String) -> Void) async throws -> String {
    var result = ""
    let coordinator = DoubaoVoiceCoordinator(
      transport: transport,
      applyText: { text, appliedGeneration in
        guard appliedGeneration == generation else { return }
        result = text
        partial(text)
      },
      decodeFrame: codec.decodeFrame
    )
    try await coordinator.runLive(endpoint: endpoint, handshake: handshake,
                                  generation: generation, pcm: pcm, codec: codec)
    guard !result.isEmpty else { throw Failure.emptyTranscript }
    return result
  }
}

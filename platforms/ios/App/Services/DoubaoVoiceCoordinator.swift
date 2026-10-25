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
  enum Failure: Error {
    case responseTooLarge
    /// 豆包发来的错误帧，只带它的数字错误码；错误帧后面的文字不解析。
    case service(code: Int64)
  }
  typealias ApplyText = (_ text: String, _ generation: UInt64) -> Void
  /// 返回 nil 的帧被跳过；错误帧由编解码器抛出 `Failure.service`。
  typealias DecodeFrame = (_ frame: Data) throws -> (isFinal: Bool, text: String?)?
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

  /// 用宿主注入的编解码器跑完一段完整的 PCM 录音。编解码器通常由 `MSIMEClientSession` 提供，因此线上帧格式仍归 client-providers 所有，不在 Swift 里重复一份。
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
      do {
        while try await group.next() != nil {}
      } catch {
        // 先关闭传输，恢复仍在等待外部回调的收发任务，再让任务组取消并排空。
        transport.finish()
        throw error
      }
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
      guard let response = try decode(frame) else { continue }
      if let text = response.text, !text.isEmpty { applyText(text, generation) }
      if response.isFinal { return }
    }
  }
}

/// 宿主注入的豆包传输层和帧编解码器。线上帧格式仍在 client-providers 里；应用只提供平台传输层和编解码器桥接。
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

/// 豆包握手被拒和错误帧的一句话，与共享 Rust 的 `credential::doubao::failure_message` 逐字相同：带上 HTTP 状态码或数字错误码，不带服务端正文。设置页的「测试连接」和听写失败都用它。
enum DoubaoFailureMessage {
  static func rejected(status: Int) -> String {
    switch status {
    case 401: return "豆包认证失败（HTTP 401），请确认密钥来自豆包语音控制台，且鉴权方式与之对应。"
    case 403: return "豆包拒绝访问（HTTP 403），请检查密钥的权限，以及资源 ID 对应的服务是否已开通。"
    case 429: return "豆包限流或额度不足（HTTP 429），请稍后重试或检查资源用量。"
    case 500...599: return "豆包服务暂时不可用（HTTP \(status)），请稍后重试。"
    default: return "豆包拒绝了连接（HTTP \(status)），请检查接口地址和资源 ID。"
    }
  }

  /// 《大模型流式语音识别 API》的错误码表：450xxxxx 是请求本身的问题，550xxxxx 是服务内部错误（55000031 是服务过载），后者改配置没有用。
  static func service(code: Int64) -> String {
    if (55_000_000...55_099_999).contains(code) { return "豆包服务暂时不可用（错误码 \(code)），请稍后重试。" }
    return "豆包返回错误码 \(code)，请检查资源 ID 和识别选项。"
  }

  static let connection = "无法连接豆包或连接中断，请检查接口地址和网络。"
  static let timeout = "豆包测试超时，请检查网络后重试。"
}

extension DoubaoVoiceCoordinator.Failure: LocalizedError {
  var errorDescription: String? {
    switch self {
    case .responseTooLarge: return nil
    case .service(let code): return DoubaoFailureMessage.service(code: code)
    }
  }
}

extension DoubaoWebSocketTransport.Failure: LocalizedError {
  var errorDescription: String? {
    switch self {
    case .rejected(let status): return DoubaoFailureMessage.rejected(status: status)
    case .notConnected, .closed: return nil
    }
  }
}

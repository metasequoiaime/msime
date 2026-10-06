import Foundation

private typealias MSIMEVoiceByte = UInt8

@_silgen_name("msime_client_doubao_decode_frame")
private func msimeClientDoubaoDecodeFrame(
  _ frame: UnsafePointer<MSIMEVoiceByte>?, _ length: UInt
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_doubao_start_frame")
private func msimeClientDoubaoStartFrame(
  _ enableITN: Bool, _ enablePunctuation: Bool, _ enableDDC: Bool,
  _ boostingTable: UnsafePointer<MSIMEVoiceByte>?, _ boostingTableLength: UInt,
  _ output: UnsafeMutablePointer<MSIMEVoiceByte>?, _ outputCapacity: UInt,
  _ outputLength: UnsafeMutablePointer<UInt>?
) -> Bool

@_silgen_name("msime_client_doubao_audio_frame")
private func msimeClientDoubaoAudioFrame(
  _ sequence: Int32, _ pcm: UnsafePointer<MSIMEVoiceByte>?, _ pcmLength: UInt,
  _ finalChunk: Bool, _ output: UnsafeMutablePointer<MSIMEVoiceByte>?,
  _ outputCapacity: UInt, _ outputLength: UnsafeMutablePointer<UInt>?
) -> Bool

@_silgen_name("msime_client_string_free")
private func msimeClientStringFree(_ value: UnsafeMutablePointer<CChar>?)

private enum IOSVoiceDoubaoFrame {
  case update(final: Bool, text: String?)
  case serviceError
}

/// Bounded WebSocket transport for one completed iOS PCM16 recording. Wire
/// frames and response decompression stay in client-core through its C ABI.
final class IOSVoiceDoubaoTransport: NSObject, URLSessionWebSocketDelegate,
    URLSessionTaskDelegate {
  private static let pcmChunkBytes = 6_400
  private static let maximumPCMBytes = 1_920_000
  private static let maximumFrameBytes = 1_048_576
  private static let maximumResponseBytes = 4 * 1_048_576
  private static let maximumMessages = 256
  private static let maximumTextCharacters = 10_000
  private static let headerNames = Set([
    "x-api-key", "x-api-app-key", "x-api-access-key",
    "x-api-resource-id", "x-api-request-id",
  ])

  private let request: URLRequest
  private var pcm: Data
  private let startFrame: Data
  private var completion: ((Result<String, VoicePluginFailure>) -> Void)?
  private var session: URLSession?
  private var task: URLSessionWebSocketTask?
  private var totalResponseBytes = 0
  private var messageCount = 0
  private var offset = 0
  private var sequence: Int32 = 2
  private var finalSent = false
  private var completed = false
  private var lastText = ""
  private var totalTimeout: DispatchWorkItem?
  private var finalTimeout: DispatchWorkItem?

  init?(endpoint: String, headers: [String: String], enableITN: Bool,
        punctuation: Bool, DDC: Bool, boostingTable: String, wav: Data,
        completion: @escaping (Result<String, VoicePluginFailure>) -> Void) {
    guard let components = URLComponents(string: endpoint),
          components.scheme?.lowercased() == "wss",
          components.host?.isEmpty == false,
          components.user == nil, components.password == nil, components.fragment == nil,
          headers.count >= 3, headers.count <= 4,
          headers.keys.allSatisfy({ Self.headerNames.contains($0.lowercased()) }),
          headers.values.allSatisfy({
            !$0.isEmpty && $0.utf8.count <= 8_192 &&
              $0.unicodeScalars.allSatisfy({ $0.value < 0x80 }) &&
              !$0.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) })
          }),
          let url = components.url,
          let pcm = Self.pcm16(fromWAV: wav),
          let startFrame = Self.makeStartFrame(
            enableITN: enableITN, punctuation: punctuation, DDC: DDC,
            boostingTable: boostingTable)
    else { return nil }
    var request = URLRequest(url: url)
    request.timeoutInterval = 30
    request.cachePolicy = .reloadIgnoringLocalCacheData
    for (name, value) in headers {
      request.setValue(value, forHTTPHeaderField: name)
    }
    self.request = request
    self.pcm = pcm
    self.startFrame = startFrame
    self.completion = completion
    super.init()
  }

  func start() {
    guard !completed, task == nil else { return }
    let configuration = URLSessionConfiguration.ephemeral
    configuration.httpCookieStorage = nil
    configuration.urlCredentialStorage = nil
    configuration.urlCache = nil
    configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
    configuration.timeoutIntervalForRequest = 30
    configuration.timeoutIntervalForResource = 100
    let session = URLSession(configuration: configuration, delegate: self,
      delegateQueue: OperationQueue.main)
    let task = session.webSocketTask(with: request)
    task.maximumMessageSize = Self.maximumFrameBytes
    self.session = session
    self.task = task
    task.resume()
    receiveNext()
    sendStartFrame()
    let timeout = DispatchWorkItem { [weak self] in
      self?.finish(.failure(VoicePluginFailure(code: "voice_service")))
    }
    totalTimeout = timeout
    DispatchQueue.main.asyncAfter(deadline: .now() + 100, execute: timeout)
  }

  func cancel() {
    guard !completed else { return }
    completed = true
    completion = nil
    close(code: .goingAway)
  }

  private func sendStartFrame() {
    guard let task else {
      finish(.failure(VoicePluginFailure(code: "voice_service")))
      return
    }
    task.send(.data(startFrame)) { [weak self] error in
      DispatchQueue.main.async {
        guard let self, !self.completed else { return }
        guard error == nil else {
          self.finish(.failure(VoicePluginFailure(code: "voice_service")))
          return
        }
        self.sendNextAudioFrame()
      }
    }
  }

  private func sendNextAudioFrame() {
    guard let task, offset < pcm.count else {
      finish(.failure(VoicePluginFailure(code: "voice_recording")))
      return
    }
    let remaining = pcm.count - offset
    let final = remaining <= Self.pcmChunkBytes
    let end = final ? pcm.count : offset + Self.pcmChunkBytes
    let chunk = pcm.subdata(in: offset..<end)
    guard let frame = Self.makeAudioFrame(sequence: sequence, pcm: chunk, final: final)
    else {
      finish(.failure(VoicePluginFailure(code: "voice_recording")))
      return
    }
    task.send(.data(frame)) { [weak self] error in
      DispatchQueue.main.async {
        guard let self, !self.completed else { return }
        guard error == nil else {
          self.finish(.failure(VoicePluginFailure(code: "voice_service")))
          return
        }
        self.offset = end
        if final {
          self.finalSent = true
          self.pcm.removeAll(keepingCapacity: false)
          let timeout = DispatchWorkItem { [weak self] in
            self?.finish(.failure(VoicePluginFailure(code: "voice_service")))
          }
          self.finalTimeout = timeout
          DispatchQueue.main.asyncAfter(deadline: .now() + 30, execute: timeout)
        } else {
          self.sequence += 1
          self.sendNextAudioFrame()
        }
      }
    }
  }

  private func receiveNext() {
    guard let task, !completed else { return }
    task.receive { [weak self] result in
      DispatchQueue.main.async {
        guard let self, !self.completed else { return }
        switch result {
        case .failure:
          self.finish(.failure(VoicePluginFailure(code: "voice_service")))
        case .success(.string):
          self.finish(.failure(VoicePluginFailure(code: "voice_response")))
        case .success(.data(let frame)):
          guard frame.count <= Self.maximumFrameBytes,
                self.totalResponseBytes <= Self.maximumResponseBytes - frame.count,
                self.messageCount < Self.maximumMessages else {
            self.finish(.failure(VoicePluginFailure(code: "voice_response")))
            return
          }
          self.totalResponseBytes += frame.count
          self.messageCount += 1
          self.consume(frame)
        @unknown default:
          self.finish(.failure(VoicePluginFailure(code: "voice_response")))
        }
      }
    }
  }

  private func consume(_ frame: Data) {
    guard let response = Self.decode(frame) else {
      finish(.failure(VoicePluginFailure(code: "voice_response")))
      return
    }
    switch response {
    case .serviceError:
      finish(.failure(VoicePluginFailure(code: "voice_service")))
    case .update(let final, let text):
      if let text, !text.isEmpty {
        lastText = text
      }
      if final {
        guard finalSent, !lastText.isEmpty else {
          finish(.failure(VoicePluginFailure(code: "voice_response")))
          return
        }
        finish(.success(lastText))
      } else {
        receiveNext()
      }
    }
  }

  private func finish(_ result: Result<String, VoicePluginFailure>) {
    guard !completed else { return }
    completed = true
    let completion = completion
    self.completion = nil
    close(code: result.isSuccess ? .normalClosure : .goingAway)
    completion?(result)
  }

  private func close(code: URLSessionWebSocketTask.CloseCode) {
    totalTimeout?.cancel()
    totalTimeout = nil
    finalTimeout?.cancel()
    finalTimeout = nil
    task?.cancel(with: code, reason: nil)
    task = nil
    session?.invalidateAndCancel()
    session = nil
    pcm.removeAll(keepingCapacity: false)
  }

  func urlSession(_ session: URLSession, task: URLSessionTask,
                  willPerformHTTPRedirection response: HTTPURLResponse,
                  newRequest request: URLRequest,
                  completionHandler: @escaping (URLRequest?) -> Void) {
    completionHandler(nil)
  }

  func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                  didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
    guard !completed else { return }
    finish(.failure(VoicePluginFailure(code: "voice_service")))
  }

  private static func makeStartFrame(enableITN: Bool, punctuation: Bool, DDC: Bool,
                                     boostingTable: String) -> Data? {
    let table = Data(boostingTable.utf8)
    var output = Data(count: 65_536)
    let capacity = output.count
    var written: UInt = 0
    let ok = output.withUnsafeMutableBytes { outputBytes in
      table.withUnsafeBytes { tableBytes in
        msimeClientDoubaoStartFrame(
          enableITN, punctuation, DDC,
          tableBytes.bindMemory(to: MSIMEVoiceByte.self).baseAddress, UInt(table.count),
          outputBytes.bindMemory(to: MSIMEVoiceByte.self).baseAddress, UInt(capacity), &written)
      }
    }
    guard ok, written > 0, written <= UInt(output.count) else { return nil }
    output.removeSubrange(Int(written)..<output.count)
    return output
  }

  private static func makeAudioFrame(sequence: Int32, pcm: Data, final: Bool) -> Data? {
    var output = Data(count: pcm.count + 65_536)
    let capacity = output.count
    var written: UInt = 0
    let ok = output.withUnsafeMutableBytes { outputBytes in
      pcm.withUnsafeBytes { pcmBytes in
        msimeClientDoubaoAudioFrame(
          sequence, pcmBytes.bindMemory(to: MSIMEVoiceByte.self).baseAddress, UInt(pcm.count),
          final, outputBytes.bindMemory(to: MSIMEVoiceByte.self).baseAddress,
          UInt(capacity), &written)
      }
    }
    guard ok, written > 0, written <= UInt(output.count),
          written <= UInt(Self.maximumFrameBytes) else { return nil }
    output.removeSubrange(Int(written)..<output.count)
    return output
  }

  private static func decode(_ frame: Data) -> IOSVoiceDoubaoFrame? {
    let raw: UnsafeMutablePointer<CChar>? = frame.withUnsafeBytes { bytes in
      msimeClientDoubaoDecodeFrame(
        bytes.bindMemory(to: MSIMEVoiceByte.self).baseAddress, UInt(frame.count))
    }
    guard let raw else { return nil }
    defer { msimeClientStringFree(raw) }
    guard let data = String(cString: raw).data(using: .utf8),
          let envelope = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          envelope["ok"] as? Bool == true,
          let value = envelope["value"] as? [String: Any] else { return nil }
    if value["error_code"] != nil { return .serviceError }
    guard let final = value["last"] as? Bool,
          let payload = value["payload"] as? String,
          let payloadData = payload.data(using: .utf8),
          let body = try? JSONSerialization.jsonObject(with: payloadData) as? [String: Any]
    else { return nil }
    let result = body["result"] as? [String: Any]
    let text = (result?["text"] as? String) ?? (body["text"] as? String)
    guard text == nil || (text!.count <= Self.maximumTextCharacters &&
      !text!.unicodeScalars.contains(where: { $0.value == 0 })) else { return nil }
    return .update(final: final, text: text)
  }

  private static func pcm16(fromWAV wav: Data) -> Data? {
    let bytes = [UInt8](wav)
    guard bytes.count >= 44, bytes.count <= 2_100_000,
          Array(bytes[0..<4]) == Array("RIFF".utf8),
          Array(bytes[8..<12]) == Array("WAVE".utf8),
          let riffSize = littleEndian32(bytes, at: 4),
          riffSize <= bytes.count - 8 else { return nil }
    let end = riffSize + 8
    var cursor = 12
    var formatIsPCM16Mono = false
    var pcmRange: Range<Int>?
    while cursor <= end - 8 {
      guard let chunkSize = littleEndian32(bytes, at: cursor + 4),
            chunkSize <= end - cursor - 8 else { return nil }
      let start = cursor + 8
      let next = start + chunkSize
      guard next <= bytes.count else { return nil }
      let name = Array(bytes[cursor..<(cursor + 4)])
      if name == Array("fmt ".utf8) {
        guard chunkSize >= 16,
              let format = littleEndian16(bytes, at: start),
              let channels = littleEndian16(bytes, at: start + 2),
              let rate = littleEndian32(bytes, at: start + 4),
              let byteRate = littleEndian32(bytes, at: start + 8),
              let alignment = littleEndian16(bytes, at: start + 12),
              let bits = littleEndian16(bytes, at: start + 14) else { return nil }
        let extensiblePCM = format == 0xfffe && chunkSize >= 40 &&
          littleEndian16(bytes, at: start + 24) == 1
        formatIsPCM16Mono = (format == 1 || extensiblePCM) && channels == 1 &&
          rate == 16_000 && byteRate == 32_000 && alignment == 2 && bits == 16
      } else if name == Array("data".utf8) {
        guard chunkSize > 0, chunkSize <= Self.maximumPCMBytes, chunkSize % 2 == 0
        else { return nil }
        pcmRange = start..<next
      }
      let padding = chunkSize % 2
      guard next <= end - padding else { return nil }
      cursor = next + padding
    }
    guard formatIsPCM16Mono, let pcmRange else { return nil }
    return Data(bytes[pcmRange])
  }

  private static func littleEndian16(_ bytes: [UInt8], at offset: Int) -> Int? {
    guard offset >= 0, offset <= bytes.count - 2 else { return nil }
    return Int(bytes[offset]) | (Int(bytes[offset + 1]) << 8)
  }

  private static func littleEndian32(_ bytes: [UInt8], at offset: Int) -> Int? {
    guard offset >= 0, offset <= bytes.count - 4 else { return nil }
    return Int(bytes[offset]) | (Int(bytes[offset + 1]) << 8) |
      (Int(bytes[offset + 2]) << 16) | (Int(bytes[offset + 3]) << 24)
  }
}

private extension Result {
  var isSuccess: Bool {
    if case .success = self { return true }
    return false
  }
}

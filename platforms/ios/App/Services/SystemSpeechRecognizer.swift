import AVFoundation
import Foundation
import Speech

/// The `system` provider: Apple's own on-device recognition. iOS 26 and later use SpeechAnalyzer with SpeechTranscriber; earlier systems, or a language SpeechTranscriber does not cover, use SFSpeechRecognizer, kept on the device whenever the device can recognize that language locally.
enum SystemSpeechRecognizer {
  /// Recognizes 16 kHz mono PCM16 as it arrives and returns the final transcript once the stream ends. `partial` gets the running transcript, on no particular thread.
  static func transcribe(pcm: AsyncStream<Data>, language: String,
                         partial: @escaping (String) -> Void) async throws -> String {
    try await authorize()
    let locale = locale(for: language)
    if #available(iOS 26, *), let prepared = await SpeechAnalyzerDictation.prepare(locale: locale) {
      return try await prepared.run(pcm: pcm, partial: partial)
    }
    return try await legacy(pcm: pcm, locale: locale, partial: partial)
  }

  /// 把页面上的语言选择转成 locale；普通话 + 英语（`auto`）跟随设备的首选语言。
  static func locale(for language: String) -> Locale {
    switch language {
    case "zh-cn": Locale(identifier: "zh-CN")
    case "yue": Locale(identifier: "zh-HK")
    case "en", "en-us": Locale(identifier: "en-US")
    default: Locale(identifier: Locale.preferredLanguages.first ?? "zh-CN")
    }
  }

  private static func authorize() async throws {
    let status = SFSpeechRecognizer.authorizationStatus() == .notDetermined
      ? await withCheckedContinuation { continuation in
          SFSpeechRecognizer.requestAuthorization { continuation.resume(returning: $0) }
        }
      : SFSpeechRecognizer.authorizationStatus()
    try Task.checkCancellation()
    guard status == .authorized else { throw ServiceFailure(message: "请在系统设置中允许水杉使用语音识别。") }
  }

  static let pcmFormat = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16_000, channels: 1, interleaved: true)!

  static func buffer(pcm16 data: Data) -> AVAudioPCMBuffer? {
    let frames = AVAudioFrameCount(data.count / MemoryLayout<Int16>.size)
    guard frames > 0, let buffer = AVAudioPCMBuffer(pcmFormat: pcmFormat, frameCapacity: frames),
          let channel = buffer.int16ChannelData else { return nil }
    buffer.frameLength = frames
    data.withUnsafeBytes { raw in
      guard let base = raw.baseAddress else { return }
      memcpy(channel[0], base, Int(frames) * MemoryLayout<Int16>.size)
    }
    return buffer
  }

  private static func legacy(pcm: AsyncStream<Data>, locale: Locale,
                             partial: @escaping (String) -> Void) async throws -> String {
    guard let recognizer = SFSpeechRecognizer(locale: locale), recognizer.isAvailable else {
      throw ServiceFailure(message: "系统语音识别暂不支持当前语言，或现在不可用。")
    }
    let request = SFSpeechAudioBufferRecognitionRequest()
    request.shouldReportPartialResults = true
    request.addsPunctuation = true
    // Audio stays on the phone whenever the phone can recognize this language by itself; otherwise Apple's server is used, as the system dictation does.
    if recognizer.supportsOnDeviceRecognition { request.requiresOnDeviceRecognition = true }
    let outcome = LegacyOutcome()
    let task = recognizer.recognitionTask(with: request) { result, error in
      if let result {
        let text = result.bestTranscription.formattedString
        if result.isFinal { outcome.finish(.success(text)) } else { outcome.update(text); partial(text) }
      } else if let error {
        outcome.finish(.failure(error))
      }
    }
    do {
      for await chunk in pcm {
        try Task.checkCancellation()
        if let buffer = buffer(pcm16: chunk) { request.append(buffer) }
      }
      try Task.checkCancellation()
    } catch {
      task.cancel()
      throw error
    }
    request.endAudio()
    return try await withTaskCancellationHandler {
      try await outcome.wait()
    } onCancel: {
      task.cancel()
      outcome.finish(.failure(CancellationError()))
    }
  }
}

/// The result of one SFSpeech task, which may arrive before or after the caller starts waiting for it.
private final class LegacyOutcome: @unchecked Sendable {
  private let lock = NSLock()
  private var latest = ""
  private var result: Result<String, Error>?
  private var waiter: CheckedContinuation<String, Error>?

  func update(_ text: String) {
    lock.lock()
    latest = text
    lock.unlock()
  }

  /// A failure after some speech was recognized keeps what was heard, and silence is an empty transcript rather than an error.
  func finish(_ outcome: Result<String, Error>) {
    lock.lock()
    guard result == nil else { lock.unlock(); return }
    switch outcome {
    case .success: result = outcome
    case .failure(let error) where error is CancellationError: result = outcome
    case .failure(let error):
      let nsError = error as NSError
      let noSpeech = nsError.domain == "kAFAssistantErrorDomain" && [203, 1110].contains(nsError.code)
      result = !latest.isEmpty || noSpeech ? .success(latest)
        : .failure(ServiceFailure(message: "系统语音识别失败：\(error.localizedDescription)"))
    }
    let waiter = waiter
    self.waiter = nil
    let settled = result!
    lock.unlock()
    waiter?.resume(with: settled)
  }

  func wait() async throws -> String {
    try await withCheckedThrowingContinuation { continuation in
      lock.lock()
      if let result {
        lock.unlock()
        continuation.resume(with: result)
      } else {
        waiter = continuation
        lock.unlock()
      }
    }
  }
}

@available(iOS 26, *)
private struct SpeechAnalyzerDictation {
  let transcriber: SpeechTranscriber
  let format: AVAudioFormat

  /// A transcriber for `locale` with its assets installed, or nil when SpeechTranscriber cannot serve it, so the caller falls back before any audio is consumed.
  static func prepare(locale: Locale) async -> SpeechAnalyzerDictation? {
    guard SpeechTranscriber.isAvailable,
          let supported = await SpeechTranscriber.supportedLocale(equivalentTo: locale) else { return nil }
    let transcriber = SpeechTranscriber(locale: supported, transcriptionOptions: [],
                                        reportingOptions: [.volatileResults], attributeOptions: [])
    do {
      if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
        try await request.downloadAndInstall()
      }
    } catch { return nil }
    guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]) else { return nil }
    return SpeechAnalyzerDictation(transcriber: transcriber, format: format)
  }

  func run(pcm: AsyncStream<Data>, partial: @escaping (String) -> Void) async throws -> String {
    let analyzer = SpeechAnalyzer(modules: [transcriber])
    let (inputs, feed) = AsyncStream<AnalyzerInput>.makeStream()
    let transcriber = self.transcriber
    // Finalized results accumulate; the volatile one is replaced by each newer guess until it is finalized. SpeechTranscriber can leave a space before Chinese punctuation ("调整 ，"), so every text handed on goes through the local recognizer's transcript rules.
    let collector = Task { () throws -> String in
      var finalized = ""
      for try await result in transcriber.results {
        let text = String(result.text.characters)
        if result.isFinal {
          finalized += text
          partial(LocalSpeechText.tidy(finalized))
        } else {
          partial(LocalSpeechText.tidy(finalized + text))
        }
      }
      return LocalSpeechText.tidy(finalized)
    }
    do {
      try await analyzer.prepareToAnalyze(in: format)
      try await analyzer.start(inputSequence: inputs)
      let converter = SystemSpeechRecognizer.pcmFormat == format ? nil
        : AVAudioConverter(from: SystemSpeechRecognizer.pcmFormat, to: format)
      for await chunk in pcm {
        try Task.checkCancellation()
        guard let source = SystemSpeechRecognizer.buffer(pcm16: chunk) else { continue }
        if let converter {
          if let converted = convert(source, with: converter) { feed.yield(AnalyzerInput(buffer: converted)) }
        } else {
          feed.yield(AnalyzerInput(buffer: source))
        }
      }
      try Task.checkCancellation()
      feed.finish()
      try await analyzer.finalizeAndFinishThroughEndOfInput()
      return try await collector.value
    } catch {
      feed.finish()
      await analyzer.cancelAndFinishNow()
      collector.cancel()
      throw error
    }
  }

  private func convert(_ buffer: AVAudioPCMBuffer, with converter: AVAudioConverter) -> AVAudioPCMBuffer? {
    let ratio = format.sampleRate / buffer.format.sampleRate
    guard let output = AVAudioPCMBuffer(pcmFormat: format,
                                        frameCapacity: AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 1)
    else { return nil }
    var supplied = false
    var error: NSError?
    converter.convert(to: output, error: &error) { _, status in
      if supplied { status.pointee = .noDataNow; return nil }
      supplied = true
      status.pointee = .haveData
      return buffer
    }
    return error == nil && output.frameLength > 0 ? output : nil
  }
}

import Foundation
import SherpaOnnxC
import UIKit

/// On-device recognition with sherpa-onnx, the Swift twin of `shared/voice/LocalAsr.cpp`: the same manifest, recognizer settings, hotword filtering and transcript rules, so a model reads the same audio the same way on the phone and the desktops. Only the app links the runtime; the keyboard extension's memory limit cannot hold a model.
enum LocalSpeechRecognizer {
  static let sampleRate: Int32 = 16_000

  /// Recognizes 16 kHz mono PCM16 as it arrives and returns the final transcript once the stream ends. `partial` gets the running transcript whenever it changes, on no particular thread. Hotwords are the user's dictionary words: handed to the recognizer when the model takes them natively, or applied to the final text through the shared pinyin matcher when its manifest says `pinyin`.
  static func transcribe(pcm: AsyncStream<Data>, modelDirectory: URL, language: String,
                         hotwords: [LocalSpeechHotword], partial: @escaping (String) -> Void) async throws -> String {
    let manifest = try LocalSpeechModelManifest(directory: modelDirectory)
    let words = hotwords.map(\.text)
    let session = try await LocalSpeechEngine.shared.run {
      try LocalSpeechSession(recognizer: LocalSpeechEngine.shared.acquire(manifest, language: language, hotwords: words),
                             hotwords: words)
    }
    // The streams go back on the engine's queue however the dictation ends, never from whichever thread drops the last reference.
    defer { LocalSpeechEngine.shared.close(session) }
    var last = ""
    for await chunk in pcm {
      try Task.checkCancellation()
      let samples = floats(pcm16: chunk)
      guard !samples.isEmpty else { continue }
      let text = try await LocalSpeechEngine.shared.run { try session.accept(samples) }
      if text != last {
        last = text
        partial(text)
      }
    }
    try Task.checkCancellation()
    let text = try await LocalSpeechEngine.shared.run { try session.finish() }
    try Task.checkCancellation()
    guard manifest.hotwords == "pinyin" else { return text }
    return LocalSpeechModelStore.correct(text, hotwords: hotwords)
  }

  static func floats(pcm16 data: Data) -> [Float] {
    let count = data.count / MemoryLayout<Int16>.size
    var samples = [Float](repeating: 0, count: count)
    data.withUnsafeBytes { raw in
      for index in 0..<count {
        let value = Int16(littleEndian: raw.loadUnaligned(fromByteOffset: index * 2, as: Int16.self))
        samples[index] = Float(value) / 32_768
      }
    }
    return samples
  }
}

/// A loaded recognizer and what was baked into it at creation.
private final class LoadedLocalRecognizer {
  let kind: LocalSpeechModelManifest.Kind
  let online: OpaquePointer?
  let offline: OpaquePointer?
  let vadModel: String
  let tokens: Set<String>
  let nativeHotwords: Bool

  init(kind: LocalSpeechModelManifest.Kind, online: OpaquePointer?, offline: OpaquePointer?, vadModel: String,
       tokens: Set<String>, nativeHotwords: Bool) {
    self.kind = kind
    self.online = online
    self.offline = offline
    self.vadModel = vadModel
    self.tokens = tokens
    self.nativeHotwords = nativeHotwords
  }

  deinit {
    if let online { SherpaOnnxDestroyOnlineRecognizer(online) }
    if let offline { SherpaOnnxDestroyOfflineRecognizer(offline) }
  }
}

/// Owns the one resident model and the queue every sherpa call runs on. Two resident models would double a footprint that is already the largest thing the app holds, so loading another releases the previous one, and the model is dropped when the app is backgrounded, warned about memory, or idle.
final class LocalSpeechEngine: @unchecked Sendable {
  static let shared = LocalSpeechEngine()

  private struct Key: Equatable {
    let directory: String
    let language: String
    let nanoHotwords: String
    let threads: Int32
  }

  private let queue = DispatchQueue(label: "app.msime.ios.local-speech", qos: .userInitiated)
  /// Touched only on `queue`.
  private var cached: (key: Key, recognizer: LoadedLocalRecognizer)?
  private var idleRelease: DispatchWorkItem?

  private init() {
    NotificationCenter.default.addObserver(forName: UIApplication.didReceiveMemoryWarningNotification, object: nil,
                                           queue: nil) { [weak self] _ in self?.release() }
    NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil,
                                           queue: nil) { [weak self] _ in self?.release() }
  }

  /// Runs sherpa work off the main thread, one call at a time.
  func run<T>(_ work: @escaping () throws -> T) async throws -> T {
    try await withCheckedThrowingContinuation { continuation in
      queue.async { continuation.resume(with: Result { try work() }) }
    }
  }

  /// Drops the resident model; a session still holding it keeps it alive until that session ends.
  func release() {
    queue.async { self.cached = nil }
  }

  /// Ends a session on the queue and keeps the model for two minutes after it, so dictating again right away does not reload it.
  fileprivate func close(_ session: LocalSpeechSession) {
    queue.async {
      session.close()
      self.idleRelease?.cancel()
      let item = DispatchWorkItem { self.cached = nil }
      self.idleRelease = item
      self.queue.asyncAfter(deadline: .now() + 120, execute: item)
    }
  }

  /// Must be called on `queue`.
  fileprivate func acquire(_ model: LocalSpeechModelManifest, language: String, hotwords: [String]) throws -> LoadedLocalRecognizer {
    dispatchPrecondition(condition: .onQueue(queue))
    idleRelease?.cancel()
    idleRelease = nil
    let key = Key(directory: model.directory.standardizedFileURL.path,
                  language: model.kind == .offlineSenseVoice ? LocalSpeechText.senseVoiceLanguage(language) : "",
                  nanoHotwords: model.kind == .offlineFunAsrNano ? LocalSpeechText.funAsrHotwords(hotwords) : "",
                  threads: LocalSpeechText.threadCount)
    if let cached, cached.key == key { return cached.recognizer }
    cached = nil
    let recognizer = try create(model, key: key)
    cached = (key, recognizer)
    return recognizer
  }

  private func create(_ model: LocalSpeechModelManifest, key: Key) throws -> LoadedLocalRecognizer {
    let strings = CStrings()
    if model.kind == .onlineTransducer {
      let tokens = try model.file("tokens")
      let bpeVocab = try model.optionalFile("bpe_vocab")
      let native = model.hotwords == "native" && bpeVocab != nil && !model.modelingUnit.isEmpty
      var config = SherpaOnnxOnlineRecognizerConfig()
      config.feat_config.sample_rate = LocalSpeechRecognizer.sampleRate
      config.feat_config.feature_dim = 80
      config.model_config.transducer.encoder = strings.add(try model.file("encoder"))
      config.model_config.transducer.decoder = strings.add(try model.file("decoder"))
      config.model_config.transducer.joiner = strings.add(try model.file("joiner"))
      config.model_config.tokens = strings.add(tokens)
      config.model_config.num_threads = key.threads
      config.model_config.provider = strings.add("cpu")
      if native, let bpeVocab {
        config.model_config.modeling_unit = strings.add(model.modelingUnit)
        config.model_config.bpe_vocab = strings.add(bpeVocab)
      }
      // Per-stream hotwords are only honoured by modified beam search.
      config.decoding_method = strings.add(native ? "modified_beam_search" : "greedy_search")
      config.max_active_paths = 4
      config.hotwords_score = 2.0
      config.enable_endpoint = 1
      config.rule1_min_trailing_silence = 2.4
      config.rule2_min_trailing_silence = 1.0
      config.rule3_min_utterance_length = 20.0
      guard let online = SherpaOnnxCreateOnlineRecognizer(&config) else {
        throw ServiceFailure(message: "无法加载本地语音模型，请删除后重新下载。")
      }
      let tokenSet = native ? LocalSpeechText.tokenSet((try? model.textFile("tokens", maximumBytes: LocalSpeechModelManifest.maximumTokensBytes)) ?? "") : []
      return LoadedLocalRecognizer(kind: model.kind, online: online, offline: nil, vadModel: "", tokens: tokenSet,
                                   nativeHotwords: native)
    }
    let vadModel = try model.file("vad")
    var config = SherpaOnnxOfflineRecognizerConfig()
    config.feat_config.sample_rate = LocalSpeechRecognizer.sampleRate
    config.feat_config.feature_dim = 80
    config.model_config.num_threads = key.threads
    config.model_config.provider = strings.add("cpu")
    config.decoding_method = strings.add("greedy_search")
    if model.kind == .offlineSenseVoice {
      config.model_config.tokens = strings.add(try model.file("tokens"))
      config.model_config.sense_voice.model = strings.add(try model.file("model"))
      config.model_config.sense_voice.language = strings.add(key.language)
      config.model_config.sense_voice.use_itn = 1
    } else {
      config.model_config.funasr_nano.encoder_adaptor = strings.add(try model.file("encoder_adaptor"))
      config.model_config.funasr_nano.llm = strings.add(try model.file("llm"))
      config.model_config.funasr_nano.embedding = strings.add(try model.file("embedding"))
      config.model_config.funasr_nano.tokenizer = strings.add(try model.file("tokenizer"))
      config.model_config.funasr_nano.itn = 1
      config.model_config.funasr_nano.hotwords = strings.add(key.nanoHotwords)
    }
    guard let offline = SherpaOnnxCreateOfflineRecognizer(&config) else {
      throw ServiceFailure(message: "无法加载本地语音模型，请删除后重新下载。")
    }
    return LoadedLocalRecognizer(kind: model.kind, online: nil, offline: offline, vadModel: vadModel, tokens: [],
                                 nativeHotwords: model.kind == .offlineFunAsrNano)
  }
}

/// One dictation against a loaded recognizer. Every method runs on the engine's queue.
private final class LocalSpeechSession {
  private static let vadWindow: Int32 = 512
  private let recognizer: LoadedLocalRecognizer
  private var onlineStream: OpaquePointer?
  private var vad: OpaquePointer?
  private var segments: [String] = []
  private var current = ""
  private var finished = false

  init(recognizer: LoadedLocalRecognizer, hotwords: [String]) throws {
    self.recognizer = recognizer
    if let online = recognizer.online {
      let words = recognizer.nativeHotwords ? LocalSpeechText.transducerHotwords(hotwords, tokens: recognizer.tokens) : ""
      onlineStream = words.isEmpty ? SherpaOnnxCreateOnlineStream(online)
        : SherpaOnnxCreateOnlineStreamWithHotwords(online, words)
      guard onlineStream != nil else { throw ServiceFailure(message: "本地语音识别失败，请重试。") }
      return
    }
    let strings = CStrings()
    var config = SherpaOnnxVadModelConfig()
    config.silero_vad.model = strings.add(recognizer.vadModel)
    config.silero_vad.threshold = 0.5
    config.silero_vad.min_silence_duration = 0.5
    config.silero_vad.min_speech_duration = 0.25
    config.silero_vad.window_size = Self.vadWindow
    // FunASR-nano shares a 512-token context between audio and text; past roughly 25 seconds it returns nothing at all.
    config.silero_vad.max_speech_duration = recognizer.kind == .offlineFunAsrNano ? 20 : 28
    config.sample_rate = LocalSpeechRecognizer.sampleRate
    config.num_threads = 1
    config.provider = strings.add("cpu")
    vad = SherpaOnnxCreateVoiceActivityDetector(&config, 60)
    guard vad != nil else { throw ServiceFailure(message: "无法加载本地语音模型的静音检测，请删除后重新下载。") }
  }

  func close() {
    finished = true
    if let onlineStream { SherpaOnnxDestroyOnlineStream(onlineStream) }
    if let vad { SherpaOnnxDestroyVoiceActivityDetector(vad) }
    onlineStream = nil
    vad = nil
  }

  /// Feeds samples and returns the running transcript.
  func accept(_ samples: [Float]) throws -> String {
    guard !finished else { throw ServiceFailure(message: "本地语音识别已结束。") }
    try samples.withUnsafeBufferPointer { buffer in
      guard let base = buffer.baseAddress else { return }
      let count = Int32(buffer.count)
      if let onlineStream {
        SherpaOnnxOnlineStreamAcceptWaveform(onlineStream, LocalSpeechRecognizer.sampleRate, base, count)
        decodeOnline()
      } else if let vad {
        // The detector judges each call as a whole (speech if any window in it is speech), so it is fed one window at a time; a long buffer in one call would merge every pause into a single segment.
        var offset: Int32 = 0
        while offset < count {
          SherpaOnnxVoiceActivityDetectorAcceptWaveform(vad, base + Int(offset), min(Self.vadWindow, count - offset))
          try drainVAD()
          offset += Self.vadWindow
        }
      }
    }
    return transcript
  }

  func finish() throws -> String {
    guard !finished else { throw ServiceFailure(message: "本地语音识别已结束。") }
    finished = true
    if let onlineStream {
      // Trailing silence lets the last chunk through the encoder's look-ahead before input ends.
      let tail = [Float](repeating: 0, count: Int(LocalSpeechRecognizer.sampleRate) * 6 / 10)
      SherpaOnnxOnlineStreamAcceptWaveform(onlineStream, LocalSpeechRecognizer.sampleRate, tail, Int32(tail.count))
      SherpaOnnxOnlineStreamInputFinished(onlineStream)
      decodeOnline()
    } else if let vad {
      SherpaOnnxVoiceActivityDetectorFlush(vad)
      try drainVAD()
    }
    return transcript
  }

  private var transcript: String {
    LocalSpeechText.tidy(LocalSpeechText.joinSegments(current.isEmpty ? segments : segments + [current]))
  }

  private func decodeOnline() {
    guard let online = recognizer.online, let onlineStream else { return }
    while SherpaOnnxIsOnlineStreamReady(online, onlineStream) != 0 {
      SherpaOnnxDecodeOnlineStream(online, onlineStream)
    }
    if let result = SherpaOnnxGetOnlineStreamResult(online, onlineStream) {
      current = result.pointee.text.map { String(cString: $0) } ?? ""
      SherpaOnnxDestroyOnlineRecognizerResult(result)
    } else {
      current = ""
    }
    if SherpaOnnxOnlineStreamIsEndpoint(online, onlineStream) != 0 {
      if !current.isEmpty { segments.append(current) }
      current = ""
      SherpaOnnxOnlineStreamReset(online, onlineStream)
    }
  }

  private func drainVAD() throws {
    guard let vad else { return }
    while SherpaOnnxVoiceActivityDetectorEmpty(vad) == 0 {
      if let segment = SherpaOnnxVoiceActivityDetectorFront(vad) {
        defer { SherpaOnnxDestroySpeechSegment(segment) }
        if segment.pointee.n > 0, let samples = segment.pointee.samples {
          try decodeSegment(samples, count: segment.pointee.n)
        }
      }
      SherpaOnnxVoiceActivityDetectorPop(vad)
    }
  }

  private func decodeSegment(_ samples: UnsafePointer<Float>, count: Int32) throws {
    guard let offline = recognizer.offline, let stream = SherpaOnnxCreateOfflineStream(offline) else {
      throw ServiceFailure(message: "本地语音识别失败，请重试。")
    }
    defer { SherpaOnnxDestroyOfflineStream(stream) }
    SherpaOnnxAcceptWaveformOffline(stream, LocalSpeechRecognizer.sampleRate, samples, count)
    SherpaOnnxDecodeOfflineStream(offline, stream)
    if let result = SherpaOnnxGetOfflineStreamResult(stream) {
      if let text = result.pointee.text, text.pointee != 0 { segments.append(String(cString: text)) }
      SherpaOnnxDestroyOfflineRecognizerResult(result)
    }
  }
}

/// C strings that outlive a sherpa create call, which reads its config's paths only while it runs.
private final class CStrings {
  private var pointers: [UnsafeMutablePointer<CChar>] = []

  func add(_ value: String) -> UnsafePointer<CChar> {
    let pointer = strdup(value)!
    pointers.append(pointer)
    return UnsafePointer(pointer)
  }

  deinit { pointers.forEach { free($0) } }
}

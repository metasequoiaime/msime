import AudioToolbox
import Foundation

/// 一次听写，录音在后台的主 App 里（`KeyboardVoiceDictation`）或在键盘里（`KeyboardDirectDictation`），两边向键盘报同样的事件，面板和上屏不分来源。
@MainActor
protocol KeyboardDictation: AnyObject {
  var onEvent: ((KeyboardVoiceDictation.Event) -> Void)? { get set }
  func start(held: Bool)
  func finish()
  func cancel()
}

/// 键盘自己录音的一次听写：App 发布了说完整段上传的联网识别服务时（`KeyboardVoiceService`）用它，录音、上传和识别都在键盘里，不打开 App，也不依赖后台会话。松开或点「说完了」后把整段录音发给服务，结果照常交给 `insertVoiceResult`。
///
/// 麦克风打不开时报 `reopensSession`，键盘退回打开 App 录音的老路：主 App 从没拿到麦克风权限时，由它去请求。
@MainActor
final class KeyboardDirectDictation: KeyboardDictation {
  /// 与 App 的 `VoiceSessionService.dictationLimit` 相同：一次最多录一分钟，到时像松开一样结束。
  static let dictationLimit: TimeInterval = 60
  /// 麦克风多久没打开就当作键盘里录不了音。
  static let startTimeout: TimeInterval = 3
  /// 电平最多这么久报一次，与 App 经后台会话报电平的间隔相同。
  static let levelInterval: TimeInterval = 0.1

  private enum Phase { case idle, starting, listening, recognizing }

  var onEvent: ((KeyboardVoiceDictation.Event) -> Void)?
  private let configuration: KeyboardVoiceConfiguration
  private let microphone: KeyboardAudioInput
  private let transcribe: (Data) async throws -> String
  private var phase = Phase.idle
  private var pcm = Data()
  private var lastLevel = Date.distantPast
  private var timer: Task<Void, Never>?
  private var recognition: Task<Void, Never>?

  /// `transcribe` 收到整段录音的 WAV、返回识别文字；不传时发给已发布的服务。
  init(configuration: KeyboardVoiceConfiguration, microphone: KeyboardAudioInput,
       transcribe: ((Data) async throws -> String)? = nil) {
    self.configuration = configuration
    self.microphone = microphone
    self.transcribe = transcribe ?? { wav in try await Self.transcribe(wav, configuration: configuration) }
  }

  /// 整段上传的服务没有停顿自动结束，点按开始和按住开始都等用户结束。
  func start(held: Bool) {
    guard phase == .idle else { return }
    phase = .starting
    pcm = Data()
    microphone.onStarted = { [weak self] in self?.started() }
    microphone.onAudio = { [weak self] in self?.heard($0) }
    microphone.onStopped = { [weak self] reason, _ in self?.stopped(reason) }
    microphone.start()
    schedule(Self.startTimeout) { [weak self] in self?.fail("键盘里没能打开麦克风，改用水杉 App 录音…", reopensSession: true) }
  }

  func finish() {
    switch phase {
    case .starting:
      // 麦克风还没打开就松手：什么也没录到，与 App 收到没开始过的停止命令一样交回空结果。
      stopMicrophone()
      phase = .idle
      onEvent?(.recognizing(text: "", message: ""))
      onEvent?(.finished(""))
    case .listening:
      recognize()
    case .idle, .recognizing:
      break
    }
  }

  /// 取消这次听写，不再报告任何事件。
  func cancel() {
    stopMicrophone()
    recognition?.cancel()
    recognition = nil
    phase = .idle
  }

  private func started() {
    guard phase == .starting else { return }
    phase = .listening
    if configuration.startSound { AudioServicesPlaySystemSound(1113) }
    onEvent?(.listening(""))
    schedule(Self.dictationLimit) { [weak self] in self?.finish() }
  }

  private func heard(_ chunk: Data) {
    guard phase == .listening else { return }
    pcm.append(chunk)
    let now = Date()
    guard now.timeIntervalSince(lastLevel) >= Self.levelInterval else { return }
    lastLevel = now
    onEvent?(.level(VoiceLevel.normalized(decibels: VoiceLevel.decibels(pcm16: chunk))))
  }

  /// 还没开始就停了是没打开麦克风，退回 App 录音；录到一半被打断时，已经录到的照样识别。
  private func stopped(_ reason: String) {
    switch phase {
    case .starting:
      fail("键盘里没能打开麦克风（\(reason)），改用水杉 App 录音…", reopensSession: true)
    case .listening:
      recognize()
    case .idle, .recognizing:
      break
    }
  }

  private func recognize() {
    stopMicrophone()
    phase = .recognizing
    onEvent?(.recognizing(text: "", message: ""))
    let audio = pcm
    pcm = Data()
    guard !audio.isEmpty else {
      fail("没有录到声音，请重试。", reopensSession: false)
      return
    }
    let transcribe = transcribe
    let endSound = configuration.endSound
    recognition = Task { [weak self] in
      do {
        let text = try await transcribe(AppServicesBridge.wav(audio))
        try Task.checkCancellation()
        guard let self, phase == .recognizing else { return }
        if endSound { AudioServicesPlaySystemSound(1114) }
        Self.recordDuration(of: audio, text: text)
        phase = .idle
        onEvent?(.finished(text))
      } catch is CancellationError {
      } catch {
        self?.fail(error.localizedDescription, reopensSession: false)
      }
    }
  }

  private func fail(_ message: String, reopensSession: Bool) {
    guard phase != .idle else { return }
    stopMicrophone()
    phase = .idle
    onEvent?(.failed(message, reopensSession: reopensSession))
  }

  private func stopMicrophone() {
    timer?.cancel()
    timer = nil
    microphone.onStarted = nil
    microphone.onAudio = nil
    microphone.onStopped = nil
    microphone.stop()
  }

  private func schedule(_ seconds: TimeInterval, _ action: @escaping () -> Void) {
    timer?.cancel()
    timer = Task {
      do { try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000)) } catch { return }
      action()
    }
  }

  /// 与 App 的 `VoiceSessionService.recordDuration` 一样计入打字统计的语音时长；16 kHz 单声道 16 位每毫秒 32 字节。
  private static func recordDuration(of pcm: Data, text: String) {
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          KeyboardPrivacyGate(incognito: KeyboardPrivacyPreference.incognito, credentialField: false).allows(.voiceDuration)
    else { return }
    TypingStatisticsExtras.recordVoice(milliseconds: pcm.count / 32)
  }

  /// 把整段录音发给已发布的服务，请求与 App 的 `CustomServiceClient.makeRequest` 相同。
  static func transcribe(_ wav: Data, configuration: KeyboardVoiceConfiguration,
                         sessionConfiguration: URLSessionConfiguration = .ephemeral) async throws -> String {
    guard let url = URL(string: configuration.endpoint) else {
      throw ServiceFailure(message: "语音服务地址无效，请在水杉 App 中重新保存语音服务。")
    }
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.timeoutInterval = 60
    let token = try KeyboardVoiceService.token()
    if !token.isEmpty { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
    if configuration.chatAudio {
      request.httpBody = try AppServicesBridge.chatAudioBody(wav, model: configuration.model)
      request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    } else {
      let multipart = try AppServicesBridge.transcriptionBody(wav, model: configuration.model, language: configuration.language)
      request.httpBody = multipart["body"] as? Data
      request.setValue(multipart["contentType"] as? String, forHTTPHeaderField: "Content-Type")
    }
    let session = URLSession(configuration: AIEndpointPolicy.sessionConfiguration(sessionConfiguration, for: url),
                             delegate: NoRedirects(), delegateQueue: nil)
    defer { session.invalidateAndCancel() }
    let (bytes, response) = try await session.bytes(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
      let status = (response as? HTTPURLResponse)?.statusCode ?? 0
      throw ServiceFailure(message: "服务请求失败（HTTP \(status)），请检查地址、模型和密钥。")
    }
    var data = Data()
    for try await byte in bytes {
      guard data.count < 1024 * 1024 else { throw ServiceFailure(message: "服务响应过大。") }
      data.append(byte)
    }
    return try AppServicesBridge.parseResponse(data, voice: true)
  }
}

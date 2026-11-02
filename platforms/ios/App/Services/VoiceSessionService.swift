import AVFoundation
import AudioToolbox
import Foundation
import UIKit

/// 按住空格说话的后台语音会话。iOS 键盘扩展不能录音，所以键盘第一次长按空格时打开本应用，由这里打开录音会话并让音频引擎一直运行，App 因此能在后台保持（`UIBackgroundModes: audio`）；用户回到原应用后，键盘经 `VoiceSessionChannel` 发「开始 / 停止 / 取消」，这里把那一段麦克风音频交给识别器，再把实时文字和结果写回去，由键盘上屏。空闲 `VoiceSessionChannel.idleLifetime` 后自动关闭会话，状态栏的麦克风标记随之消失。
///
/// 没有在听写时引擎采到的音频直接丢掉，不落盘也不识别。
@MainActor
final class VoiceSessionService: ObservableObject {
  static let shared = VoiceSessionService()

  enum Status: Equatable {
    case inactive
    case starting
    case ready
    case failed(String)
  }

  @Published private(set) var status: Status = .inactive
  @Published private(set) var expiresAt: Date?

  /// 和语音页一样，一次听写最长 60 秒。
  static let dictationLimit: TimeInterval = 60

  private let channel = VoiceSessionChannel()
  private let tap = VoiceSessionTap()
  private var engine: AVAudioEngine?
  private var commandObserver: DarwinNotificationObserver?
  private var systemObservers: [NSObjectProtocol] = []
  private var heartbeat: Timer?
  private var current: VoiceSessionState?
  /// `current` 上一次随通知发出的时刻；心跳只刷新时间，不算发出。
  private var publishedAt = Date.distantPast
  private var dictation: (id: UUID, task: Task<Void, Never>, limit: Task<Void, Never>)?
  /// 点按开始的本地识别的停顿计时（#10310）；按住说话、系统识别和云端服务没有它。
  private var pauseStop: VoicePauseStop?
  /// 听写中定时把麦克风电平报给键盘，听写结束就停。
  private var levelTimer: Timer?
  /// 上次报给键盘的电平；这次听写还没报过时为 nil。
  private var publishedLevel: Float?
  private var doubaoGeneration: UInt64 = 0

  var isActive: Bool { engine != nil }

  private init() {}

  /// 打开会话；已经开着时只延长时限。键盘打开 `voice-session` 链接时，App 在前台调用它。
  func activate() async {
    if isActive {
      status = .ready
      extend()
      return
    }
    status = .starting
    do {
      let allowed = await withCheckedContinuation { continuation in
        AVAudioApplication.requestRecordPermission { continuation.resume(returning: $0) }
      }
      guard allowed else { throw ServiceFailure(message: "请在系统设置中允许水杉使用麦克风。") }
      let session = AVAudioSession.sharedInstance()
      // 和其他声音混音：会话开着、没在听写时，原应用里的音乐和视频照常播放。
      try session.setCategory(.playAndRecord, mode: .default, options: [.mixWithOthers, .defaultToSpeaker])
      try session.setActive(true)
      let engine = AVAudioEngine()
      try start(engine)
      self.engine = engine
      LocalSpeechEngine.shared.keepsModelInBackground = true
      commandObserver = DarwinNotificationObserver(VoiceSessionChannel.commandNotification) { [weak self] in
        self?.handleCommand()
      }
      observeInterruptions()
      heartbeat = Timer.scheduledTimer(withTimeInterval: VoiceSessionChannel.heartbeatInterval, repeats: true) { [weak self] _ in
        Task { @MainActor in self?.beat() }
      }
      status = .ready
      extend()
      publish(.ready, request: nil)
    } catch {
      deactivate()
      status = .failed(error.localizedDescription)
    }
  }

  /// 按输入节点当前的格式装上 tap 并启动引擎。换了音频线路（插拔耳机、连上蓝牙）后输入格式会变，要按新格式重来。
  private func start(_ engine: AVAudioEngine) throws {
    let input = engine.inputNode
    let source = input.outputFormat(forBus: 0)
    guard source.sampleRate > 0,
          let target = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16_000, channels: 1, interleaved: true),
          let converter = AVAudioConverter(from: source, to: target)
    else { throw ServiceFailure(message: "无法开始录音，请重试。") }
    input.installTap(onBus: 0, bufferSize: 4096, format: source,
                     block: Self.tapBlock(converter: converter, target: target, tap: tap))
    engine.prepare()
    try engine.start()
  }

  /// 关闭会话：停掉引擎、交还音频会话、删掉状态，键盘下次长按空格会重新打开 App。
  func deactivate() {
    cancelDictation()
    heartbeat?.invalidate()
    heartbeat = nil
    commandObserver = nil
    systemObservers.forEach(NotificationCenter.default.removeObserver)
    systemObservers = []
    if let engine {
      engine.inputNode.removeTap(onBus: 0)
      engine.stop()
      self.engine = nil
      try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
    LocalSpeechEngine.shared.keepsModelInBackground = false
    // App 在后台时模型已经没有用处，几百 MB 不留着。
    if UIApplication.shared.applicationState != .active { LocalSpeechEngine.shared.release() }
    current = nil
    expiresAt = nil
    channel.clearState()
    if status == .ready || status == .starting { status = .inactive }
  }

  private func extend() {
    let expiry = Date().addingTimeInterval(VoiceSessionChannel.idleLifetime)
    expiresAt = expiry
    current?.expiresAt = expiry
  }

  private func beat() {
    guard isActive else { return }
    if dictation == nil, let expiresAt, expiresAt <= Date() {
      deactivate()
      return
    }
    guard var state = current else { return }
    state.heartbeat = Date()
    // 键盘收到通知时就读走了结果；过几秒后的心跳把文字从共享文件里清掉，不留着。不能是紧跟着的那次心跳，键盘可能还没来得及读。
    if state.phase == .finished || state.phase == .failed, Date().timeIntervalSince(publishedAt) > 3 {
      state = VoiceSessionState(phase: .ready, request: nil, heartbeat: state.heartbeat, expiresAt: state.expiresAt)
    }
    current = state
    channel.write(state)
  }

  /// `listening` 的状态带上最近报过的电平，实时文字的更新不会把键盘的光晕清零；其他阶段不带电平。
  private func publish(_ phase: VoiceSessionState.Phase, request: UUID?, text: String = "", message: String = "") {
    guard isActive, let expiresAt else { return }
    let state = VoiceSessionState(phase: phase, request: request, text: text, message: message,
                                  level: phase == .listening ? publishedLevel : nil,
                                  endsOnPause: phase == .listening && pauseStop != nil ? true : nil,
                                  heartbeat: Date(), expiresAt: expiresAt)
    current = state
    publishedAt = Date()
    channel.publish(state)
  }

  private func observeInterruptions() {
    let center = NotificationCenter.default
    let end: (String) -> Void = { [weak self] message in
      guard let self else { return }
      deactivate()
      status = .failed(message)
    }
    systemObservers = [
      // 来电、闹钟或另一个应用开始录音会打断会话，引擎随之停下，会话不能再用。
      center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { note in
        let type = (note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt).flatMap(AVAudioSession.InterruptionType.init)
        guard type == .began else { return }
        MainActor.assumeIsolated { end("语音被来电或其他应用打断，已经关闭。在键盘上长按空格可以重新开启。") }
      },
      center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { _ in
        MainActor.assumeIsolated { end("系统音频服务重置，语音已经关闭。在键盘上长按空格可以重新开启。") }
      },
      // 换音频线路时系统会停掉引擎；按新的输入格式重新启动，正在进行的听写照常接着录。
      center.addObserver(forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main) { [weak self] _ in
        MainActor.assumeIsolated {
          guard let self, let engine = self.engine else { return }
          engine.inputNode.removeTap(onBus: 0)
          do { try self.start(engine) } catch { end("音频线路变化后无法继续录音，语音已经关闭。在键盘上长按空格可以重新开启。") }
        }
      },
    ]
  }

  // MARK: - 键盘的命令

  private func handleCommand() {
    guard isActive, let command = channel.readCommand(), command.version == 1,
          abs(command.sentAt.timeIntervalSinceNow) < 30 else { return }
    switch command.action {
    case .start:
      guard dictation?.id != command.request, current?.request != command.request else { return }
      start(command.request, tapStarted: command.tapStarted == true)
    case .stop:
      if dictation?.id == command.request {
        endPauseStop()
        tap.end()
        publish(.recognizing, request: command.request, text: current?.text ?? "")
      } else if current?.request != command.request {
        // 开始命令被同一个文件里随后的停止命令覆盖、从没开始过：告诉键盘这次没有内容，不让它一直等。
        publish(.finished, request: command.request)
      }
    case .cancel:
      if dictation?.id == command.request {
        cancelDictation()
        publish(.ready, request: nil)
      }
    }
  }

  private func start(_ id: UUID, tapStarted: Bool) {
    cancelDictation()
    let configuration = CustomServiceConfiguration.load(.voice)
    let settings = VoicePolishSettings(MetasequoiaInputSessionBridge.loadSharedPreferences())
    let pcm = tap.begin()
    if settings.soundEnabled && settings.startSound { AudioServicesPlaySystemSound(1113) }
    extend()
    publish(.listening, request: id)
    startLevelUpdates(id)
    let task = Task { [weak self] in
      guard let self else { return }
      await recognize(id: id, pcm: pcm, configuration: configuration, settings: settings, tapStarted: tapStarted)
    }
    let limit = Task { [weak self] in
      do { try await Task.sleep(nanoseconds: UInt64(Self.dictationLimit * 1_000_000_000)) } catch { return }
      guard let self, dictation?.id == id else { return }
      endPauseStop()
      tap.end()
      publish(.recognizing, request: id, text: current?.text ?? "")
    }
    dictation = (id, task, limit)
  }

  private func cancelDictation() {
    endPauseStop()
    guard let dictation else { return }
    self.dictation = nil
    dictation.limit.cancel()
    dictation.task.cancel()
    _ = tap.end()
    stopLevelUpdates()
  }

  /// 每 `VoiceSessionChannel.levelInterval` 看一次 tap 记下的电平，变化够大才连同当前文字发给键盘。
  private func startLevelUpdates(_ id: UUID) {
    stopLevelUpdates()
    levelTimer = Timer.scheduledTimer(withTimeInterval: VoiceSessionChannel.levelInterval, repeats: true) { [weak self] _ in
      Task { @MainActor in self?.publishLevel(id) }
    }
  }

  private func stopLevelUpdates() {
    levelTimer?.invalidate()
    levelTimer = nil
    publishedLevel = nil
  }

  /// 只在这次听写还在录音（`listening`）时报；松开后进入识别，电平就不再写。
  private func publishLevel(_ id: UUID) {
    // 上一段听写的计时器在换成这一段之前排进来的回调，不能停掉这一段的。
    guard dictation?.id == id else { return }
    guard let current, current.phase == .listening, current.request == id else {
      stopLevelUpdates()
      return
    }
    let level = tap.level
    guard VoiceSessionChannel.publishesLevel(level, after: publishedLevel) else { return }
    publishedLevel = level
    publish(.listening, request: id, text: current.text)
  }

  /// 识别一段听写，按语音设置里选的服务：本地模型（没有下载模型时退回系统语音识别）、系统语音识别、豆包边说边识别，其他服务在说完后上传整段录音。点按开始的本地识别还按用户设的停顿时间和判停灵敏度在停顿后自己结束录音（#10310），与 Android、鸿蒙键盘里的本地识别一致；按住说话由松手结束，系统识别、豆包和上传式服务都不按停顿结束。
  private func recognize(id: UUID, pcm: AsyncStream<Data>, configuration: CustomServiceConfiguration,
                         settings: VoicePolishSettings, tapStarted: Bool) async {
    let partial: @Sendable (String) -> Void = { text in
      Task { @MainActor [weak self] in
        guard let self, dictation?.id == id, current?.phase == .listening || current?.phase == .recognizing else { return }
        publish(current?.phase ?? .listening, request: id, text: text)
      }
    }
    do {
      var result: String
      switch configuration.voiceProvider {
      case .local, .system:
        if configuration.voiceProvider == .local, let model = Self.selectedLocalModel() {
          let stateRoot = URL(fileURLWithPath: MetasequoiaInputSessionBridge.sharedStateDirectory, isDirectory: true)
          let resources = PersonalDictionaryBridge.packagedResources
          let hotwords = await Task.detached(priority: .userInitiated) {
            resources.map { LocalSpeechModelStore.hotwords(resources: $0, stateRoot: stateRoot) } ?? []
          }.value
          let pause = VoicePauseSettings.forDictation(tapStarted: tapStarted,
                                                      preferences: MetasequoiaInputSessionBridge.loadSharedPreferences())
          if let pause, dictation?.id == id {
            pauseStop = VoicePauseStop(milliseconds: pause.pauseMilliseconds) { [weak self] in self?.endOnPause(id) }
            // 马上告诉键盘这次会在停顿后结束，聆听面板换成相应的提示，不等下一次出字或电平变化。
            if let current, current.phase == .listening, current.request == id {
              publish(.listening, request: id, text: current.text)
            }
          }
          // 出字和说话状态都先回到主线程再交给停顿计时，与实时文字走同一条路。
          let heard: @Sendable (String) -> Void = { text in
            partial(text)
            guard !text.isEmpty else { return }
            Task { @MainActor [weak self] in
              guard let self, dictation?.id == id else { return }
              pauseStop?.heard()
            }
          }
          let speech: @Sendable (Bool) -> Void = { speaking in
            Task { @MainActor [weak self] in
              guard let self, dictation?.id == id else { return }
              pauseStop?.speech(speaking)
            }
          }
          result = try await LocalSpeechRecognizer.transcribe(
            pcm: pcm, modelDirectory: model, language: settings.language, hotwords: hotwords,
            vadThreshold: pause?.vadThreshold ?? VoicePauseSettings.defaultVADThreshold, partial: heard, speech: speech)
        } else {
          result = try await SystemSpeechRecognizer.transcribe(pcm: pcm, language: settings.language, partial: partial)
        }
      case .doubao where settings.streamLive:
        let url = try configuration.validatedURL(requiresModel: false, allowWebSocket: true)
        let token = try ServiceTokenStore.read(.voice, url: url)
        result = try await CustomServiceClient.streamDoubao(
          configuration: configuration, token: token, generation: nextDoubaoGeneration(), client: Self.doubaoClient(configuration),
          pcm: pcm, partial: partial)
      default:
        // 说完才上传：等键盘停止录音，再把整段录音发给服务。
        for await _ in pcm { try Task.checkCancellation() }
        let audio = tap.captured
        guard !audio.isEmpty else { throw ServiceFailure(message: "没有录到声音，请重试。") }
        let doubao = configuration.voiceProvider == .doubao
        let url = try configuration.validatedURL(allowWebSocket: doubao)
        let token = try ServiceTokenStore.read(.voice, url: url)
        result = try await CustomServiceClient.request(
          kind: .voice, configuration: configuration, wav: CustomServiceClient.silentWAV(audio), pcm: audio, token: token,
          language: settings.transcriptionLanguage(for: configuration.voiceProvider),
          generation: doubao ? nextDoubaoGeneration() : 0, doubaoClient: doubao ? Self.doubaoClient(configuration) : nil)
      }
      try Task.checkCancellation()
      guard dictation?.id == id else { return }
      if settings.soundEnabled && settings.endSound { AudioServicesPlaySystemSound(1114) }
      recordDuration(result)
      if settings.polishEnabled && !result.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        publish(.recognizing, request: id, text: result, message: "正在润色…")
        // 润色失败时用识别原文，和语音页一样不丢结果。
        if let polished = try? await VoicePolishService.polish(result, settings: settings) { result = polished }
        try Task.checkCancellation()
        guard dictation?.id == id else { return }
      }
      finish(id)
      publish(.finished, request: id, text: result)
    } catch is CancellationError {
    } catch {
      guard dictation?.id == id else { return }
      finish(id)
      publish(.failed, request: id, message: error.localizedDescription)
    }
  }

  /// 点按开始的本地识别停顿够久了：和到时长上限一样停止录音，识别器读完剩下的音频后照常交出结果。
  private func endOnPause(_ id: UUID) {
    guard dictation?.id == id, let current, current.phase == .listening, current.request == id else { return }
    pauseStop = nil
    tap.end()
    publish(.recognizing, request: id, text: current.text)
  }

  private func endPauseStop() {
    pauseStop?.cancel()
    pauseStop = nil
  }

  private func finish(_ id: UUID) {
    guard let dictation, dictation.id == id else { return }
    endPauseStop()
    dictation.limit.cancel()
    self.dictation = nil
    _ = tap.end()
    stopLevelUpdates()
    extend()
  }

  /// 动口不动手：和语音页一样，一段听写转成文字后把录音时长计入统计；隐私模式下不记。
  private func recordDuration(_ result: String) {
    guard !result.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          KeyboardPrivacyGate(incognito: KeyboardPrivacyPreference.incognito, credentialField: false).allows(.voiceDuration)
    else { return }
    TypingStatisticsExtras.recordVoice(milliseconds: VoiceRecorder.milliseconds(pcm16Bytes: tap.captured.count))
  }

  private func nextDoubaoGeneration() -> UInt64 {
    doubaoGeneration &+= 1
    return doubaoGeneration
  }

  private static func doubaoClient(_ configuration: CustomServiceConfiguration) -> DoubaoVoiceClient {
    DoubaoVoiceClient(transport: DoubaoWebSocketTransport(), codec: DoubaoHostFrameCodec.make(
      enableITN: configuration.doubaoEnableITN, punctuation: configuration.doubaoEnablePunctuation,
      DDC: configuration.doubaoEnableDDC, boostingTable: configuration.doubaoBoostingTableID))
  }

  /// 设置里选用的本地模型，和 `LocalSpeechModelManager.selectedDirectory` 读同一个偏好；没有选或模型已经不在时为 nil。
  static func selectedLocalModel() -> URL? {
    let voice = MetasequoiaInputSessionBridge.loadSharedPreferences()?["voice_input"] as? [String: Any] ?? [:]
    return LocalSpeechModelLocation.resolve(storedPath: voice["asr_model_path"] as? String ?? "",
                                            root: try? LocalSpeechModelLocation.root())
  }

  /// 引擎的 tap 跑在音频线程上，所以在主 actor 之外构造，只碰加锁的 `VoiceSessionTap`。
  private nonisolated static func tapBlock(converter: AVAudioConverter, target: AVAudioFormat,
                                           tap: VoiceSessionTap) -> AVAudioNodeTapBlock {
    { buffer, _ in
      // 没在听写时连转换都不做；电平也在这里算，主线程只取最近的一个。
      guard tap.isListening, let pcm = VoiceRecorder.pcm16(buffer, converter: converter, target: target) else { return }
      tap.append(pcm, level: VoiceLevel.normalized(decibels: VoiceLevel.decibels(pcm16: pcm)))
    }
  }
}

/// 一段听写的音频：音频线程写，主线程开始和结束。没在听写时不收任何音频。
private final class VoiceSessionTap: @unchecked Sendable {
  /// 60 秒 16 kHz PCM16，和语音页的上限相同。
  private static let limit = 60 * 32_000
  private let lock = NSLock()
  private var continuation: AsyncStream<Data>.Continuation?
  private var pcm = Data()
  private var latestLevel: Float = 0

  var isListening: Bool {
    lock.lock()
    defer { lock.unlock() }
    return continuation != nil
  }

  /// 最近一块音频的麦克风电平（0…1）；没在听写时为 0。
  var level: Float {
    lock.lock()
    defer { lock.unlock() }
    return latestLevel
  }

  /// 这段听写到目前为止录到的音频；结束后保留到下一段开始。
  var captured: Data {
    lock.lock()
    defer { lock.unlock() }
    return pcm
  }

  func begin() -> AsyncStream<Data> {
    let (stream, continuation) = AsyncStream<Data>.makeStream()
    lock.lock()
    self.continuation?.finish()
    self.continuation = continuation
    pcm = Data()
    latestLevel = 0
    lock.unlock()
    return stream
  }

  func append(_ chunk: Data, level: Float) {
    lock.lock()
    defer { lock.unlock() }
    guard let continuation else { return }
    latestLevel = level
    guard pcm.count + chunk.count <= Self.limit else { return }
    pcm.append(chunk)
    continuation.yield(chunk)
  }

  /// 结束音频流，识别器随之给出最终结果。
  @discardableResult
  func end() -> Bool {
    lock.lock()
    let continuation = continuation
    self.continuation = nil
    latestLevel = 0
    lock.unlock()
    continuation?.finish()
    return continuation != nil
  }
}

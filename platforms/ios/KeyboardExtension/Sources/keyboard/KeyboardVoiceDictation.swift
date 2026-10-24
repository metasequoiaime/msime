import Foundation

/// 键盘这一侧的一次按住说话：经 `VoiceSessionChannel` 让后台的主 App 开始、停止或取消录音，把 App 写回的实时文字和结果交给键盘。键盘扩展不能录音，录音和识别都在 App 里（`VoiceSessionService`）。
@MainActor
final class KeyboardVoiceDictation {
  enum Event: Equatable {
    /// 正在听，带实时识别的文字。
    case listening(String)
    /// 松开以后在识别或润色；`message` 为空时就是在识别。
    case recognizing(text: String, message: String)
    case finished(String)
    /// `reopensSession` 为真时 App 没有回应，键盘应重新打开 App 开启会话。
    case failed(String, reopensSession: Bool)
  }

  /// App 收到开始命令后多久内必须回应，否则当作会话已经不在。
  static let startTimeout: TimeInterval = 2
  /// 松开以后最多等识别结果多久。润色要走网络，留宽一些。
  static let resultTimeout: TimeInterval = 30

  private let channel: VoiceSessionChannel
  private var observer: DarwinNotificationObserver?
  private var watchdog: Task<Void, Never>?
  private(set) var request: UUID?
  private var acknowledged = false
  private var stopRequested = false
  var onEvent: ((Event) -> Void)?

  init(channel: VoiceSessionChannel = VoiceSessionChannel()) {
    self.channel = channel
  }

  /// App 的语音会话此刻能不能接听写；不能时要先打开 App。
  var sessionAvailable: Bool { channel.readState()?.isAvailable() == true }

  func start() {
    let id = UUID()
    request = id
    acknowledged = false
    stopRequested = false
    observer = DarwinNotificationObserver(VoiceSessionChannel.stateNotification) { [weak self] in self?.stateChanged() }
    channel.send(VoiceSessionCommand(action: .start, request: id, sentAt: Date()))
    watch(Self.startTimeout, .failed("水杉没有回应，正在重新开启语音…", reopensSession: true))
  }

  /// 松开或再点一次：停止录音，等结果。App 还没回应开始命令时先记下，回应后再发，免得停止命令盖掉还没读到的开始命令。
  func finish() {
    guard let request, !stopRequested else { return }
    stopRequested = true
    onEvent?(.recognizing(text: "", message: ""))
    guard acknowledged else { return }
    sendStop(request)
  }

  /// 取消这次听写，不再报告任何事件。
  func cancel() {
    if let request { channel.send(VoiceSessionCommand(action: .cancel, request: request, sentAt: Date())) }
    reset()
  }

  private func sendStop(_ request: UUID) {
    channel.send(VoiceSessionCommand(action: .stop, request: request, sentAt: Date()))
    watch(Self.resultTimeout, .failed("识别超时，请重试。", reopensSession: false))
  }

  private func stateChanged() {
    guard let request, let state = channel.readState(), state.request == request else { return }
    switch state.phase {
    case .listening:
      if !acknowledged {
        acknowledged = true
        watchdog?.cancel()
        if stopRequested { sendStop(request) }
      }
      onEvent?(stopRequested ? .recognizing(text: state.text, message: "") : .listening(state.text))
    case .recognizing:
      acknowledged = true
      onEvent?(.recognizing(text: state.text, message: state.message))
    case .finished:
      reset()
      onEvent?(.finished(state.text))
    case .failed:
      reset()
      onEvent?(.failed(state.message.isEmpty ? "语音识别失败，请重试。" : state.message, reopensSession: false))
    case .ready:
      break
    }
  }

  private func watch(_ seconds: TimeInterval, _ event: Event) {
    watchdog?.cancel()
    watchdog = Task { [weak self] in
      do { try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000)) } catch { return }
      guard let self, request != nil else { return }
      cancel()
      onEvent?(event)
    }
  }

  private func reset() {
    request = nil
    observer = nil
    watchdog?.cancel()
    watchdog = nil
  }
}

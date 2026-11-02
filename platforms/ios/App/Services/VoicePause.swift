import Foundation

/// 点按开始的本地识别的停顿时间和判停灵敏度（#10310），存在共享文档的 `voice_input.pause_millis`、`voice_input.stop_sensitivity` 里。共享文档的 `voice_input` 整组不进设置文件导出，账号同步（Rust 的 `account::ios_settings`）也不导出这两项，所以它们只留在这台设备上。范围、步进和缺省值与 client-core 的 `VOICE_PAUSE_MILLIS_*`、`VOICE_STOP_SENSITIVITY_*`、鸿蒙和 Android 本机设置相同。
struct VoicePauseSettings: Equatable {
  static let pauseRange = 500...5000
  static let pauseStep = 100
  /// 与 Android 键盘原来写死的停顿时间相同。
  static let defaultPauseMilliseconds = 1500
  static let sensitivityRange = 10...90
  static let defaultSensitivity = 50
  /// Silero VAD 原来写死的判定阈值，按住说话和 App 内录音照旧用它。
  static let defaultVADThreshold: Float = 0.5

  var pauseMilliseconds = Self.defaultPauseMilliseconds
  var stopSensitivity = Self.defaultSensitivity

  init() {}

  /// 缺键、类型不对、越界或不在步进上的值都按缺省读；client-core 保存时已经校验过，这里只防旧文档和手改的文件。
  init(_ preferences: [String: Any]?) {
    let voice = preferences?["voice_input"] as? [String: Any] ?? [:]
    if let pause = voice["pause_millis"] as? Int, Self.pauseRange.contains(pause),
       (pause - Self.pauseRange.lowerBound) % Self.pauseStep == 0 {
      pauseMilliseconds = pause
    }
    if let sensitivity = voice["stop_sensitivity"] as? Int, Self.sensitivityRange.contains(sensitivity) {
      stopSensitivity = sensitivity
    }
  }

  /// 一次听写用的停顿设置：点按开始的才有，按住说话为 nil，由松手结束、VAD 阈值保持 `defaultVADThreshold`。`VoiceSessionService` 只在本地模型识别时调用它。
  static func forDictation(tapStarted: Bool, preferences: [String: Any]?) -> VoicePauseSettings? {
    tapStarted ? VoicePauseSettings(preferences) : nil
  }

  /// 只写这两项，`voice_input` 的其他字段保持不变。
  func write(into preferences: inout [String: Any]) {
    var voice = preferences["voice_input"] as? [String: Any] ?? [:]
    voice["pause_millis"] = pauseMilliseconds
    voice["stop_sensitivity"] = stopSensitivity
    preferences["voice_input"] = voice
  }

  /// 判停灵敏度换成 Silero VAD 的判定阈值：灵敏度越高，阈值越高，越不容易把声音当作还在说话，停顿计时也就越早结束录音。默认 50 正是原来的 0.5；与鸿蒙的 `LocalAsrPolicy.vadThreshold` 同一个换算。
  var vadThreshold: Float { Float(stopSensitivity) / 100 }
}

/// 点按开始的本地识别按停顿结束录音，规则与 Android 的 `ImeVoiceEntry.armSilenceStop` 和鸿蒙的 `armVoiceSilence` 相同：出字后开始计时，每次出新字从头计；经过 VAD 的整段模型还判定在说话时不结束，到点顺延一个完整的停顿；说话状态变化时，已经在计的停顿从这一刻重新计。还没出字时不计时，所以开口之前不会被结束。按住说话和云端服务不用它。
@MainActor
final class VoicePauseStop {
  private let delay: UInt64
  private let onStop: () -> Void
  private var timer: Task<Void, Never>?
  private var speaking = false
  private var ended = false

  init(milliseconds: Int, onStop: @escaping () -> Void) {
    delay = UInt64(max(0, milliseconds)) * 1_000_000
    self.onStop = onStop
  }

  /// 识别出了文字（不为空）：从这一刻起重新计停顿。
  func heard() {
    arm()
  }

  /// VAD 开始或停止判定在说话。计时还没开始时只记下状态。
  func speech(_ speaking: Bool) {
    self.speaking = speaking
    if timer != nil { arm() }
  }

  /// 听写由别的原因结束了（用户点「说完了」、取消、到时长上限或出错），之后不再触发。
  func cancel() {
    ended = true
    timer?.cancel()
    timer = nil
  }

  private func arm() {
    guard !ended else { return }
    timer?.cancel()
    let delay = delay
    timer = Task { [weak self] in
      do { try await Task.sleep(nanoseconds: delay) } catch { return }
      self?.elapsed()
    }
  }

  private func elapsed() {
    guard !ended, timer != nil, !Task.isCancelled else { return }
    if speaking {
      arm()
      return
    }
    ended = true
    timer = nil
    onStop()
  }
}

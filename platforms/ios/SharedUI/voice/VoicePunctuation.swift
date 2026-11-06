import Foundation

@_silgen_name("msime_client_voice_punctuation")
private func msimeClientVoicePunctuation(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

/// 语音标点（共享偏好 `voice_input.punctuation_mode`，#10595）：识别结果上屏前去掉或替换识别器加的标点，对应 Android 的 `VoicePunctuation`。
///
/// 规则只在共享层（`msime_client_voice_punctuation`）一处，这里只认取值、调用它。只处理最终文本：所有识别服务（本地模型、系统识别、豆包、整句上传）和 AI 润色之后、上屏或存进语音交接之前，每条路径处理一次——键盘听写在 `KeyboardViewController.insertVoiceResult`，App 语音页在 `deliver`；`VoiceSessionService` 和键盘领取「语音结果」时不再处理。中间结果照原样显示。`voice_input` 整组不进账号同步，这一项只留在本机。
enum VoicePunctuation {
  /// 识别结果原样上屏，出厂默认，也是加这个选项之前的行为。
  static let auto = "auto"
  /// 设置页的四个选项，与 `labels` 一一对应。
  static let modes = [auto, "omit_trailing", "omit_all", "space"]
  /// 与 Android `VoicePunctuation.LABELS` 逐字相同。
  static let labels = ["智能添加", "省略句末标点", "省略所有标点", "空格代替标点"]

  /// 共享偏好里的取值；缺键、不是字符串或不认识（例如更新版本写下的新选项）时按 `auto`，与共享层一致。
  static func mode(_ preferences: [String: Any]?) -> String {
    let voice = preferences?["voice_input"] as? [String: Any]
    guard let value = voice?["punctuation_mode"] as? String, modes.contains(value) else { return auto }
    return value
  }

  /// 设置页显示的名字。
  static func label(_ mode: String) -> String {
    modes.firstIndex(of: mode).map { labels[$0] } ?? labels[0]
  }

  /// 只写 `voice_input.punctuation_mode`，`voice_input` 的其他字段保持不变。
  static func write(_ mode: String, into preferences: inout [String: Any]) {
    var voice = preferences["voice_input"] as? [String: Any] ?? [:]
    voice["punctuation_mode"] = mode
    preferences["voice_input"] = voice
  }

  /// 按 `mode` 处理一段最终识别结果。`auto` 和空文本不经过共享层；整段只有标点时返回空串。共享层调用失败时返回原文：标点没去掉不值得让这次识别的内容丢掉。
  static func apply(_ text: String, mode: String) -> String {
    guard !text.isEmpty, mode != auto else { return text }
    let value = ClientRequestEnvelope.call(msimeClientVoicePunctuation, ["text": text, "mode": mode]) as? [String: Any]
    return value?["text"] as? String ?? text
  }

  /// 上屏或交给键盘前的最后一步。识别出了内容、处理后什么都不剩（整段只有标点）时为 nil，调用方提示「没有识别到文字」，与 Android 一致；本来就为空白的结果原样返回，由调用方按没有听到内容处理。
  static func finalText(_ recognized: String, mode: String) -> String? {
    guard !isBlank(recognized) else { return recognized }
    let text = apply(recognized, mode: mode)
    return isBlank(text) ? nil : text
  }

  private static func isBlank(_ text: String) -> Bool {
    text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }
}

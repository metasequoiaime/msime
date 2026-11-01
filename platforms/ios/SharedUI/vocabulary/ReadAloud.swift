import AVFoundation
import Foundation

@_silgen_name("msime_client_read_aloud_text")
private func msimeClientReadAloudText(
  _ request: UnsafePointer<UInt8>?, _ length: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientReadAloudStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 长按候选菜单的「朗读」和背单词卡片的喇叭要读哪段英文，经由共享入口。
///
/// 规则在 `client-services::vocabulary::read_aloud`：取每行释义的第一个义项，第一个是英文的就是答案；没有时候选本身是英文就读候选。这里只把候选原文和显示着的释义交过去。见 `.agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md`。
enum ReadAloudText {
  /// 长按菜单里这一项的标题。
  static let menuTitle = "朗读"

  /// 要读的英文；没有能读的或请求不合法时为 `nil`，菜单和卡片上就没有这一项。不读写文件，打开菜单时在主线程调用。背单词卡片把词头当作 `candidate`、不给 `gloss`。
  static func text(candidate: String, gloss: String = "") -> String? {
    guard let data = try? JSONSerialization.data(withJSONObject: ["candidate": candidate, "gloss": gloss]),
          data.count <= 65536 else { return nil }
    let raw = data.withUnsafeBytes { bytes in
      msimeClientReadAloudText(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(bytes.count))
    }
    guard let raw else { return nil }
    defer { msimeClientReadAloudStringFree(raw) }
    guard let envelope = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any],
          envelope["ok"] as? Bool == true,
          let text = envelope["value"] as? String, !text.isEmpty else { return nil }
    return text
  }
}

/// 用系统的 `AVSpeechSynthesizer` 读一个英文单词，只用设备上已安装的声音：先找美式英语，再找任何一个英文声音。新的一次打断还在读的上一次。键盘扩展只在有完全访问时用它：没有完全访问时 iOS 不让键盘扩展播放自己的声音（见 `KeyboardSoundPlayer`）。
@MainActor
final class ReadAloudSpeaker {
  /// 没有任何英文声音时给用户看的话。
  static let noVoiceNotice = "系统没有可用的英文朗读语音"

  private var synthesizer: AVSpeechSynthesizer?

  /// 读 `text`；读不出来时返回给用户看的那句话，读出来时返回 `nil`。
  @discardableResult
  func speak(_ text: String) -> String? {
    guard !text.isEmpty else { return nil }
    guard let voice = AVSpeechSynthesisVoice(language: "en-US")
      ?? AVSpeechSynthesisVoice.speechVoices().first(where: { $0.language.hasPrefix("en") }) else {
      return Self.noVoiceNotice
    }
    let synthesizer = synthesizer ?? AVSpeechSynthesizer()
    self.synthesizer = synthesizer
    // 朗读的音频会话交给系统管理，不去改键盘按键音用的那个 `.ambient` 会话，也不改 App 自己的会话。
    synthesizer.usesApplicationAudioSession = false
    synthesizer.stopSpeaking(at: .immediate)
    let utterance = AVSpeechUtterance(string: text)
    utterance.voice = voice
    synthesizer.speak(utterance)
    return nil
  }

  /// 停下正在读的。键盘收起或页面消失时调用。
  func stop() {
    synthesizer?.stopSpeaking(at: .immediate)
  }
}

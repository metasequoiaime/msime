import Foundation

/// 行内预编辑：键盘把正在拼写的内容写进输入框（marked text）的几种形式，与 Apple 自带中文键盘和 Android 宿主一样。键盘固定用「拼音分词」，候选栏因此不再画读音，候选从行首开始、不随拼写变长左右移动；不再提供开关。`raw` 与 `off` 对应 Windows `tsf_preedit_style` 的另外两种形式，留给 `InlineCompositionPolicy` 的规则与测试使用。韩语无论哪种形式都标记它的音节（见 `InlineCompositionPolicy.markedText`）。
enum InlinePreeditPreference {
  enum Style: String, CaseIterable {
    case off, raw, pinyin

    /// The marked text for a composition. `raw` falls back to the segmented spelling when the Engine reports no ASCII keys, as a local mode or the Japanese reading does; Japanese always shows its kana, which is what was typed.
    func text(phrasePrefix: String, preedit: String, editingText: String, japaneseReading: String?) -> String {
      switch self {
      case .off: return ""
      case .pinyin: return phrasePrefix + (japaneseReading ?? preedit)
      case .raw:
        if let japaneseReading { return phrasePrefix + japaneseReading }
        let keys = !editingText.isEmpty && editingText.allSatisfy(\.isASCII) ? editingText : preedit
        return phrasePrefix + keys
      }
    }
  }
}

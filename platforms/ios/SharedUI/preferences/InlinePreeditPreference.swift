import Foundation

/// 行内预编辑: whether the keyboard also writes the composition into the text field as marked text, the way Apple's own Chinese keyboards and the Android host do, instead of showing it only on the candidate strip, and in which form.
///
/// Windows offers the same choice as `tsf_preedit_style`: 原始按键 writes the keys as typed, 拼音分词 the segmented pinyin. Off by default. Korean marks its syllable inline whatever this says (see `InlineCompositionPolicy.markedText`), since the syllable is the text being written. The shared `tsf_preedit_style` cannot carry it: every document holds its default `raw`, so honouring that would move every existing iPhone and iPad user to inline text at once, and third-party apps differ in how well they draw a keyboard extension's marked text. The choice lives in the App Group because the keyboard reads it on every keystroke.
enum InlinePreeditPreference {
  enum Style: String, CaseIterable {
    case off, raw, pinyin

    var title: String {
      switch self {
      case .off: "不显示"
      case .raw: "原始按键"
      case .pinyin: "拼音分词"
      }
    }

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

  static let styleKey = "keyboard.inline_preedit_style"
  /// The switch that came before the style. A stored `true` reads as 拼音分词, which is what it wrote into the field.
  static let key = "keyboard.inline_preedit"

  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }

  static var style: Style {
    get {
      if let stored = defaults.string(forKey: styleKey), let style = Style(rawValue: stored) { return style }
      return defaults.bool(forKey: key) ? .pinyin : .off
    }
    set { defaults.set(newValue.rawValue, forKey: styleKey) }
  }

  static var isEnabled: Bool { style != .off }
}

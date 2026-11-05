import Foundation

/// One write that brings the host's marked text in line with the composition.
enum InlineCompositionEdit: Equatable {
  /// Replace the marked text with this, caret at its end.
  case mark(String)
  /// Remove the marked text and end marking.
  case clear
}

/// 上屏时怎样把宿主里的行内预编辑换成正文。
enum InlineCommitEdit: Equatable {
  /// `setMarkedText(正文, selectedRange: caret)` 先把标记区整段换成正文，紧接 `unmarkText()` 定稿；`caret` 是正文内的 UTF-16 折叠光标位置。
  case markThenUnmark(caret: Int)
  /// `insertText(正文)`，靠宿主自己把标记区替换掉。
  case insert
}

/// The marked text a keyboard extension leaves in the host while 行内预编辑 is on.
///
/// The host document holds the marked letters as long as the composition runs, so anything that reads the text before the caret has to look past them, and a composition that ends by commit has to take them out before the committed text goes in; otherwise the letters and the conversion would both stay.
enum InlineCompositionPolicy {
  /// The write that turns `current` marked text into `next`, or nil when the host already shows it.
  static func edit(showing current: String, next: String) -> InlineCompositionEdit? {
    guard current != next else { return nil }
    return next.isEmpty ? .clear : .mark(next)
  }

  /// The marked text for a composition. A scheme that composes in place (`inPlace`: Korean, Zhuyin, Vietnamese) always marks its preedit, whatever 行内预编辑 says: a Korean syllable, a Zhuyin conversion with its pending bopomofo, or a Vietnamese word is the text being written rather than a spelling waiting to be converted, and a keyboard that kept it off the field would leave the user typing into a strip above it. Every other scheme follows the chosen style.
  ///
  /// A scheme that draws its keys as glyphs (`drawsKeysAsGlyphs`: Stroke) marks those glyphs under 原始按键 too: its `editing_text` holds the letters h s p n z x the keys send, which the user never typed as letters, and msime_client.h tells every host to draw the reading or preedit instead.
  static func markedText(inPlace: Bool, style: InlinePreeditPreference.Style, drawsKeysAsGlyphs: Bool = false,
                         phrasePrefix: String, preedit: String, editingText: String, japaneseReading: String?) -> String {
    if inPlace { return preedit }
    return style.text(phrasePrefix: phrasePrefix, preedit: preedit, editingText: drawsKeysAsGlyphs ? "" : editingText,
                      japaneseReading: japaneseReading)
  }

  /// The document text before the composition, which is what punctuation and smart-punctuation context mean by "before the caret".
  static func contextBefore(_ before: String?, marked: String) -> String? {
    guard !marked.isEmpty, let before, before.hasSuffix(marked) else { return before }
    return String(before.dropLast(marked.count))
  }

  /// 运行时上屏替换行内预编辑时的写法（#10873）。
  ///
  /// 标记区里有拼写、这次上屏后不再组字时，用 `setMarkedText` 加 `unmarkText` 显式替换标记区，不交给 `insertText`：UIKit 约定 `insertText` 替换标记区，但 Flutter 引擎的 `FlutterTextInputView` 在 2026-09（flutter/flutter#191062）之前一直只替换当前选区，标记区里的拼音会作为普通文字留在正文前面，闲鱼这类 Flutter 应用里就是拼音和汉字一起上屏。`setMarkedText` 在两类宿主里都替换标记区，`unmarkText` 只是定稿，不会再写一遍正文。
  ///
  /// 同一次快照还接着组字（部分选词、韩语等就地组字的下一个音节）时仍用 `insertText`：真实跨进程代理上，`setMarkedText(上屏)`、`unmarkText()`、`setMarkedText(剩余)` 连续调用会让剩余组字盖掉刚上屏的汉字（#10284，见 2026-10-31-ios-cantonese-partial-commit）。指定了光标后缀时沿用快捷短语定位的临时标记（`cursor`），不受这条限制。
  static func commit(_ text: String, marked: String, cursor: Int?, continuesComposition: Bool) -> InlineCommitEdit {
    if let cursor { return .markThenUnmark(caret: cursor) }
    if !marked.isEmpty && !continuesComposition { return .markThenUnmark(caret: text.utf16.count) }
    return .insert
  }
}

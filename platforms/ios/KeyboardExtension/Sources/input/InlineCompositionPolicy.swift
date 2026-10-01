import Foundation

/// One write that brings the host's marked text in line with the composition.
enum InlineCompositionEdit: Equatable {
  /// Replace the marked text with this, caret at its end.
  case mark(String)
  /// Remove the marked text and end marking.
  case clear
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
  static func markedText(inPlace: Bool, style: InlinePreeditPreference.Style, phrasePrefix: String, preedit: String,
                         editingText: String, japaneseReading: String?) -> String {
    if inPlace { return preedit }
    return style.text(phrasePrefix: phrasePrefix, preedit: preedit, editingText: editingText,
                      japaneseReading: japaneseReading)
  }

  /// The document text before the composition, which is what punctuation and smart-punctuation context mean by "before the caret".
  static func contextBefore(_ before: String?, marked: String) -> String? {
    guard !marked.isEmpty, let before, before.hasSuffix(marked) else { return before }
    return String(before.dropLast(marked.count))
  }
}

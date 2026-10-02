import Foundation

enum KeyboardPunctuationContext {
  private static let chineseInputs = [
    "，": ",", "。": ".", "？": "?", "！": "!", "、": "\\", "；": ";", "：": ":",
  ]
  private static let japaneseInputs = [
    "、": "\\", "。": ".", "？": "?", "！": "!", "「": "[", "」": "]", "・": "/",
  ]

  static func precedingScalar(_ contextBeforeInput: String?) -> UInt32 {
    contextBeforeInput?.unicodeScalars.last?.value ?? 0
  }

  /// The ASCII key the Engine punctuation route takes for `symbol`, or nil when the keyboard types the symbol itself. Korean and Vietnamese write half-width ASCII marks (`asciiMarks`), so a Chinese mark picked from the symbol panel is typed as it is there rather than folded back into its ASCII key.
  static func engineInput(for symbol: String, japanese: Bool, asciiMarks: Bool = false) -> String? {
    if symbol.utf8.count == 1, let value = symbol.utf8.first,
       (33...47).contains(value) || (58...64).contains(value)
        || (91...96).contains(value) || (123...126).contains(value) {
      return symbol
    }
    if asciiMarks { return nil }
    return (japanese ? japaneseInputs : chineseInputs)[symbol]
  }
}

import Foundation

enum KeyboardPunctuationContext {
  private static let chineseInputs = [
    "，": ",", "。": ".", "？": "?", "！": "!", "；": ";", "：": ":",
  ]
  private static let japaneseInputs = [
    "。": ".", "？": "?", "！": "!", "「": "[", "」": "]", "・": "/",
  ]

  static func precedingScalar(_ contextBeforeInput: String?) -> UInt32 {
    contextBeforeInput?.unicodeScalars.last?.value ?? 0
  }

  /// 顿号发给 Engine 的键按 View 的顿号开关（契约 `enumerationComma` 段）取开着的那个：`\` 优先、`/` 其次；两个都关按归一的 `\`。中日文的顿号走同一套开关。
  static func enumerationCommaInput(backslash: Bool, slash: Bool) -> String {
    backslash ? "\\" : (slash ? "/" : "\\")
  }

  /// The ASCII key the Engine punctuation route takes for `symbol`, or nil when the keyboard types the symbol itself. Korean and Vietnamese write half-width ASCII marks (`asciiMarks`), so a Chinese mark picked from the symbol panel is typed as it is there rather than folded back into its ASCII key. `、` 的顿号键随 View 的顿号开关变。
  static func engineInput(for symbol: String, japanese: Bool, asciiMarks: Bool = false,
                          enumerationCommaBackslash: Bool = true,
                          enumerationCommaSlash: Bool = false) -> String? {
    if symbol.utf8.count == 1, let value = symbol.utf8.first,
       (33...47).contains(value) || (58...64).contains(value)
        || (91...96).contains(value) || (123...126).contains(value) {
      return symbol
    }
    if asciiMarks { return nil }
    if symbol == "、" {
      return enumerationCommaInput(backslash: enumerationCommaBackslash, slash: enumerationCommaSlash)
    }
    return (japanese ? japaneseInputs : chineseInputs)[symbol]
  }
}

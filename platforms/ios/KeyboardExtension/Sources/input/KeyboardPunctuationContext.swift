import Foundation

enum KeyboardPunctuationContext {
  private static let chineseInputs = [
    "，": ",", "。": ".", "？": "?", "！": "!", "；": ";", "：": ":",
  ]
  /// 日语方案的标点键面与引擎标点契约 `japanese` 表逐字一致：键面在这里映射回它对应的 ASCII 键，由上屏的引擎写出同一个字。`、` 走 `\`（日语读点与中文顿号开关无关，不走 `enumerationCommaInput`）。
  private static let japaneseInputs = [
    "、": "\\", "。": ".", "？": "?", "！": "!", "「": "[", "」": "]",
    "『": "{", "』": "}", "・": "/", "〜": "~", "（": "(", "）": ")",
    "：": ":", "；": ";",
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
    // 中文的顿号键随 View 的顿号开关；日语的读点固定走 `\`，不随这个开关。
    if !japanese && symbol == "、" {
      return enumerationCommaInput(backslash: enumerationCommaBackslash, slash: enumerationCommaSlash)
    }
    return (japanese ? japaneseInputs : chineseInputs)[symbol]
  }
}

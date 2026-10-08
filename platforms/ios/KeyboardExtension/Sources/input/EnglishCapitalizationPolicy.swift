import Foundation

enum EnglishCapitalizationMode {
  case none
  case words
  case sentences
  case allCharacters
}

enum EnglishCapitalizationPolicy {
  private static let apostrophes: Set<Character> = ["'", "’"]
  private static let sentenceTerminators: Set<Character> = [".", "!", "?", "。", "！", "？"]
  private static let closingCharacters: Set<Character> = ["'", "\"", "’", "”", ")", "]", "}"]

  static func shouldShift(
    for mode: EnglishCapitalizationMode,
    contextBeforeInput: String?
  ) -> Bool {
    switch mode {
    case .none:
      return false
    case .allCharacters:
      return true
    case .words:
      guard let contextBeforeInput else { return false }
      guard let lastCharacter = contextBeforeInput.last else { return true }
      if apostrophes.contains(lastCharacter) {
        return false
      }
      let continuesWord = lastCharacter.unicodeScalars.contains {
        CharacterSet.alphanumerics.contains($0)
      }
      return !continuesWord
    case .sentences:
      guard let contextBeforeInput else { return false }
      guard !contextBeforeInput.isEmpty else { return true }
      // 句末标点后要隔着空白才算新句，与系统键盘一致；否则刚点下 `.`、删字删到 `abc.` 后面或在 `你好。` 后切到英文都会立刻变成大写。
      var whitespaceAfter = false
      for character in contextBeforeInput.reversed() {
        if character == "\n" || character == "\r" || character == "\r\n" {
          return true
        }
        if character.isWhitespace {
          whitespaceAfter = true
          continue
        }
        if closingCharacters.contains(character) {
          continue
        }
        return whitespaceAfter && sentenceTerminators.contains(character)
      }
      return true
    }
  }
}

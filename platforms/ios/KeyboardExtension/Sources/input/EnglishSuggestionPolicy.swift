import Foundation

/// 文档里保留真实按键；接受共享引擎给出的纠错时，只替换光标前这一词。
enum EnglishSuggestionPolicy {
  struct Replacement: Equatable {
    let deleteCount: Int
    let insert: String
  }

  /// 从光标向前读字母及词内撇号，全角字母规范成 ASCII；汉字、假名等是边界。
  static func currentWord(before context: String) -> String {
    var letters: [Character] = []
    for character in context.reversed() {
      let normalized: Character?
      if character == "'" || (character.isASCII && character.isLetter) {
        normalized = character
      } else if character.unicodeScalars.count == 1,
                let scalar = character.unicodeScalars.first,
                ((0xFF21...0xFF3A).contains(scalar.value) || (0xFF41...0xFF5A).contains(scalar.value)) {
        normalized = Character(String(UnicodeScalar(scalar.value - 0xFEE0)!))
      } else {
        normalized = nil
      }
      guard let normalized else { break }
      letters.append(normalized)
    }
    return String(letters.reversed())
  }

  static func replacement(typed: String, candidate: String, startedCapitalized: Bool) -> Replacement? {
    guard !candidate.isEmpty else { return nil }
    var word = candidate
    if startedCapitalized, let first = word.first {
      word = first.uppercased() + word.dropFirst()
    }
    guard word != typed else { return nil }
    return Replacement(deleteCount: typed.count, insert: word)
  }
}

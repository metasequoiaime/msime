import UIKit

// A field's preferred keyboard temporarily overrides the language, not the user's input scheme.
// Track its identity as well as its type so typing callbacks do not undo a manual language choice.
struct KeyboardInputContext {
  private var documentIdentifier: UUID?
  private(set) var keyboardType: UIKeyboardType = .default
  private var languageBeforeLatinField: Bool?

  /// The field forced English; a switch made here is about this field, and leaving it restores the mode from before.
  var isInLatinField: Bool { languageBeforeLatinField != nil }

  static func prefersLatin(_ type: UIKeyboardType) -> Bool {
    switch type {
    case .asciiCapable, .URL, .emailAddress: return true
    default: return false
    }
  }

  mutating func languageOverride(for type: UIKeyboardType, document: UUID, isChinese: Bool,
                                 temporaryEnglish: Bool = false) -> Bool? {
    guard documentIdentifier != document || keyboardType != type else { return nil }
    documentIdentifier = document
    keyboardType = type
    if Self.prefersLatin(type) {
      if languageBeforeLatinField == nil { languageBeforeLatinField = isChinese || temporaryEnglish }
      return false
    }
    defer { languageBeforeLatinField = nil }
    // 普通输入框之间换焦点也结束临时英文；同一输入框的自身回声已由开头的身份判断排除。
    return languageBeforeLatinField ?? (temporaryEnglish ? true : nil)
  }
}

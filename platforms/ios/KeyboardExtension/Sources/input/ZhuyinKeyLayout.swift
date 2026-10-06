import Foundation

/// The Dachen (大千) keys shown while the Zhuyin scheme is active.
///
/// The Engine reads Dachen as the ASCII keys of a standard keyboard (crates/engine/src/zhuyin/layout.rs, after libchewing), so the keyboard lays out those 41 keys in their physical rows, draws the bopomofo symbol or tone mark each one types, and sends the ASCII key. Space is tone 1 and needs no key of its own.
enum ZhuyinKeyLayout {
  /// The ASCII keys in their physical rows: the digit row with `-`, then the three letter rows, each letter row extended by the punctuation keys Dachen gives a symbol.
  static let rows: [[String]] = [
    ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"],
    ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
    ["a", "s", "d", "f", "g", "h", "j", "k", "l", ";"],
    ["z", "x", "c", "v", "b", "n", "m", ",", ".", "/"],
  ]

  private static let faces: [String: String] = [
    "1": "ㄅ", "2": "ㄉ", "3": "ˇ", "4": "ˋ", "5": "ㄓ", "6": "ˊ", "7": "˙", "8": "ㄚ", "9": "ㄞ", "0": "ㄢ", "-": "ㄦ",
    "q": "ㄆ", "w": "ㄊ", "e": "ㄍ", "r": "ㄐ", "t": "ㄔ", "y": "ㄗ", "u": "ㄧ", "i": "ㄛ", "o": "ㄟ", "p": "ㄣ",
    "a": "ㄇ", "s": "ㄋ", "d": "ㄎ", "f": "ㄑ", "g": "ㄕ", "h": "ㄘ", "j": "ㄨ", "k": "ㄜ", "l": "ㄠ", ";": "ㄤ",
    "z": "ㄈ", "x": "ㄌ", "c": "ㄏ", "v": "ㄒ", "b": "ㄖ", "n": "ㄙ", "m": "ㄩ", ",": "ㄝ", ".": "ㄡ", "/": "ㄥ",
  ]

  /// The bopomofo symbol or tone mark drawn on `key`, or nil for a key that is not on the Dachen layout.
  static func keycap(for key: String) -> String? {
    faces[key]
  }

  /// Whether the Engine would take `key` as a row pick while the candidate list is open: there the digits 1-9 choose a row instead of typing ㄅ, ㄉ or a tone (crates/engine/src/session/input.rs). A touch keyboard picks rows on the strip, so the keyboard closes the list before sending such a key and it types what its face shows.
  static func selectsWhileListOpen(_ key: String) -> Bool {
    key.count == 1 && key >= "1" && key <= "9"
  }
}

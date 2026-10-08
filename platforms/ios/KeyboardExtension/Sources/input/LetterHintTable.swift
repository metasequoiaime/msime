import CoreGraphics

/// 26 个字母键右上角的符号提示，取自设计稿：q1…p0 / a@ s# d¥ f% g& h* j( k) l" / z~ x… c、 v? b! n- m/。在字母键上下滑或长按即输入这个符号。与 Android 的 `LetterHintTable` 是同一张表。
enum LetterHintTable {
  private static let hints: [Character: String] = [
    "q": "1", "w": "2", "e": "3", "r": "4", "t": "5", "y": "6", "u": "7", "i": "8", "o": "9", "p": "0",
    "a": "@", "s": "#", "d": "¥", "f": "%", "g": "&", "h": "*", "j": "(", "k": ")", "l": "\"",
    "z": "~", "x": "…", "c": "、", "v": "?", "b": "!", "n": "-", "m": "/",
  ]

  /// 字母键的提示符号，大小写字母都可传入；不是 26 个 ASCII 字母之一时为 nil。
  static func hint(for letter: String) -> String? {
    guard letter.count == 1, let character = letter.first, character.isASCII else { return nil }
    return hints[Character(character.lowercased())]
  }
}

/// 字母键上的滑动输入：手指从按下处向下移动超过 `threshold` 点，抬起时输入该键的提示符号（`LetterHintTable`），按键预览也显示提示符号而不是字母。阈值和方向取 Android `SwipeHintPolicy` 的默认值；Android 设置里还有开关和「向上」方向，本键盘没有对应设置，所以始终用默认的向下滑。
enum SwipeHintPolicy {
  /// 手指需要移动的距离，必须严格大于这个值。
  static let threshold: CGFloat = 14

  /// 在 `downY` 处按下、当前位于 `currentY`（两者都向下增大）的手指是否已经下滑。
  static func swiped(downY: CGFloat, currentY: CGFloat) -> Bool {
    currentY - downY > threshold
  }
}

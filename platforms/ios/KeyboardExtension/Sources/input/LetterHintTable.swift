import CoreGraphics

/// 26 个字母键右上角的符号提示，取自设计稿：q1…p0 / a@ s# d¥ f% g& h* j( k) l" / z~ x… c、 v? b! n- m/。沿设定方向滑动字母键或长按即输入这个符号。与 Android 的 `LetterHintTable` 是同一张表。
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

/// 字母键沿设定方向滑动超过阈值时，松手输入角标；三端默认上滑，也可在设置里改成下滑。
enum SwipeHintPolicy {
  /// 手指需要移动的距离，必须严格大于这个值。
  static let threshold: CGFloat = 14

  /// 窗口纵坐标向下增大，严格超过所选方向的阈值才接管点按。
  static func swiped(direction: KeyboardLayoutPreference.SwipeDirection = .up,
                     downY: CGFloat, currentY: CGFloat) -> Bool {
    let distance = direction == .up ? downY - currentY : currentY - downY
    return distance > threshold
  }
}

import Foundation

/// 放大拼写编辑条里的光标换算，与 Android 的 `CompositionCaretPolicy` 是同一套规则（#10591，三端对齐）。
///
/// 引擎的光标是 `caret_position`，落在 `editing_text`（打下的原始按键，全拼是字母，九键是数字，全拼 14 键、18 键是每组的首字母）里；编辑条画的却不一定是这串按键：九键画的是首选读法的拼音（`9426` 画成 `xi'an`）或选过拼音后的 `ni'426`，全拼 14 键画的是 `ni'hao`（按键是 `bugao`），26 键全拼画的是带引擎分隔的 `zuo'tian`。两者按顺序对齐：只在显示时才有的分隔（`'` 和空格）跳过，字母对数字时按九键键位比，传了字母网格（`letterGroups`）时字母还按它在这张网格上的组码比，其余逐字相同。对不上的读音一律不猜位置，编辑条退回画原始按键。
///
/// 字母网格只给全拼 14 键、18 键传：双拼网格的按键是双拼码，读音是拼音，两者不按字母一一对应，照旧对不上、退回原始按键；26 键不传，`n` 不会被当成 `b`。
///
/// 下标都按字符（`Character`）算；可编辑的按键全是 ASCII，读音里出现别的字符时对不上，同样退回原始按键。
enum CompositionCaretPolicy {
  /// 把光标从当前位置移到目标位置要连发的引擎命令。
  enum Move: Equatable {
    /// 移到开头（引擎命令 6，`moveCaretToStart`）。
    case home
    /// 移到末尾（引擎命令 7，`moveCaretToEnd`）。
    case end
    /// 逐格左移这么多次（引擎命令 4，`moveCaretLeft`）。
    case left(Int)
    /// 逐格右移这么多次（引擎命令 5，`moveCaretRight`）。
    case right(Int)
  }

  /// 读音 `display` 的每个字符位置对应的光标位置：`result[i]` 是光标放在 `display` 第 i 个字符前面时，它在 `editing` 里的位置；长度是 `display.count + 1`。对不上时为 nil。
  /// - Parameter letterGroups: 按键是在这张全拼字母网格上打的组码（引擎 `KeyGrid::FourteenKey`、`KeyGrid::EighteenKey`）；26 键和双拼网格为 nil。
  static func alignment(_ display: String, _ editing: String, letterGroups: LetterKeyGrid? = nil) -> [Int]? {
    let shownKeys = Array(display)
    let typedKeys = Array(editing)
    var map = [Int](repeating: 0, count: shownKeys.count + 1)
    var shown = 0
    var typed = 0
    while shown < shownKeys.count {
      map[shown] = typed
      let face = shownKeys[shown]
      if typed < typedKeys.count && sameKey(face, typedKeys[typed], letterGroups: letterGroups) {
        shown += 1
        typed += 1
      } else if isSeparator(face) {
        shown += 1
      } else if typed < typedKeys.count && isSeparator(typedKeys[typed]) {
        typed += 1
      } else {
        return nil
      }
    }
    while typed < typedKeys.count && isSeparator(typedKeys[typed]) { typed += 1 }
    guard typed == typedKeys.count else { return nil }
    map[shownKeys.count] = typed
    return map
  }

  /// 点在读音 `tapped` 处（画出来的文字里的字符下标）时光标该去的位置；对不上时返回 nil。
  ///
  /// - Parameters:
  ///   - prefixLength: 画在读音前面、已经选好的那段词的长度；点在它上面把光标移到开头。编辑条不画前缀，传 0。
  ///   - caretMark: 画出来的光标符在读音里的位置（插在 `display` 的这个下标前），没有画光标符时为 nil。编辑条的光标是另画的竖条，不占字符，传 nil。
  static func tapTarget(prefixLength: Int = 0, display: String, editing: String, caretMark: Int? = nil, tapped: Int,
                        letterGroups: LetterKeyGrid? = nil) -> Int? {
    guard tapped >= 0, let map = alignment(display, editing, letterGroups: letterGroups) else { return nil }
    var offset = tapped - max(0, prefixLength)
    if offset <= 0 { return 0 }
    if let caretMark, caretMark >= 0, offset > caretMark { offset -= 1 }
    return map[min(offset, map.count - 1)]
  }

  /// 光标 `caret` 该画在读音的哪个下标前；光标在末尾或对不上时返回 nil。同一个光标位置对应好几处时（中间隔着只在显示时才有的分隔），按退格真正删掉的那一格来画：
  ///
  /// - 组码网格（`keyGrid`：这次组字是在九键、14 键、18 键、九键双拼的网格里打的，引擎 View 的 `grid_composing`）画在最后一处，即分隔后面（`94'|26`、14 键的 `ba'|ba`）：网格组字的退格先删光标前的用户切分。
  /// - 26 键的全拼、双拼、五笔、粤拼画在第一处，即分隔前面（`zuo|'tian`，#10133）：分隔是引擎加的，退格删的是分隔前的字母。
  static func markIndex(display: String, editing: String, caret: Int, keyGrid: Bool, letterGroups: LetterKeyGrid? = nil) -> Int? {
    guard caret >= 0, caret < editing.count, let map = alignment(display, editing, letterGroups: letterGroups) else { return nil }
    let indices = keyGrid ? Array(map.indices.reversed()) : Array(map.indices)
    return indices.first { map[$0] == caret }
  }

  /// 编辑条上光标画在读音的哪个下标前。和读音行不同，编辑条在光标位于末尾时也画光标；对不上时返回 nil。
  static func displayOffset(display: String, editing: String, caret: Int, keyGrid: Bool, letterGroups: LetterKeyGrid? = nil) -> Int? {
    guard alignment(display, editing, letterGroups: letterGroups) != nil else { return nil }
    return caret == editing.count
      ? display.count
      : markIndex(display: display, editing: editing, caret: caret, keyGrid: keyGrid, letterGroups: letterGroups)
  }

  /// 把光标从 `current` 移到 `target` 要发的命令：两头各一条命令，其余逐格左移或右移。不用移、或目标越界时为 nil。
  static func moves(current: Int, target: Int, length: Int) -> Move? {
    guard target >= 0, target <= length, target != current else { return nil }
    if target == 0 { return .home }
    if target == length { return .end }
    return target < current ? .left(current - target) : .right(target - current)
  }

  static func isSeparator(_ value: Character) -> Bool {
    value == "'" || value == " "
  }

  /// 读音上的一个字符是不是这个按键：同一个字母（不分大小写），九键数字键上的字母，或者传了字母网格时这个字母在网格上的组码（14 键的 `n` 对 `b`）。选过拼音后按键里有原字母（`ni'gao`），所以同一个字母照样算。
  static func sameKey(_ face: Character, _ key: Character, letterGroups: LetterKeyGrid? = nil) -> Bool {
    if face.lowercased() == key.lowercased() { return true }
    if let letterGroups, let letter = face.lowercased().first,
       let code = (letterGroups == .eighteenKey ? EighteenKeyLayout.key(for: letter) : FourteenKeyLayout.key(for: letter))?.code {
      return code == key.lowercased()
    }
    guard let digit = key.asciiValue, digit >= UInt8(ascii: "2"), digit <= UInt8(ascii: "9"),
          let raw = face.asciiValue else { return false }
    let letter = raw >= UInt8(ascii: "A") && raw <= UInt8(ascii: "Z") ? raw + 32 : raw
    guard letter >= UInt8(ascii: "a"), letter <= UInt8(ascii: "z") else { return false }
    return keypad[Int(letter - UInt8(ascii: "a"))] == digit
  }

  /// `a..z` 各自所在的九键数字键。
  private static let keypad = Array("22233344455566677778889999".utf8)
}

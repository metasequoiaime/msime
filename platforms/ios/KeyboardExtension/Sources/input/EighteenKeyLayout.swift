import CoreGraphics
import Foundation

/// 全拼与双拼共用的 18 键键面，与 Android 的 `EighteenKeyLayout` 和引擎的 `KeyGrid::EighteenKey`、`KeyGrid::EighteenKeyShuangpin` 同一张表：`Q WE RT Y U IO P / A SD FG H JK L / Z XC V BN M`。
///
/// 三排依次是七、六、五个字母键，所有键与第一排同宽：第二排居中，两端各空出半个键；第三排两端是分词键和 ⌫，与字母键同宽，合起来也是七个键。点一下送这一组的首字母，由引擎消歧；长按弹出两个字母，与 14 键相同。双拼下两个字母的键还可以左右横滑，限定这一下是哪个字母（`EighteenKeySwipePolicy`）。
enum EighteenKeyLayout {
  static let rows: [[FourteenKeyLayout.Key]] = [
    ["q", "we", "rt", "y", "u", "io", "p"],
    ["a", "sd", "fg", "h", "jk", "l"],
    ["z", "xc", "v", "bn", "m"],
  ].map { $0.map { FourteenKeyLayout.Key(letters: $0, grid: .eighteenKey) } }

  /// `letter` 所在的那一键；不是 a–z 时为 nil。
  static func key(for letter: Character) -> FourteenKeyLayout.Key? {
    rows.joined().first { $0.letters.contains(letter) }
  }

  /// `a` 到 `z` 依次落在的组码，与引擎 `EIGHTEEN_KEYPAD` 的 `abxswffhijjlmbipqrsruvwxyz` 相同。
  static var codeTable: String {
    "abcdefghijklmnopqrstuvwxyz".map { key(for: $0)?.code ?? "?" }.joined()
  }
}

/// 18 键双拼的横滑：两个字母的键左滑取左边的字母，右滑取右边的字母，松手时把这个字母作为精确组码交给当前双拼网格。规则与 Android 的 `EighteenKeySwipePolicy` 相同：横向位移超过 24pt 且大于纵向位移的 1.5 倍才接手，轻微抖动和纵向移动仍是普通点按。
///
/// 送出的是大写 ASCII：`msime_client_grid_key` 只在十八键双拼里把 A–Z 当作同一网格里的精确小写字母，它不是 Shift，也不上屏英文字母。
enum EighteenKeySwipePolicy {
  static let threshold: CGFloat = 24

  /// 位移 (`dx`, `dy`) 选中的字母，大写；还没越过阈值、纵向为主或键面不是两个小写字母时为 nil。
  static func letter(_ letters: String, dx: CGFloat, dy: CGFloat) -> Character? {
    let characters = Array(letters)
    guard characters.count == 2, characters.allSatisfy({ ("a"..."z").contains($0) }),
          dx.isFinite, dy.isFinite, abs(dx) > threshold, abs(dx) > 1.5 * abs(dy) else { return nil }
    return Character(characters[dx < 0 ? 0 : 1].uppercased())
  }
}

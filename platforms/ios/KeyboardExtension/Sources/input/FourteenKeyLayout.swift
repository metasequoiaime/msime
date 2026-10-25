import Foundation

/// 全拼与双拼共用的 14 键键面。三端同一张表：Android、HarmonyOS 的 `FourteenKeyLayout` 和引擎的 `KeyGrid::FourteenKey`。
///
/// QWERTY 的位置不变，相邻两个字母合成一个键，L、M 各占一键：`QW ER TY UI OP / AS DF GH JK L / ZX CV BN M`，第二排不缩进。点一下输入的是这一组而不是某个字母：键盘经 `msime_client_grid_key` 把这一组的首字母（组码 `q e t u o a d g j l z c b m`）交给引擎，由引擎消歧。长按弹出这一键的两个字母（L、M 不弹），选中后与九键一样先结束组字，再上屏这个字母。
enum FourteenKeyLayout {
  struct Key: Equatable {
    /// 这一组的字母，小写，按键面上的顺序：`qw`。
    let letters: String
    /// 键所在的网格：18 键（`EighteenKeyLayout`）复用这里的组码、键面、读屏和长按，统计 id 另取 `Eighteen` 前缀。
    var grid: LetterKeyGrid = .fourteenKey

    /// 送给引擎的组码：这一组的首字母。引擎收组里任一字母都会归成同一组，三端约定送首字母。
    var code: String { String(letters.prefix(1)) }

    /// 键面：大写字母，两个字母的键写两个，没有角标。
    var face: String { letters.uppercased() }

    /// 无障碍标签：两个字母的键读「按键 Q W」，单字母键读「字母 L」。
    var accessibilityLabel: String {
      letters.count == 1 ? "字母 \(face)" : "按键 " + face.map(String.init).joined(separator: " ")
    }

    /// 微软双拼的分号与 L 共键，键面和读屏都说明这个入口；组码与统计 id 不变。
    func face(semicolon: Bool) -> String { semicolon && letters == "l" ? "L;" : face }

    func accessibilityLabel(semicolon: Bool) -> String {
      semicolon && letters == "l" ? "字母 L 和分号" : accessibilityLabel
    }

    /// 长按弹出的字母，小写；单字母键没有长按选项。
    var holdLetters: [String] { letters.count > 1 ? letters.map(String.init) : [] }

    /// 打字统计里这一键的 id：14 键是 `FourteenQW`，18 键是 `EighteenWE`。
    var keyID: String? { grid == .eighteenKey ? TypingKeyID.eighteenKey(face) : TypingKeyID.fourteenKey(face) }
  }

  /// 三排按键。第三排两端另有分词键和 ⌫，由键盘自己画。
  static let rows: [[Key]] = [
    ["qw", "er", "ty", "ui", "op"],
    ["as", "df", "gh", "jk", "l"],
    ["zx", "cv", "bn", "m"],
  ].map { $0.map { Key(letters: $0) } }

  /// `letter` 所在的那一键；不是 a–z 时为 nil。
  static func key(for letter: Character) -> Key? {
    rows.joined().first { $0.letters.contains(letter) }
  }

  /// `a` 到 `z` 依次落在的组码，与引擎 `KeyGrid::FourteenKey` 的编码表 `abcdedggujjlmbooqeatucqztz` 相同。
  static var codeTable: String {
    "abcdefghijklmnopqrstuvwxyz".map { key(for: $0)?.code ?? "?" }.joined()
  }
}

import Foundation

/// 笔画方案的六个键：五种笔画加一个通配。
///
/// Engine 读的是小写字母（crates/engine/src/stroke）：h 横、s 竖、p 撇、n 点、z 折，x 匹配任意一笔。键面画笔画字形，与 Engine 预编辑里的字形一致；键发送字母，走 `handleCharacter`，从不走标点路径。
enum StrokeKeyLayout {
  struct Key: Equatable {
    /// 发给 Engine 的字母。
    let ascii: String
    /// 键面上的笔画字形，也是 Engine 预编辑里这一笔的样子。
    let face: String
    /// 笔画名，画在字形下面，也是 VoiceOver 读的名字。
    let name: String
  }

  /// 通配键的字母。空组合时 Engine 不拿它开始组合（返回未处理），所以键盘只在组合中让它可按。
  static let wildcard = "x"

  /// 2×3 网格：横 竖 撇 / 点 折 通配。
  static let rows: [[Key]] = [
    [Key(ascii: "h", face: "一", name: "横"), Key(ascii: "s", face: "丨", name: "竖"), Key(ascii: "p", face: "丿", name: "撇")],
    [Key(ascii: "n", face: "丶", name: "点"), Key(ascii: "z", face: "乛", name: "折"), Key(ascii: wildcard, face: "＊", name: "通配")],
  ]

  /// `key` 的键面字形；不是笔画键时为 nil。
  static func keycap(for key: String) -> String? {
    rows.joined().first { $0.ascii == key }?.face
  }

  /// 按下 `key` 时这一下是否交给 Engine：五种笔画随时可以，通配只在已有笔画时可以。
  static func accepts(_ key: String, composing: Bool) -> Bool {
    guard keycap(for: key) != nil else { return false }
    return composing || key != wildcard
  }
}

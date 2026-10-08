import CoreGraphics

/// 设计稿的 123 层与 #+= 层，移植自 Android `KeyboardLayout` 中「新设计的 123 层与 #+= 层」那一段（`numberLayer` / `moreSymbolLayer`）。
///
/// 每层四行：十个键，再十个键，然后是层切换键加五个标点键和 ⌫，最后是各层自己的底行（回到字母、表情或符号、空格、回车）。中文版放的是中文写作常用的全角标点，英文版放对应的 ASCII 符号。层切换键、⌫ 和底行由键盘控制器自己画；这几张表只给出字符键、键面和宽度。
enum SymbolLayerLayout {
  /// 第三行两端的键（`#+=` / `123` 和 ⌫）相对一个标点键的宽度，即 Android 的 `LAYER_EDGE_WEIGHT`。
  static let edgeWeight: CGFloat = 1.4
  /// 该层底行各键的宽度，对应 Android 的 `LAYER_*_WEIGHT`：回到字母 1.25，表情或符号 1.05，空格 6，回车 1.9。地球键只有 iOS 在这里加，占一个键的份额。
  static let lettersWeight: CGFloat = 1.25
  static let panelWeight: CGFloat = 1.05
  static let globeWeight: CGFloat = 1
  static let spaceWeight: CGFloat = 6
  static let returnWeight: CGFloat = 1.9

  private static let numberDigits = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]
  private static let chineseNumberSymbols = ["-", "/", "：", "；", "（", "）", "¥", "@", "“", "”"]
  private static let englishNumberSymbols = ["-", "/", ":", ";", "(", ")", "$", "@", "\"", "'"]
  private static let chinesePunctuation = ["。", "，", "、", "？", "！"]
  private static let englishPunctuation = [".", ",", "?", "!", "…"]
  private static let moreSymbolsFirst = ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="]
  private static let chineseMoreSymbolsSecond = ["_", "\\", "|", "~", "《", "》", "€", "&", "·", "…"]
  private static let englishMoreSymbolsSecond = ["_", "\\", "|", "~", "<", ">", "€", "&", "·", "£"]

  /// 一层的字符键，从上到下：两行各十个，然后是层切换键与 ⌫ 之间的五个标点键。`more` 选 #+= 层，`chinese` 选中文版。
  static func characterRows(more: Bool, chinese: Bool) -> [[String]] {
    let punctuation = chinese ? chinesePunctuation : englishPunctuation
    if more {
      return [moreSymbolsFirst, chinese ? chineseMoreSymbolsSecond : englishMoreSymbolsSecond, punctuation]
    }
    return [numberDigits, chinese ? chineseNumberSymbols : englishNumberSymbols, punctuation]
  }

  /// 第三行的第一个键：123 层上是 `#+=`，#+= 层上是 `123`。
  static func toggleTitle(more: Bool) -> String { more ? "123" : "#+=" }

  /// VoiceOver 对层切换键的称呼，与 Android 对它的描述相同。
  static func toggleLabel(more: Bool) -> String { more ? "切换到数字和符号" : "更多符号" }

  /// 底行回到字母的键：中文是拼音，英文是 ABC。
  static func lettersTitle(chinese: Bool) -> String { chinese ? "拼音" : "ABC" }

  /// 字符键怎样写出它的键面。
  enum Input: Equatable {
    /// 带着这个键走键盘的符号键路径：Engine 的标点路由（智能标点、成对标点）、组字中的拼写符号，以及数字按编号选候选。
    case symbol(String)
    /// 先结束组字，再把键面原样插入，与 Android `commitNineKeyLiteral` 对每个层键的做法相同。
    case literal(String)
  }

  /// Engine 对某个 ASCII 键一对一写出的中文标点（`contracts/punctuation/policy.json` 的 `simple`）。中文标点开着时，键面是其中之一的键发送对应的 ASCII 键，这样标点路由、智能标点和成对标点照样生效，写出来的也正是键上那个标点；中文标点关闭或标点锁定为英文时 Engine 会写出 ASCII 标点，这些键改为按键面原样插入。引号在 Engine 里会交替、《 》会嵌套，而它的 ￥ 也不是键上的 ¥，所以这几个按键面原样插入。
  private static let chineseLayerKeys: [String: String] = [
    "，": ",", "。": ".", "？": "?", "！": "!", "；": ";", "：": ":", "（": "(", "）": ")", "、": "\\",
  ]

  /// 中文模式下 Engine 会转成中文标点的 ASCII 键（`policy.json`：`simple` 表、两种引号和嵌套书名号的键）。中文层上键面就是这些 ASCII 符号本身，所以不能走那条路由，否则 `[` 会变成【、`\` 会变成、。
  private static let engineTranslatedKeys: Set<String> = [
    ",", ".", "?", "!", ";", ":", "(", ")", "[", "]", "\\", "`", "$", "^", "_", "\"", "'", "<", ">",
  ]

  /// 键面为 `face` 的键怎样写出它。数字一律走符号键路径，这样组字时 1-9 能选候选，韩文、越南文或藏文的组字也会在数字之前结束。英文层上所有 ASCII 符号也走这条路径，没有 ASCII 键能打出的符号（… € · £）直接插入。中文层上 Engine 一对一写出的标点在 `chinesePunctuation`（Engine 此刻写中文标点）时发送对应的 ASCII 键，否则与 Android 设计层一样按键面原样插入，键面与写出的字始终一致；Engine 原样放行的 ASCII 键面（`- / @ { } # % * + = | ~ &`）保留符号键路径，让网址或拼写符号仍能进入组字；其余的按键面原样插入。
  static func input(for face: String, chinese: Bool, chinesePunctuation: Bool) -> Input {
    let ascii = face.unicodeScalars.count == 1 && face.unicodeScalars.allSatisfy { $0.isASCII && $0.value > 0x20 && $0.value < 0x7F }
    if ascii, let scalar = face.unicodeScalars.first, ("0"..."9").contains(scalar) { return .symbol(face) }
    guard chinese else { return ascii ? .symbol(face) : .literal(face) }
    if let key = chineseLayerKeys[face] { return chinesePunctuation ? .symbol(key) : .literal(face) }
    return ascii && !engineTranslatedKeys.contains(face) ? .symbol(face) : .literal(face)
  }
}

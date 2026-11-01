import CoreGraphics
import Foundation

/// 九键左侧符号栏的两张表：字母层（拼音九键、九键双拼、笔画字母层）和数字层（所有九宫格数字键盘的左列），共享文档里是 `touch_nine_key_sidebar_symbols` 与 `touch_nine_key_digit_sidebar_symbols`（client-core `Preferences`），存在 `TouchKeyLayoutStyle` 里，随它一起抄进 App Group 镜像。空表就是 iOS 出厂表，画面与原来相同。三端同一套上限、可加入的符号、标题和说明，见 .agents/notes/implemented/feature/2026-10-29-nine-key-sidebar-shared.md。
enum NineKeySidebarTable: String, CaseIterable, Hashable {
  /// 字母层那张表。
  case letter
  /// 数字层那张表。
  case digit

  /// 设置页导航行和编辑页的标题，三端逐字相同。
  var title: String {
    switch self {
    case .letter: NineKeySidebar.letterTitle
    case .digit: NineKeySidebar.digitTitle
    }
  }

  /// 编辑页顶部的一行说明，三端逐字相同。
  var note: String {
    switch self {
    case .letter: NineKeySidebar.letterNote
    case .digit: NineKeySidebar.digitNote
    }
  }

  /// iOS 的出厂表：字母层是原来的 ，。？，数字层是原来的四个运算符。与 Android、鸿蒙的出厂表有意不同。
  var defaults: [String] {
    switch self {
    case .letter: NineKeySidebar.defaultLetterSymbols
    case .digit: NineKeySidebar.defaultDigitSymbols
    }
  }

  /// 共享文档里的字段名。
  var documentKey: String {
    switch self {
    case .letter: TouchKeyLayoutStyle.nineKeySidebarDocumentKey
    case .digit: TouchKeyLayoutStyle.digitSidebarDocumentKey
    }
  }
}

/// 九键左侧符号栏的规则：上限、规范化与整表校验（与 client-core `normalize_nine_key_sidebar_symbols` 同一套）、设置页摘要、可加入的常用符号、成对符号，以及一屏放几项。
enum NineKeySidebar {
  /// 一张表最多几项，与 client-core `NINE_KEY_SIDEBAR_MAX_SYMBOLS`、Android `NineKeySidebarPolicy.MAX_SYMBOLS` 相同。
  static let maxSymbols = 30
  /// 一项最多几个 Unicode 码位（不是字素：👍🏽 两个码位可以，👨‍👩‍👧 五个码位不行），与 client-core `NINE_KEY_SIDEBAR_MAX_SYMBOL_CODE_POINTS` 相同。
  static let maxSymbolCodePoints = 4
  /// 用一个空格连起来的写法最多几个 UTF-16 单元，与 client-core `NINE_KEY_SIDEBAR_MAX_TEXT_UTF16`、Android `MAX_TEXT_LENGTH` 相同。
  static let maxTextUTF16 = 200

  /// iOS 字母层的出厂表，即键盘原来的 `nineKeySidebarMarks`。
  static let defaultLetterSymbols = ["，", "。", "？"]
  /// iOS 数字层的出厂表，即数字九宫格左列原来的四个运算符；#+= 不在表里，固定在最后。
  static let defaultDigitSymbols = ["+", "-", "*", "/"]
  /// 半角语境（英文、本地模式、韩文、越南文、藏文和其他写 ASCII 标点的方案，即数字九宫格右列结束标点是 ASCII `!` 的那些）下，数字层出厂表之后按这个顺序追加的半角符号，三端同一顺序：原来横排 123 页上的常用符号，邮箱框里的 `@` 由此仍在键盘层上。中文和日文不追加，用户自定义过数字层的表时也不追加（`TouchKeyLayoutStyle.numberPadSidebarSymbols`）。
  static let asciiDigitExtras = ["@", ":", ";", "(", ")", "'", "\"", "$"]
  /// 右侧大回车用出厂表时接在字母层末尾的 ！（右列的 ！ 让给了回车）；自定义表不接。
  static let tallReturnClosingMark = "！"

  /// 编辑页里可以点一下加入的常用符号，顺序固定，逐项照抄 Android `NineKeySidebarPolicy.ADDABLE_SYMBOLS`。
  static let addableSymbols = [
    "，", "。", "？", "！", "、", "；", "：", "…", "……", "—", "·",
    "“", "”", "‘", "’", "（", "）", "（）", "《", "》", "【", "】", "～",
    "+", "-", "*", "/", "=", "%", "(", ")", "()", ":", "@", "#", "&",
    "_", "|", "<", ">", "^", "￥", "★", "☆", "♡", "♥", "→", "←", "↑", "↓", ":-)",
  ]

  /// 成对符号的前半个和后半个，逐项照抄 Android `PairedPunctuationPolicy.SYMBOL_PAIRS`。表项正好是其中一对（如 `（）`、`()`）时上屏后光标停在两者中间。
  static let pairedSymbols: [String: String] = [
    "（": "）", "《": "》", "〈": "〉", "【": "】",
    "「": "」", "『": "』", "〔": "〕", "〖": "〗",
    "＜": "＞", "｛": "｝", "［": "］",
    "“": "”", "‘": "’",
    "(": ")", "[": "]", "{": "}",
  ]

  static let letterTitle = "九键左侧符号"
  static let digitTitle = "数字键盘左侧符号"
  static let letterNote = "拼音九键、九键双拼和笔画共用，多于一屏时上下滑动。"
  static let digitNote = "所有九宫格数字键盘共用，#+= 固定在最后。"

  /// 设置页摘要最多列几项，与 Android `NineKeySidebarPolicy.summary(symbols, 4)` 相同。
  static let summaryLimit = 4
  /// 每项至少多高（pt），与 Android `MIN_ROW_HEIGHT_DP` 相同：键盘调到最矮时一屏少放几项，不把键压扁。
  static let minRowHeight: CGFloat = 28
  /// 字母层一屏最多几项：与九宫格三行对齐。
  static let letterVisibleRows = 3

  /// 一项是否能放进表：去掉首尾空白后不为空、不超过 `maxSymbolCodePoints` 个码位、中间没有空白（含全角空格、NBSP）和控制字符。与 client-core 的逐项规则相同，只是这里不替人去首尾空白。
  static func isValidSymbol(_ symbol: String) -> Bool {
    let scalars = symbol.unicodeScalars
    return !scalars.isEmpty && scalars.count <= maxSymbolCodePoints
      && !scalars.contains { $0.properties.isWhitespace || $0.properties.generalCategory == .control }
  }

  /// client-core `normalize_nine_key_sidebar_symbols` 的同一套规则：每项去首尾空白，丢掉空串、含空白或控制字符的、超过 4 个码位的项和重复项（保留第一次出现的位置），到 30 项或再加一项会让空格连起来的写法超过 200 个 UTF-16 单元时截掉后面的。读别处写的文档时逐项丢坏值，不让一项连累整张表。
  static func normalize(_ symbols: [String]) -> [String] {
    var normalized: [String] = []
    var units = 0
    for symbol in symbols {
      let value = trimmed(symbol)
      guard isValidSymbol(value), !normalized.contains(where: { same($0, value) }) else { continue }
      let added = value.utf16.count + (normalized.isEmpty ? 0 : 1)
      if normalized.count == maxSymbols || units + added > maxTextUTF16 { break }
      units += added
      normalized.append(value)
    }
    return normalized
  }

  /// 去掉首尾空白（Unicode White_Space，与 Rust `str::trim` 相同）。
  private static func trimmed(_ symbol: String) -> String {
    let scalars = Array(symbol.unicodeScalars)
    guard let start = scalars.firstIndex(where: { !$0.properties.isWhitespace }),
          let end = scalars.lastIndex(where: { !$0.properties.isWhitespace }) else { return "" }
    var view = String.UnicodeScalarView()
    view.append(contentsOf: scalars[start...end])
    return String(view)
  }

  /// 按码位逐个比较，与 client-core 比较字节相同；Swift 的 `==` 按标准等价比较，会把组合前后的 é 当成同一项。
  static func same(_ lhs: String, _ rhs: String) -> Bool {
    lhs.unicodeScalars.elementsEqual(rhs.unicodeScalars)
  }

  /// 在 `symbols` 末尾按 `asciiDigitExtras` 的顺序接上表里还没有的半角符号：按码位比较，已有的跳过，原有各项的位置不动。
  static func appendingAsciiDigitExtras(to symbols: [String]) -> [String] {
    symbols + asciiDigitExtras.filter { extra in !symbols.contains { same($0, extra) } }
  }

  /// 两张表逐项、同顺序按码位相同。
  static func same(_ lhs: [String], _ rhs: [String]) -> Bool {
    lhs.count == rhs.count && zip(lhs, rhs).allSatisfy { same($0, $1) }
  }

  /// 文档或 App Group 里存的值：字符串数组里不是字符串的元素丢掉，再规范化；不是数组时是空表（出厂表）。
  static func symbols(in value: Any?) -> [String] {
    guard let items = value as? [Any] else { return [] }
    return normalize(items.compactMap { $0 as? String })
  }

  /// 编辑页的整表校验，照 Android `NineKeySidebarPolicy.parse`：不为空、不超过 30 项、没有重复、每项合规、空格连起来不超过 200 个 UTF-16 单元。不合规时确认键不可点。合规的表规范化后不变，client-core 的 `validate` 才收。
  static func isValid(_ symbols: [String]) -> Bool {
    !symbols.isEmpty && symbols.count <= maxSymbols
      && symbols.indices.allSatisfy { index in !symbols[..<index].contains { same($0, symbols[index]) } }
      && symbols.allSatisfy(isValidSymbol) && symbols.joined(separator: " ").utf16.count <= maxTextUTF16
  }

  /// 写进文档的值：与本端出厂表逐项、同顺序相同时写空数组，否则写规范化后的表。「恢复默认」就是写空数组。
  static func stored(_ symbols: [String], for table: NineKeySidebarTable) -> [String] {
    let normalized = normalize(symbols)
    return same(normalized, table.defaults) ? [] : normalized
  }

  /// 编辑页「当前顺序」每行符号前的序号：第 `position` 位（从 0 起）写成「1.」「2.」…，与 Android `NineKeySidebarPolicy.ordinal`、共享设置页 `nineKeySidebarOrdinal` 同一写法。
  static func ordinal(_ position: Int) -> String {
    "\(position + 1)."
  }

  /// 设置页导航行右侧的摘要，照 Android `NineKeySidebarPolicy.summary(symbols, 4)`：不超过 4 项时用空格连起来，多于 4 项时前 4 项加「 等 N 个」。
  static func summary(_ symbols: [String]) -> String {
    guard symbols.count > summaryLimit else { return symbols.joined(separator: " ") }
    return symbols.prefix(summaryLimit).joined(separator: " ") + " 等 \(symbols.count) 个"
  }

  /// 还不在当前表里的可加入符号，保持 `addableSymbols` 的顺序。
  static func addable(excluding current: [String]) -> [String] {
    addableSymbols.filter { symbol in !current.contains { same($0, symbol) } }
  }

  /// 表项正好是一对成对符号（前半个是 `pairedSymbols` 里的一个、后面紧跟它的后半个）时的后半个，否则为 nil，照 Android `PairedPunctuationPolicy.literalPairClosing`。
  static func literalPairClosing(_ text: String) -> String? {
    guard let first = text.unicodeScalars.first else { return nil }
    let opening = String(first)
    guard let closing = pairedSymbols[opening], same(text, opening + closing) else { return nil }
    return closing
  }

  /// 一屏放几项，照 Android `NineKeySidebarPolicy.rowHeight` 的规则：不超过 `limit` 项时按实际项数均分高度，超过时一屏 `limit` 项、其余上下滚动；每项会低于 `minRowHeight` 时少放几项，至少一项。没有项时为 0。
  static func visibleRows(count: Int, limit: Int, height: CGFloat) -> Int {
    guard count > 0 else { return 0 }
    let rows = min(count, max(limit, 1))
    let fitting = height > 0 ? Int((height / minRowHeight).rounded(.down)) : rows
    return max(min(rows, fitting), 1)
  }
}

/// 编辑页正在改的那张表，照 Android `NineKeySidebarSheet` 的交互：从当前表起步（空表时是出厂表），可删除、可调整顺序、可从常用符号或手动输入加一项、可恢复默认；重复项、不合规的项和放不下的项在加入时就挡掉，整表不合规时不能保存。保存只写这一张表（`TouchKeyLayoutStyle.save(sidebar:table:)`），与出厂表相同时写空数组。
struct NineKeySidebarDraft: Equatable {
  let table: NineKeySidebarTable
  private(set) var symbols: [String]

  init(table: NineKeySidebarTable, style: TouchKeyLayoutStyle) {
    self.table = table
    symbols = style.sidebarSymbols(table)
  }

  /// 还能点一下加入的常用符号。
  var addable: [String] { NineKeySidebar.addable(excluding: symbols) }

  /// 整表合规（`NineKeySidebar.isValid`）时保存键才可点。
  var canSave: Bool { NineKeySidebar.isValid(symbols) }

  /// 当前表就是出厂表时「恢复默认」不可点。
  var isDefault: Bool { NineKeySidebar.same(symbols, table.defaults) }

  /// 加入一项前的检查：去掉首尾空白后合规、不与已有的项重复、表里还不到 30 项、加上它空格连起来不超过 200 个 UTF-16 单元。
  func canAdd(_ symbol: String) -> Bool {
    NineKeySidebar.isValid(symbols + [symbol.trimmingCharacters(in: .whitespacesAndNewlines)])
  }

  /// 把一项接到表末尾；挡掉时返回 false，表不变。
  @discardableResult
  mutating func add(_ symbol: String) -> Bool {
    guard canAdd(symbol) else { return false }
    symbols.append(symbol.trimmingCharacters(in: .whitespacesAndNewlines))
    return true
  }

  mutating func remove(at offsets: IndexSet) {
    for index in offsets.sorted(by: >) where symbols.indices.contains(index) { symbols.remove(at: index) }
  }

  /// 与 SwiftUI `onMove` 相同的语义：把 `source` 这几项挪到原表的 `destination` 之前。
  mutating func move(from source: IndexSet, to destination: Int) {
    let moving = source.sorted().filter { symbols.indices.contains($0) }
    let items = moving.map { symbols[$0] }
    let target = destination - moving.filter { $0 < destination }.count
    for index in moving.reversed() { symbols.remove(at: index) }
    symbols.insert(contentsOf: items, at: min(max(target, 0), symbols.count))
  }

  mutating func restoreDefaults() {
    symbols = table.defaults
  }

  /// 写进文档的值：与出厂表相同时是空数组。
  var stored: [String] { NineKeySidebar.stored(symbols, for: table) }

  /// 编辑页「当前顺序」逐行要画的东西，与 `symbols` 同序。
  var rows: [NineKeySidebarRow] {
    symbols.enumerated().map { NineKeySidebarRow(index: $0.offset, symbol: $0.element) }
  }
}

/// 编辑页「当前顺序」的一行：序号、符号和 VoiceOver 念的那一句（#10396）。
struct NineKeySidebarRow: Identifiable, Equatable {
  let index: Int
  let symbol: String

  /// 按码位取的标识。表里没有码位相同的两项（`NineKeySidebar.isValid`），挪动后一行的标识跟着符号走，SwiftUI 把落下当成换位置而不是换内容，序号随之刷新；不用 `\.self`，因为 Swift 的 `==` 按标准等价比较，组合前后的 é 在表里是两项，却会被当成同一个标识。
  var id: [UInt32] { symbol.unicodeScalars.map(\.value) }

  /// 符号前的序号，「1.」起。
  var ordinal: String { NineKeySidebar.ordinal(index) }

  /// VoiceOver 念的一句：位置和符号一起念，序号本身不单独念。
  var accessibilityLabel: String { "第 \(index + 1) 项，符号 \(symbol)" }
}

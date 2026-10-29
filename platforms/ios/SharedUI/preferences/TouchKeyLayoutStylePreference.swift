import CoreGraphics
import Foundation

/// 「按键布局」里 26 键的底行样式，共享文档里是 `touch_twenty_six_key_style`（client-core `TouchTwentySixKeyStyle`），作用于手机中文 26 键字母层；英文 26 键和 iPad 全尺寸键盘不受它影响。各样式的排法见 `TouchBottomRowLayout`。
enum TouchTwentySixKeyStyle: String, CaseIterable, Hashable {
  /// 123 | 符 | 空格 | ，（长按出。）| 中/英 | 回车，即原来的底行。
  case standard
  /// 123 | 符 | ， | 空格 | 。 | 中/英 | 回车。
  case commaPeriod = "comma_period"
  /// 123 | ， | 长空格 | 中/英 | 回车，符号入口移到工具栏，逗号上滑、下滑或长按输入句号。
  case longSpace = "long_space"
  /// 123 | 😀 | 空格 | ， | 中/英 | 回车，符号面板经长按 123 打开。
  case emoji

  var title: String {
    switch self {
    case .standard: "标准"
    case .commaPeriod: "逗号句号"
    case .longSpace: "长空格"
    case .emoji: "表情键"
    }
  }
}

/// 14 键与 18 键共用的底行样式，共享文档里是 `touch_fourteen_key_style`（client-core `TouchFourteenKeyStyle`）。
enum TouchFourteenKeyStyle: String, CaseIterable, Hashable {
  /// 123 | ， | 空格 | 。 | 中/英 | 回车，即原来的底行。
  case standard
  /// 123 | ， | 长空格 | 中/英 | 回车，句号的份额给空格。
  case longSpace = "long_space"
  /// 123 | 😀 | 空格 | ， | 中/英 | 回车。
  case emoji

  var title: String {
    switch self {
    case .standard: "标准"
    case .longSpace: "长空格"
    case .emoji: "表情键"
    }
  }
}

/// 九键字母层的底行样式，拼音九键、九键双拼和笔画共用，共享文档里是 `touch_nine_key_style`（client-core `TouchNineKeyStyle`）。除右侧大回车外，左侧符号栏、右侧功能列和数字层不变。
enum TouchNineKeyStyle: String, CaseIterable, Hashable {
  /// 符 | 123 | 空格 | 中/英 | 回车，即原来的底行。
  case standard
  /// 符 | 123 | ， | 空格 | 。 | 中/英 | 回车。
  case commaPeriod = "comma_period"
  /// 符 | 123 | 😀 | 空格 | 中/英 | 回车。
  case emoji
  /// 右侧大回车：符号栏下方是 符，九宫格下方是 123 |（🌐）| 空格 | 中/英，右列 ⌫、中间键之下的回车跨第三行和底行；右列原来的 ！ 接到符号栏末尾。数字层照同一框架画。排法见 `TouchBottomRowLayout.nineKeyTallReturn`。
  case tallReturn = "tall_return"

  var title: String {
    switch self {
    case .standard: "标准"
    case .commaPeriod: "底栏带标点"
    case .emoji: "表情键"
    case .tallReturn: "右侧大回车"
    }
  }
}

/// 九宫格数字键盘里空格的位置，共享文档里是 `touch_number_pad_space`（client-core `TouchNumberPadSpace`）；0 总在底行，右侧大回车的数字层不受它影响。三端出厂排法不同，文档缺键或写 `platform_default` 时 iOS 按原来的 `afterZero` 画。
enum TouchNumberPadSpace: String, CaseIterable, Hashable {
  /// 空格在右列最下面一格（⌫ | . | 空格，替下 ！），0 占底行里原来 0 和空格合起来的宽度；Android 的出厂排法。
  case rightColumn = "right_column"
  /// 0 和空格都在底行，空格在 0 右边、与中/英同宽；iOS 的出厂排法。
  case afterZero = "after_zero"
  /// 0 和空格都在底行，空格在 0 左边占大份，0 与中/英同宽；鸿蒙的出厂排法。
  case beforeZero = "before_zero"

  /// 文档里没选过（缺键、`platform_default` 或不认识的写法）时 iOS 画的排法。
  static let platformDefault: TouchNumberPadSpace = .afterZero

  /// 设置页的选项名，三端逐字相同。
  var title: String {
    switch self {
    case .rightColumn: "空格在右列"
    case .afterZero: "空格在 0 右边"
    case .beforeZero: "空格在 0 左边"
    }
  }
}

/// 中/英键在空格哪一边，共享文档里是 `touch_language_key_side`（client-core `TouchLanguageKeySide`）。`left` 时手机触屏底行按 `TouchBottomRowLayout.languageBeforeSpace` 把中/英排到空格（和贴着空格的 0、逗号、句号）左边；iPad 全尺寸底行、123 / #+= 层和假名九键不受影响。
enum TouchLanguageKeySide: String, CaseIterable, Hashable {
  /// 中/英在空格右边、紧挨回车，即原来的底行。
  case right
  /// 中/英在空格左边，排在贴着空格的 0、逗号、句号之前。
  case left
}

/// 触屏键盘的按键布局：三类键面各自的底行样式、顶部数字行（`touch_number_row`）、26 键「符」与「123」互换（`touch_twenty_six_symbol_first`）、数字键盘空格位置（`touch_number_pad_space`）、九键左侧符号栏的两张表（`touch_nine_key_sidebar_symbols`、`touch_nine_key_digit_sidebar_symbols`）和中/英键在空格哪一边（`touch_language_key_side`），三端共用同一组共享偏好，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md、2026-10-29-touch-symbol-first-and-number-pad-space.md、2026-10-29-nine-key-sidebar-shared.md 与 2026-10-29-language-key-left-of-space.md。
///
/// 共享文档是权威来源，键盘从 App Group 镜像读：键盘和设置页读到文档时刷新镜像（`mirror`），设置页保存时文档接受了才写镜像，与 `TouchShuangpinLayoutPreference` 和「组字时保留 Shift」的做法相同。iOS 以前没有对应的本机设置，没有旧值要迁移。
struct TouchKeyLayoutStyle: Equatable, Hashable {
  var twentySixKey: TouchTwentySixKeyStyle = .standard
  var fourteenKey: TouchFourteenKeyStyle = .standard
  var nineKey: TouchNineKeyStyle = .standard
  /// 在 26 键、14 键和 18 键字母层上方画一排 1 到 0；手机横屏时不画，存着的值不变。
  var numberRow = false
  /// 手机 26 键底行把 符 排到 123 左边（`TouchBottomRowLayout.symbolFirst`）；长空格和表情键样式没有 符，不受影响。
  var symbolFirst = false
  /// 九宫格数字键盘里空格的位置。
  var numberPadSpace: TouchNumberPadSpace = .platformDefault
  /// 九键字母层左侧符号栏的表（拼音九键、九键双拼和笔画共用），已规范化；空表是 iOS 出厂表 ，。？（`NineKeySidebar`）。
  var nineKeySidebar: [String] = []
  /// 九宫格数字键盘左列的表（九键一族的数字层和各键面借来的数字九宫格共用），已规范化；空表是出厂的 + - * /，#+= 不在表里。
  var digitSidebar: [String] = []
  /// 中/英键在空格哪一边；隐藏了中/英键时不起作用，存着的值不变。
  var languageKeySide: TouchLanguageKeySide = .right

  static let twentySixKeyDocumentKey = "touch_twenty_six_key_style"
  static let fourteenKeyDocumentKey = "touch_fourteen_key_style"
  static let nineKeyDocumentKey = "touch_nine_key_style"
  static let numberRowDocumentKey = "touch_number_row"
  static let symbolFirstDocumentKey = "touch_twenty_six_symbol_first"
  static let numberPadSpaceDocumentKey = "touch_number_pad_space"
  static let nineKeySidebarDocumentKey = "touch_nine_key_sidebar_symbols"
  static let digitSidebarDocumentKey = "touch_nine_key_digit_sidebar_symbols"
  static let languageKeySideDocumentKey = "touch_language_key_side"

  private static let twentySixKeyMirrorKey = "keyboard.style.twentySixKey"
  private static let fourteenKeyMirrorKey = "keyboard.style.fourteenKey"
  private static let nineKeyMirrorKey = "keyboard.style.nineKey"
  private static let numberRowMirrorKey = "keyboard.numberRow"
  private static let symbolFirstMirrorKey = "keyboard.style.symbolFirst"
  private static let numberPadSpaceMirrorKey = "keyboard.style.numberPadSpace"
  private static let nineKeySidebarMirrorKey = "keyboard.nineKeySidebar"
  private static let digitSidebarMirrorKey = "keyboard.nineKeyDigitSidebar"
  private static let languageKeySideMirrorKey = "keyboard.style.languageKeySide"

  init(twentySixKey: TouchTwentySixKeyStyle = .standard, fourteenKey: TouchFourteenKeyStyle = .standard,
       nineKey: TouchNineKeyStyle = .standard, numberRow: Bool = false, symbolFirst: Bool = false,
       numberPadSpace: TouchNumberPadSpace = .platformDefault, nineKeySidebar: [String] = [], digitSidebar: [String] = [],
       languageKeySide: TouchLanguageKeySide = .right) {
    self.twentySixKey = twentySixKey
    self.fourteenKey = fourteenKey
    self.nineKey = nineKey
    self.numberRow = numberRow
    self.symbolFirst = symbolFirst
    self.numberPadSpace = numberPadSpace
    self.nineKeySidebar = nineKeySidebar
    self.digitSidebar = digitSidebar
    self.languageKeySide = languageKeySide
  }

  /// 文档里的各项。缺键、不是字符串或不认识的写法（较新版本加的样式）一律按标准读，与 client-core 的宽松读取一致；数字行和「符」与「123」互换不是布尔值时按关；数字键盘空格位置缺键、写 `platform_default` 或不认识时按 iOS 原来的排法；符号栏的两张表逐项丢掉坏值（`NineKeySidebar.symbols(in:)`），缺键、不是数组或与 iOS 出厂表逐项相同时是出厂表（空表），与 Android `NineKeySidebarPolicy.custom` 相同；中/英键只有恰好写着字符串 `left` 时在空格左边，其余（缺键、`right`、不认识的字符串、不是字符串）都在右边。
  init(in document: [String: Any]?) {
    twentySixKey = (document?[Self.twentySixKeyDocumentKey] as? String).flatMap(TouchTwentySixKeyStyle.init(rawValue:)) ?? .standard
    fourteenKey = (document?[Self.fourteenKeyDocumentKey] as? String).flatMap(TouchFourteenKeyStyle.init(rawValue:)) ?? .standard
    nineKey = (document?[Self.nineKeyDocumentKey] as? String).flatMap(TouchNineKeyStyle.init(rawValue:)) ?? .standard
    numberRow = document?[Self.numberRowDocumentKey] as? Bool ?? false
    symbolFirst = document?[Self.symbolFirstDocumentKey] as? Bool ?? false
    numberPadSpace = (document?[Self.numberPadSpaceDocumentKey] as? String).flatMap(TouchNumberPadSpace.init(rawValue:))
      ?? .platformDefault
    nineKeySidebar = NineKeySidebar.stored(NineKeySidebar.symbols(in: document?[Self.nineKeySidebarDocumentKey]), for: .letter)
    digitSidebar = NineKeySidebar.stored(NineKeySidebar.symbols(in: document?[Self.digitSidebarDocumentKey]), for: .digit)
    languageKeySide = document?[Self.languageKeySideDocumentKey] as? String == TouchLanguageKeySide.left.rawValue ? .left : .right
  }

  private static var defaults: UserDefaults { InputSchemePreference.appGroupDefaults }

  /// App Group 里的镜像，键盘画键时读它，不读文档。
  static var mirrored: TouchKeyLayoutStyle {
    get {
      TouchKeyLayoutStyle(
        twentySixKey: defaults.string(forKey: twentySixKeyMirrorKey).flatMap(TouchTwentySixKeyStyle.init(rawValue:)) ?? .standard,
        fourteenKey: defaults.string(forKey: fourteenKeyMirrorKey).flatMap(TouchFourteenKeyStyle.init(rawValue:)) ?? .standard,
        nineKey: defaults.string(forKey: nineKeyMirrorKey).flatMap(TouchNineKeyStyle.init(rawValue:)) ?? .standard,
        numberRow: defaults.object(forKey: numberRowMirrorKey) as? Bool ?? false,
        symbolFirst: defaults.object(forKey: symbolFirstMirrorKey) as? Bool ?? false,
        numberPadSpace: defaults.string(forKey: numberPadSpaceMirrorKey).flatMap(TouchNumberPadSpace.init(rawValue:))
          ?? .platformDefault,
        nineKeySidebar: NineKeySidebar.stored(NineKeySidebar.symbols(in: defaults.array(forKey: nineKeySidebarMirrorKey)), for: .letter),
        digitSidebar: NineKeySidebar.stored(NineKeySidebar.symbols(in: defaults.array(forKey: digitSidebarMirrorKey)), for: .digit),
        languageKeySide: defaults.string(forKey: languageKeySideMirrorKey).flatMap(TouchLanguageKeySide.init(rawValue:)) ?? .right)
    }
    set {
      defaults.set(newValue.twentySixKey.rawValue, forKey: twentySixKeyMirrorKey)
      defaults.set(newValue.fourteenKey.rawValue, forKey: fourteenKeyMirrorKey)
      defaults.set(newValue.nineKey.rawValue, forKey: nineKeyMirrorKey)
      defaults.set(newValue.numberRow, forKey: numberRowMirrorKey)
      defaults.set(newValue.symbolFirst, forKey: symbolFirstMirrorKey)
      defaults.set(newValue.numberPadSpace.rawValue, forKey: numberPadSpaceMirrorKey)
      defaults.set(newValue.nineKeySidebar, forKey: nineKeySidebarMirrorKey)
      defaults.set(newValue.digitSidebar, forKey: digitSidebarMirrorKey)
      defaults.set(newValue.languageKeySide.rawValue, forKey: languageKeySideMirrorKey)
    }
  }

  /// 把文档里的各项抄进 App Group 镜像；读不到文档时保留镜像。
  static func mirror(_ document: [String: Any]?) {
    guard document != nil else { return }
    let style = TouchKeyLayoutStyle(in: document)
    if style != mirrored { mirrored = style }
  }

  /// 每次只写用户刚改的那一项，其余保持文档里的样子；文档接受了才更新镜像。标准样式和关着的数字行由 client-core 从文档里省掉。
  @discardableResult
  static func save(twentySixKey style: TouchTwentySixKeyStyle, stateRoot: URL? = nil) -> Bool {
    save(key: twentySixKeyDocumentKey, value: style.rawValue, stateRoot: stateRoot) { $0.twentySixKey = style }
  }

  @discardableResult
  static func save(fourteenKey style: TouchFourteenKeyStyle, stateRoot: URL? = nil) -> Bool {
    save(key: fourteenKeyDocumentKey, value: style.rawValue, stateRoot: stateRoot) { $0.fourteenKey = style }
  }

  @discardableResult
  static func save(nineKey style: TouchNineKeyStyle, stateRoot: URL? = nil) -> Bool {
    save(key: nineKeyDocumentKey, value: style.rawValue, stateRoot: stateRoot) { $0.nineKey = style }
  }

  @discardableResult
  static func save(numberRow enabled: Bool, stateRoot: URL? = nil) -> Bool {
    save(key: numberRowDocumentKey, value: enabled, stateRoot: stateRoot) { $0.numberRow = enabled }
  }

  @discardableResult
  static func save(symbolFirst enabled: Bool, stateRoot: URL? = nil) -> Bool {
    save(key: symbolFirstDocumentKey, value: enabled, stateRoot: stateRoot) { $0.symbolFirst = enabled }
  }

  /// 写用户点的那一种排法（包括与 iOS 出厂排法相同的 `after_zero`）：用户明确选过之后，同步到别的设备也按这一种画。
  @discardableResult
  static func save(numberPadSpace space: TouchNumberPadSpace, stateRoot: URL? = nil) -> Bool {
    save(key: numberPadSpaceDocumentKey, value: space.rawValue, stateRoot: stateRoot) { $0.numberPadSpace = space }
  }

  /// 编辑页确认后写一张符号栏表：与 iOS 出厂表逐项相同时写空数组（`NineKeySidebar.stored`），「恢复默认」也是写空数组；只写这一个字段，文档接受了才写镜像。
  @discardableResult
  static func save(sidebar symbols: [String], table: NineKeySidebarTable, stateRoot: URL? = nil) -> Bool {
    let stored = NineKeySidebar.stored(symbols, for: table)
    return save(key: table.documentKey, value: stored, stateRoot: stateRoot) { style in
      switch table {
      case .letter: style.nineKeySidebar = stored
      case .digit: style.digitSidebar = stored
      }
    }
  }

  /// 一张符号栏表此刻存着的值（已规范化，空表是出厂表）。
  func sidebar(_ table: NineKeySidebarTable) -> [String] {
    switch table {
    case .letter: nineKeySidebar
    case .digit: digitSidebar
    }
  }

  /// 一张符号栏表实际画的项：空表画 iOS 出厂表，自定义表原样。
  func sidebarSymbols(_ table: NineKeySidebarTable) -> [String] {
    let symbols = sidebar(table)
    return symbols.isEmpty ? table.defaults : symbols
  }

  /// 九键字母层左列画的项：出厂表在右侧大回车时末尾接 ！（右列的 ！ 让给了回车），自定义表原样使用、不接。
  func letterSidebarSymbols(tallReturn: Bool) -> [String] {
    guard nineKeySidebar.isEmpty else { return nineKeySidebar }
    return tallReturn ? NineKeySidebar.defaultLetterSymbols + [NineKeySidebar.tallReturnClosingMark] : NineKeySidebar.defaultLetterSymbols
  }

  /// 打开写 `left`、关上写 `right`，与 Android、鸿蒙相同；`right` 是默认值，client-core 保存时从文档里省掉。
  @discardableResult
  static func save(languageKeySide side: TouchLanguageKeySide, stateRoot: URL? = nil) -> Bool {
    save(key: languageKeySideDocumentKey, value: side.rawValue, stateRoot: stateRoot) { $0.languageKeySide = side }
  }

  private static func save(key: String, value: Any, stateRoot: URL?, apply: (inout TouchKeyLayoutStyle) -> Void) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { $0[key] = value }) else { return false }
    var style = mirrored
    apply(&style)
    mirrored = style
    return true
  }
}

/// 设置页「按键布局」的分段列出哪几类键盘，按版本提供的入口判断，与 Android、鸿蒙同一规则：没有的键面类型不列进分段，既没有中文 26 键也没有 14/18 键时不列「顶部数字行」开关。`offers` 在设置页是本版本（`edition`），测试换成别的版本。各样式的说明和预览里高亮的键也在这里，三端逐字、逐键相同。
struct TouchKeyLayoutOffering: Equatable {
  /// 读 26 键样式的中文 26 键入口：全拼、各种双拼、五笔、粤拼。日文罗马字、韩文、越南文和藏文的 26 键画标准底行（键盘的 `drawsChineseTwentySixKey`），不算。
  static let twentySixKeySchemes: [ChineseInputScheme] = [
    .quanpin, .shuangpin, .ziranma, .microsoft, .shoudao, .wubi, .cantonese, .cantoneseShuangpin,
  ]
  /// 有 14 键或 18 键的入口：全拼的两个入口，以及双拼和五笔可选的字母键布局。
  static let letterGridSchemes: [ChineseInputScheme] = [.fourteenKey, .eighteenKey, .shuangpin, .wubi]
  /// 九键框架：拼音九键、九键双拼和笔画。
  static let nineKeySchemes: [ChineseInputScheme] = [.nineKey, .nineKeyShuangpin, .stroke]

  let twentySixKey: Bool
  let letterGrid: Bool
  let nineKey: Bool
  /// 顶部数字行只画在中文 26 键和 14/18 键上时才有意义，两样都没有就不列开关。
  var numberRow: Bool { twentySixKey || letterGrid }
  /// 至少有一组可列：键盘页的「按键布局」入口只在这时显示，一组都没有的版本不留一个点进去是空页的入口。
  var offersAnyGroup: Bool { twentySixKey || letterGrid || nineKey }

  /// 键盘页「按键布局」入口的副标题。这一行和下面几条说明三端逐字相同（Android、鸿蒙的设置页用同样的文字）。
  static let entrySubtitle = "各类键盘的底行样式、键位和顶部数字行"
  /// 26 键「长空格」的说明。
  static let longSpaceNote = "符号键移到工具栏。"
  /// 26 键和 14/18 键「表情键」的说明。
  static let emojiNote = "长按 123 打开符号。"
  /// 9 键「右侧大回车」的说明。
  static let tallReturnNote = "回车竖占两行，符号键在左下角。"
  /// 「顶部数字行」开关的说明。
  static let numberRowSubtitle = "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。"
  /// 26 键一组里「符」与「123」互换开关的标题和说明。
  static let symbolFirstTitle = "26 键「符」与「123」互换"
  static let symbolFirstSubtitle = "符 排在 123 左边；长空格和表情键样式没有 符，不受影响。"
  /// 每个分段的样式卡片里「中/英键放在空格左边」开关的标题和说明，三个分段是同一个偏好。
  static let languageLeftTitle = "中/英键放在空格左边"
  static let languageLeftSubtitle = "中/英 排到空格左边，逗号、句号仍紧挨空格。"
  /// 「数字键盘」一组的标题和组末说明；有入口就列这一组，九宫格数字键盘从各类中文键面都进得去。
  static let numberPadTitle = "数字键盘"
  static let numberPadFooter = "九宫格数字键盘里空格的位置；右侧大回车不受影响。"

  /// 页面顶部分段控件列出的键盘类型，按 26 键、14/18 键、9 键的顺序，只列本版本提供的；只有一种时页面不画分段控件。
  var kinds: [TouchKeyLayoutKind] {
    let listed: [TouchKeyLayoutKind?] = [twentySixKey ? .twentySixKey : nil, letterGrid ? .letterGrid : nil, nineKey ? .nineKey : nil]
    return listed.compactMap { $0 }
  }

  /// 页面打开时停在哪一类：用户当前中文键盘所属的那一类（`TouchKeyLayoutKind.of`）；判断不了或本版本不列那一类时停在第一个。一类都不列时为 nil。
  func initialKind(current: TouchKeyLayoutKind?) -> TouchKeyLayoutKind? {
    if let current, kinds.contains(current) { return current }
    return kinds.first
  }

  /// 选中样式的副作用，显示在样式按钮下面一行；没有副作用的样式为 nil，页面不画这一行。
  static func note(_ style: TouchTwentySixKeyStyle) -> String? {
    switch style {
    case .longSpace: longSpaceNote
    case .emoji: emojiNote
    case .standard, .commaPeriod: nil
    }
  }

  static func note(_ style: TouchFourteenKeyStyle) -> String? {
    style == .emoji ? emojiNote : nil
  }

  static func note(_ style: TouchNineKeyStyle) -> String? {
    style == .tallReturn ? tallReturnNote : nil
  }

  /// 预览里这个样式与「标准」不同、用强调色描边的键，按底行表的键位（`TouchBottomRowLayout.Slot`）认，不比较键面文字；标准样式不高亮。
  static func highlightedSlots(_ style: TouchTwentySixKeyStyle) -> Set<TouchBottomRowLayout.Slot> {
    switch style {
    case .standard: []
    case .commaPeriod: [.comma, .period]
    case .longSpace: [.space]
    case .emoji: [.emoji]
    }
  }

  static func highlightedSlots(_ style: TouchFourteenKeyStyle) -> Set<TouchBottomRowLayout.Slot> {
    switch style {
    case .standard: []
    case .longSpace: [.space]
    case .emoji: [.emoji]
    }
  }

  static func highlightedSlots(_ style: TouchNineKeyStyle) -> Set<TouchBottomRowLayout.Slot> {
    switch style {
    case .standard: []
    case .commaPeriod: [.comma, .period]
    case .emoji: [.emoji]
    case .tallReturn: [.returnKey]
    }
  }

  init(offers: (ChineseInputScheme) -> Bool) {
    twentySixKey = Self.twentySixKeySchemes.contains(where: offers)
    letterGrid = Self.letterGridSchemes.contains(where: offers)
    nineKey = Self.nineKeySchemes.contains(where: offers)
  }

  static var edition: TouchKeyLayoutOffering { TouchKeyLayoutOffering { $0.isOfferedByEdition } }
}

/// 「按键布局」页顶部分段控件里的一类键盘：26 键、14 键与 18 键、9 键，各自对应一项底行样式偏好。切换分段只换预览和样式按钮，不写任何偏好；三端同一套，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
enum TouchKeyLayoutKind: String, CaseIterable, Hashable {
  case twentySixKey = "26"
  case letterGrid = "14"
  case nineKey = "9"

  var title: String {
    switch self {
    case .twentySixKey: "26 键"
    case .letterGrid: "14/18 键"
    case .nineKey: "9 键"
    }
  }

  /// 分段按钮的 accessibility identifier：`keyLayoutKind_26`、`keyLayoutKind_14`、`keyLayoutKind_9`。
  var identifier: String { "keyLayoutKind_\(rawValue)" }

  /// 用户当前的中文键盘属于哪一类：全拼、双拼、五笔、粤拼的 26 键是 26 键；全拼 14 键、18 键，以及双拼、五笔选了 14 键或 18 键布局（`LetterKeyGrid`）是 14/18 键；拼音九键、九键双拼和笔画是 9 键。手写、注音和非中文的 26 键不读这几项样式，为 nil。
  static func of(_ scheme: ChineseInputScheme, shuangpinLayout: TouchShuangpinLayoutPreference.Layout,
                 wubiLayout: TouchWubiLayoutPreference.Layout) -> TouchKeyLayoutKind? {
    if LetterKeyGrid.of(scheme, shuangpinLayout: shuangpinLayout, wubiLayout: wubiLayout) != nil { return .letterGrid }
    if TouchKeyLayoutOffering.nineKeySchemes.contains(scheme) { return .nineKey }
    if TouchKeyLayoutOffering.twentySixKeySchemes.contains(scheme) { return .twentySixKey }
    return nil
  }

  /// 预览顶上画不画数字行：数字行只加在 26 键、14 键和 18 键上，9 键的预览不画，开关照样列出。
  func drawsNumberRow(_ style: TouchKeyLayoutStyle) -> Bool {
    style.numberRow && self != .nineKey
  }

  /// 预览里这一类在 `style` 下的底行，取自键盘用的同一张表；不画地球键，`language` 为 false 时（用户隐藏了中/英键）它的份额并进空格，与键盘一致。26 键打开「符」与「123」互换时照键盘把 符 排到 123 左边（`TouchBottomRowLayout.symbolFirst`），没有 符 的样式不变；中/英键放在空格左边时最后再照键盘重排一次（`TouchBottomRowLayout.languageBeforeSpace`）。
  func bottomRow(_ style: TouchKeyLayoutStyle, language: Bool = true) -> [TouchBottomRowLayout.Key] {
    let keys: [TouchBottomRowLayout.Key]
    switch self {
    case .twentySixKey:
      let row = TouchBottomRowLayout.twentySixKey(style.twentySixKey, globe: false, language: language)
      keys = style.symbolFirst ? TouchBottomRowLayout.symbolFirst(row) : row
    case .letterGrid: keys = TouchBottomRowLayout.letterGrid(style.fourteenKey, globe: false, language: language)
    case .nineKey: keys = TouchBottomRowLayout.nineKey(style.nineKey, globe: false, language: language)
    }
    return style.languageKeySide == .left ? TouchBottomRowLayout.languageBeforeSpace(keys) : keys
  }

  /// 这一类当前选中样式的说明（`TouchKeyLayoutOffering.note`）。
  func note(_ style: TouchKeyLayoutStyle) -> String? {
    switch self {
    case .twentySixKey: TouchKeyLayoutOffering.note(style.twentySixKey)
    case .letterGrid: TouchKeyLayoutOffering.note(style.fourteenKey)
    case .nineKey: TouchKeyLayoutOffering.note(style.nineKey)
    }
  }

  /// 这一类当前选中样式在预览里高亮的键位（`TouchKeyLayoutOffering.highlightedSlots`）。26 键打开「符」与「123」互换、且这个样式有 符 时，再高亮 符 和 123 两个键；中/英键放在空格左边时再高亮中/英（预览底行里没有中/英时，比如隐藏了中/英键，高亮集合里有它也画不出来）。被挤动的空格和标点宽度、样子都没变，不高亮。
  func highlightedSlots(_ style: TouchKeyLayoutStyle) -> Set<TouchBottomRowLayout.Slot> {
    let slots: Set<TouchBottomRowLayout.Slot>
    switch self {
    case .twentySixKey:
      let swapped = style.symbolFirst
        && TouchBottomRowLayout.twentySixKey(style.twentySixKey, globe: false).contains { $0.slot == .symbols }
      slots = TouchKeyLayoutOffering.highlightedSlots(style.twentySixKey).union(swapped ? [.symbols, .layer] : [])
    case .letterGrid: slots = TouchKeyLayoutOffering.highlightedSlots(style.fourteenKey)
    case .nineKey: slots = TouchKeyLayoutOffering.highlightedSlots(style.nineKey)
    }
    return style.languageKeySide == .left ? slots.union([.language]) : slots
  }

  /// 9 键预览的左侧符号栏要不要整列描强调色：字母层用的是自定义表（与出厂不同）时描，空表（出厂表）时不描；别的分段没有这一栏。
  func highlightsSidebar(_ style: TouchKeyLayoutStyle) -> Bool {
    self == .nineKey && !style.nineKeySidebar.isEmpty
  }

  /// 9 键预览左侧符号栏画的项，与键盘一屏看到的相同：出厂表照键盘画（右侧大回车时末尾接 ！），自定义表画一屏最多三项（`NineKeySidebar.letterVisibleRows`），等高均分。
  static func previewSidebar(_ style: TouchKeyLayoutStyle) -> [String] {
    let symbols = style.letterSidebarSymbols(tallReturn: style.nineKey == .tallReturn)
    return style.nineKeySidebar.isEmpty ? symbols : Array(symbols.prefix(NineKeySidebar.letterVisibleRows))
  }
}

/// 整块键盘预览（`KeyboardSkinPreview`：键盘页的实时预览，以及皮肤、工具栏、AI 皮肤和自定义皮肤编辑器各页的键盘缩略图）字母层的框架：画哪一类键面、顶上画不画 1 到 0、底行的键和份额，九键时左侧符号栏和右列各画什么。样式取用户在「按键布局」里选的（`TouchKeyLayoutStyle`，含「符」与「123」互换、中/英键放在空格左边和九键左侧符号），中/英键跟「隐藏中/英键」设置，键序和份额与键盘、「按键布局」页取自同一张表（`TouchKeyLayoutKind.bottomRow`、`TouchBottomRowLayout`、`TypingKeyHeatmap`），不另写一套。预览不做高亮，高亮只在「按键布局」页。
struct KeyboardPreviewLetterLayer: Equatable {
  let kind: TouchKeyLayoutKind
  /// 字母行上方画不画一排 1 到 0（`TouchKeyLayoutKind.drawsNumberRow`）：只在 26 键和 14/18 键上画。
  let numberRow: Bool
  /// 字母层底行，从左到右；不画地球键。「右侧大回车」时第一格 符 画在符号栏下方、最后一格回车画在右列并竖跨两行，中间几格画在九宫格下方。
  let bottomRow: [TouchBottomRowLayout.Key]
  /// 九键「右侧大回车」：回车从右列第三格伸到底行。
  let tallReturn: Bool
  /// 九键左侧符号栏自上而下的标点，与「按键布局」页的 9 键预览相同（`TouchKeyLayoutKind.previewSidebar`）：出厂表时右侧大回车把右列的 ！ 接到末尾；用户换了字母层的表时画那张表的一屏。
  let nineKeySidebar: [String]
  /// 九键右列在九宫格旁的几格（⌫、分词、！），右侧大回车时只剩 ⌫ 和分词。
  let nineKeyControls: [String]

  /// `nineKey` 与 `letterGrid` 是预览画的键面：九键优先，其次 14/18 键，其余是 26 键。`language` 为 false 时（用户隐藏了中/英键）它的份额并进空格。`nineKeyZero` 是九键双拼字母层空格后面的 0。
  init(nineKey: Bool, letterGrid: Bool, style: TouchKeyLayoutStyle, language: Bool, nineKeyZero: Bool = false) {
    kind = nineKey ? .nineKey : letterGrid ? .letterGrid : .twentySixKey
    numberRow = kind.drawsNumberRow(style)
    if kind == .nineKey {
      // 九键双拼字母层的 0 只有键盘控制器知道，这里自己排，再照 `bottomRow` 的最后一步把中/英挪到空格左边。
      let row = TouchBottomRowLayout.nineKey(style.nineKey, globe: false, zero: nineKeyZero, language: language)
      bottomRow = style.languageKeySide == .left ? TouchBottomRowLayout.languageBeforeSpace(row) : row
    } else {
      bottomRow = kind.bottomRow(style, language: language)
    }
    tallReturn = kind == .nineKey && style.nineKey == .tallReturn
    nineKeySidebar = TouchKeyLayoutKind.previewSidebar(style)
    nineKeyControls = TypingKeyHeatmap.nineKeyControls(for: style.nineKey).map { $0.id == TypingKeyID.backspace ? "⌫" : $0.face }
  }
}

/// 整块键盘预览（「键盘」页的实时预览、皮肤页的缩略图）画哪一种键面：用户当前的中文键盘属于哪一类（`TouchKeyLayoutKind.of`），与 Android `KeyLayoutStylesPolicy.previewKind`、共享页相同。九键一族（拼音九键、九键双拼、笔画）都画九宫格外框，笔画与键盘一样沿用九键外框和九键的底行样式（含右侧大回车）；14/18 键画对应的字母格；其余方案，包括不读「按键布局」样式的手写、注音、日文和韩文等，画 26 键。见 .agents/notes/implemented/bug-fix/2026-10-29-keyboard-previews-follow-layout.md。
struct KeyboardPreviewFace: Equatable {
  /// 当前方案读不读「按键布局」样式（`TouchKeyLayoutKind.of` 不为 nil）；不读时「键盘」页的预览画标准底行。
  let styled: Bool
  /// 画九宫格外框。
  let nineKey: Bool
  /// 14 键或 18 键的字母格；九键和 26 键为 nil。
  let letterGrid: LetterKeyGrid?
  /// 字母格第三排左端画「分词」：五笔 14 键没有音节可分，画「符」。
  let gridSplitsSyllables: Bool
  /// 九键双拼字母层空格后面的 0。
  let nineKeyZero: Bool

  init(_ scheme: ChineseInputScheme, shuangpinLayout: TouchShuangpinLayoutPreference.Layout,
       wubiLayout: TouchWubiLayoutPreference.Layout) {
    let kind = TouchKeyLayoutKind.of(scheme, shuangpinLayout: shuangpinLayout, wubiLayout: wubiLayout)
    styled = kind != nil
    nineKey = kind == .nineKey
    letterGrid = LetterKeyGrid.of(scheme, shuangpinLayout: shuangpinLayout, wubiLayout: wubiLayout)
    gridSplitsSyllables = scheme != .wubi
    nineKeyZero = scheme == .nineKeyShuangpin
  }

  /// 共享文档里用户正在用的方案和双拼、五笔布局；文档里没记的方案退回 App Group 镜像，与「键盘」页读的相同。
  init(in document: [String: Any]?) {
    self.init(InputSchemePreference.current(in: document).scheme,
              shuangpinLayout: TouchShuangpinLayoutPreference.layout(in: document),
              wubiLayout: TouchWubiLayoutPreference.layout(in: document))
  }

  /// App Group 镜像里的方案和布局，共享文档读回来之前先按它画。
  static var mirrored: KeyboardPreviewFace {
    KeyboardPreviewFace(InputSchemePreference.scheme, shuangpinLayout: TouchShuangpinLayoutPreference.layout,
                        wubiLayout: TouchWubiLayoutPreference.layout)
  }
}

/// 手机触屏底行按「按键布局」样式的排法：每个显示的键、从左到右的顺序和它在本行里的宽度份额。三端相同，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
///
/// 标准样式就是原来的底行（`KeyboardViewController` 的 `phoneActionWidths`、`nineKeyActionWidths` 等），表里照抄一份，供统计热力图、设置页预览和测试使用；其余样式由键盘控制器按这张表排列并加宽度约束。样式加的键都从空格里让出份额，整行总份额不变。地球键只在宿主要求时出现，占 1 份，从行宽里另外分；隐藏中/英键时它的 1.05 份交给空格。
enum TouchBottomRowLayout {
  enum Slot: String, Hashable {
    case symbols, layer, globe, emoji, comma, space, period, zero, language, returnKey
  }

  struct Key: Hashable {
    let slot: Slot
    let weight: CGFloat

    init(_ slot: Slot, _ weight: CGFloat) {
      self.slot = slot
      self.weight = weight
    }
  }

  static let layerWeight: CGFloat = 1.25
  static let symbolsWeight: CGFloat = 1.05
  static let languageWeight: CGFloat = 1.05
  static let globeWeight: CGFloat = 1
  static let punctuationWeight: CGFloat = 1
  /// 26 键、14 键和 18 键底行的空格与回车。
  static let spaceWeight: CGFloat = 4
  static let returnWeight: CGFloat = 1.9
  /// 九键底行的空格与回车；九键双拼字母层的 0 与 符 同宽。
  static let nineKeySpaceWeight: CGFloat = 4.2
  static let nineKeyReturnWeight: CGFloat = 1.6
  static let zeroWeight: CGFloat = 1.05

  /// 中文 26 键字母层。标准：123 | 符 | 空格 | ， | 中/英 | 回车；逗号句号在空格两侧：。从空格让出 1 份；长空格：符 的 1.05 份给空格；表情键：😀 占 符 的位置和份额。
  static func twentySixKey(_ style: TouchTwentySixKeyStyle, globe: Bool, language: Bool = true) -> [Key] {
    var keys = [Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    switch style {
    case .standard:
      keys += [Key(.symbols, symbolsWeight), Key(.space, spaceWeight), Key(.comma, punctuationWeight)]
    case .commaPeriod:
      keys += [Key(.symbols, symbolsWeight), Key(.comma, punctuationWeight),
               Key(.space, spaceWeight - punctuationWeight), Key(.period, punctuationWeight)]
    case .longSpace:
      keys += [Key(.comma, punctuationWeight), Key(.space, spaceWeight + symbolsWeight)]
    case .emoji:
      keys += [Key(.emoji, symbolsWeight), Key(.space, spaceWeight), Key(.comma, punctuationWeight)]
    }
    return finish(keys, language: language, returnWeight: returnWeight)
  }

  /// 14 键与 18 键字母层。标准：123 | ， | 空格 | 。 | 中/英 | 回车；长空格：。的份额给空格；表情键：123 | 😀 | 空格 | ， | 中/英 | 回车，😀 用原来 ， 的份额，， 用原来 。 的份额。
  static func letterGrid(_ style: TouchFourteenKeyStyle, globe: Bool, language: Bool = true) -> [Key] {
    var keys = [Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    switch style {
    case .standard:
      keys += [Key(.comma, punctuationWeight), Key(.space, spaceWeight), Key(.period, punctuationWeight)]
    case .longSpace:
      keys += [Key(.comma, punctuationWeight), Key(.space, spaceWeight + punctuationWeight)]
    case .emoji:
      keys += [Key(.emoji, punctuationWeight), Key(.space, spaceWeight), Key(.comma, punctuationWeight)]
    }
    return finish(keys, language: language, returnWeight: returnWeight)
  }

  /// 九键字母层（拼音九键、九键双拼、笔画）。标准：符 | 123 | 空格 | 中/英 | 回车，九键双拼在空格后多一个 0；底栏带标点：， 和 。 各从空格让出 1 份；表情键：😀 从空格让出 1.05 份；右侧大回车见 `nineKeyTallReturn`。除右侧大回车外数字层不用这张表。
  static func nineKey(_ style: TouchNineKeyStyle, globe: Bool, zero: Bool = false, language: Bool = true) -> [Key] {
    if style == .tallReturn { return nineKeyTallReturn(globe: globe, zero: zero, language: language) }
    var keys = [Key(.symbols, symbolsWeight), Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    switch style {
    case .standard:
      keys.append(Key(.space, nineKeySpaceWeight))
    case .commaPeriod:
      keys += [Key(.comma, punctuationWeight), Key(.space, nineKeySpaceWeight - 2 * punctuationWeight),
               Key(.period, punctuationWeight)]
    case .emoji:
      keys += [Key(.emoji, symbolsWeight), Key(.space, nineKeySpaceWeight - symbolsWeight)]
    case .tallReturn:
      break
    }
    if zero { keys.append(Key(.zero, zeroWeight)) }
    return finish(keys, language: language, returnWeight: nineKeyReturnWeight)
  }

  /// 九键标准底行不带地球键时的总份额（符 1.05 + 123 1.25 + 空格 4.2 + 中/英 1.05 + 回车 1.6），即整个九键外框的宽度。
  static let nineKeyFrameWeight: CGFloat = symbolsWeight + layerWeight + nineKeySpaceWeight + languageWeight + nineKeyReturnWeight
  /// 九键外框左侧符号栏和右列各占的宽度：整宽的 14%（`KeyboardGeometry.sidebarRatio`）。
  static let nineKeySideRatio: CGFloat = 0.14

  /// 同一地球键状态下九键标准底行的总份额：宿主要求地球键时它在 123 后另占 1 份，整行按 10.15 份分，每一份因此比不带地球键时窄。
  static func nineKeyRowWeight(globe: Bool) -> CGFloat {
    nineKeyFrameWeight + (globe ? globeWeight : 0)
  }

  /// 同一地球键和中/英状态下拼音九键标准底行（符 | 123 |（🌐）| 空格 |（中/英）| 回车）显示几个键，不算九键双拼另加的 0。键盘画这一行时先从行宽里扣掉键数减一道键距，剩下的才按份额分；右侧大回车的中列要与它绝对等宽，就得扣同样多的键距（`KeyboardViewController.tallReturnActionWidths`）。
  static func nineKeyStandardKeyCount(globe: Bool, language: Bool) -> Int {
    nineKey(.standard, globe: globe, language: language).count
  }

  /// 一个侧列折成同一地球键状态下标准底行的份额：`nineKeyRowWeight(globe:)` × 14%。
  static func nineKeySideWeight(globe: Bool) -> CGFloat {
    nineKeyRowWeight(globe: globe) * nineKeySideRatio
  }

  /// 九键「右侧大回车」的底行，整行与九键外框同宽、总份额与同一地球键状态下的标准底行相同（带地球键时是 10.15 份）。第一格 符 在符号栏正下方、与符号栏同宽，最后一格回车与右列同宽（它在键盘上从右列第三行一直伸到底行，两倍高）；中间几格合起来正好是九宫格的宽度：123、地球键、中/英 与同一地球键状态下的标准九键底行同宽，空格占剩下的部分，地球键也从空格里让出。这里的份额不含键距；键盘上的绝对宽度按标准底行的键距扣法算（`nineKeyStandardKeyCount`），于是与标准底行的键逐点等宽。九键双拼字母层的 0 照旧跟在空格后面。
  ///
  /// 数字层（`digits`）照同一框架：符 不动，123 的位置是回到字母层的切换键，空格的位置放 0、占剩下的部分；空格按 iOS 数字层原来的规则留在 0 的右边，与 中/英 同宽。
  static func nineKeyTallReturn(globe: Bool, zero: Bool = false, language: Bool = true, digits: Bool = false) -> [Key] {
    var middle = [Key(.layer, layerWeight)]
    if globe { middle.append(Key(.globe, globeWeight)) }
    let fill: Slot = digits ? .zero : .space
    middle.append(Key(fill, 0))
    if digits {
      middle.append(Key(.space, zeroWeight))
    } else if zero {
      middle.append(Key(.zero, zeroWeight))
    }
    if language { middle.append(Key(.language, languageWeight)) }
    let side = nineKeySideWeight(globe: globe)
    let gridWeight = nineKeyRowWeight(globe: globe) - 2 * side
    let rest = gridWeight - middle.reduce(0) { $0 + $1.weight }
    middle = middle.map { $0.slot == fill ? Key(fill, rest) : $0 }
    return [Key(.symbols, side)] + middle + [Key(.returnKey, side)]
  }

  /// 拼音九键、九键双拼和笔画的数字层（不是右侧大回车时）：符 | 九键 |（🌐）| 0 | 空格 | 中/英 | 回车。与 Android #9113 的规则相同，往返字母层和数字层时 符 和切换键不换位置、宽度也不变：符 和切换键沿用字母层的份额，0 和它右边的空格合起来占字母层空格（九键双拼还有它的 0）那一份，整行总份额与字母层相同。空格与 中/英 同宽，跟数字层原来的规则一样在 0 的右边；隐藏中/英时它的份额并进 0。从 26 键、14 键、18 键、手写借来的数字九宫格不用这张表，返回键本来就在它们 123 那一格。
  ///
  /// `space`（`touch_number_pad_space`）决定 0 和空格的排法：`afterZero`（iOS 原来的排法）如上；`beforeZero` 两键对调，空格占大份、0 与中/英同宽；`rightColumn` 底行没有空格，0 占两键合起来的份额，空格在右列最下面一格（键盘的 `numberPadColumnSpace`）。
  static func nineKeyDigits(globe: Bool, zero: Bool = false, language: Bool = true,
                            space: TouchNumberPadSpace = .afterZero) -> [Key] {
    var keys = [Key(.symbols, symbolsWeight), Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    let wideShare = nineKeySpaceWeight + (zero ? zeroWeight : 0) - zeroWeight + (language ? 0 : languageWeight)
    switch space {
    case .afterZero: keys += [Key(.zero, wideShare), Key(.space, zeroWeight)]
    case .beforeZero: keys += [Key(.space, wideShare), Key(.zero, zeroWeight)]
    case .rightColumn: keys.append(Key(.zero, wideShare + zeroWeight))
    }
    if language { keys.append(Key(.language, languageWeight)) }
    keys.append(Key(.returnKey, nineKeyReturnWeight))
    return keys
  }

  /// 从 26 键、14 键、18 键、手写借来的数字九宫格（返回 |（🌐）| 0 | 空格 | 中/英 | 回车）在空格位置不是 iOS 原来的 `afterZero` 时的份额：返回 1.25、地球键 1、中/英 1.05、回车 1.6 与原来的数字页相同，0 和空格合起来的 5.25 份（隐藏中/英时再加 1.05）按 `space` 分：`beforeZero` 空格在左占大份、0 是 1.05，`rightColumn` 底行只有 0。`afterZero` 仍走原来的约束（不带地球键时与上方五列对齐），不用这张表。
  static func borrowedNumberPad(globe: Bool, language: Bool = true, space: TouchNumberPadSpace) -> [Key] {
    var keys = [Key(.layer, layerWeight)]
    if globe { keys.append(Key(.globe, globeWeight)) }
    let wideShare = nineKeySpaceWeight + (language ? 0 : languageWeight)
    switch space {
    case .afterZero: keys += [Key(.zero, wideShare), Key(.space, zeroWeight)]
    case .beforeZero: keys += [Key(.space, wideShare), Key(.zero, zeroWeight)]
    case .rightColumn: keys.append(Key(.zero, wideShare + zeroWeight))
    }
    if language { keys.append(Key(.language, languageWeight)) }
    keys.append(Key(.returnKey, nineKeyReturnWeight))
    return keys
  }

  /// 「26 键「符」与「123」互换」（`touch_twenty_six_symbol_first`）：排在 123 后面的 符 挪到 123 左边，其余键次序和份额不变，与 Android `KeyboardActionRow.designEntries`、鸿蒙 `TouchLayoutStylePolicy.symbolFirst` 相同；没有 符（长空格、表情键）或 符 已经在前面时原样返回。
  static func symbolFirst(_ keys: [Key]) -> [Key] {
    guard let layer = keys.firstIndex(where: { $0.slot == .layer }),
          let symbols = keys.firstIndex(where: { $0.slot == .symbols }), symbols > layer else { return keys }
    var moved = keys
    let panel = moved.remove(at: symbols)
    moved.insert(panel, at: layer)
    return moved
  }

  /// 「中/英键放在空格左边」（`touch_language_key_side` 为 `left`）：中/英左边紧挨着的、由空格、0、逗号、句号组成的一串连续的键整串右移一格，中/英挪到这一串的最左边；所有键的份额不变，只换次序。样式、「符」与「123」互换和数字键盘空格位置都排完之后最后做这一步。空格左边没有标点时中/英紧挨空格（与豆包、百度九键「系统布局」相同）；有标点（逗号句号在空格两侧、长空格、14/18 键、英文的句点、手写）时标点仍紧挨空格，中/英排在标点左边，于是中文和英文 26 键上的中/英停在同一格。行里没有中/英（隐藏了中/英键）或它左边紧挨的不是这四种键时原样返回。与 Android `KeyboardActionRow.languageBeforeSpace`、鸿蒙 `TouchLayoutStylePolicy.languageBeforeSpace` 是同一条规则，见 .agents/notes/implemented/feature/2026-10-29-language-key-left-of-space.md。
  static func languageBeforeSpace(_ keys: [Key]) -> [Key] {
    languageBeforeSpace(keys) { $0.slot }
  }

  /// 中/英往左越过的那几种键。
  static let languageRunSlots: Set<Slot> = [.space, .zero, .comma, .period]

  /// 同一条规则按任意元素排：`slot` 说出每个元素是哪一格，说不出的（nil）当作别的键，挡住这一串。键盘控制器拿它排底行里的按键视图，分离用的中缝和第二个空格算作空格。
  static func languageBeforeSpace<Element>(_ items: [Element], slot: (Element) -> Slot?) -> [Element] {
    guard let language = items.firstIndex(where: { slot($0) == .language }) else { return items }
    var start = language
    while start > 0, let previous = slot(items[start - 1]), languageRunSlots.contains(previous) { start -= 1 }
    guard start < language else { return items }
    var moved = items
    let key = moved.remove(at: language)
    moved.insert(key, at: start)
    return moved
  }

  /// 补上中/英和回车；隐藏中/英时把它的份额并进空格。
  private static func finish(_ keys: [Key], language: Bool, returnWeight: CGFloat) -> [Key] {
    var keys = keys
    if language {
      keys.append(Key(.language, languageWeight))
    } else if let index = keys.firstIndex(where: { $0.slot == .space }) {
      keys[index] = Key(.space, keys[index].weight + languageWeight)
    }
    keys.append(Key(.returnKey, returnWeight))
    return keys
  }
}

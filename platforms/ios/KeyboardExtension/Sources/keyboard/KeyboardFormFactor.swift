import UIKit

/// The physical keyboard the extension is drawing: a phone keyboard or a full-width tablet one.
///
/// It is decided from the traits the host hands the extension rather than from the device model, because an iPad does not always get the tablet keyboard: the floating keyboard and narrow Slide Over / Stage Manager windows report a compact width, and the system keyboard draws its phone layout there too. Following the same rule keeps this keyboard the same size as the one it replaces.
enum KeyboardFormFactor: Equatable {
  case phone
  case tablet

  static func resolve(
    idiom: UIUserInterfaceIdiom, horizontalSizeClass: UIUserInterfaceSizeClass
  ) -> KeyboardFormFactor {
    idiom == .pad && horizontalSizeClass != .compact ? .tablet : .phone
  }

  static func resolve(_ traits: UITraitCollection) -> KeyboardFormFactor {
    resolve(idiom: traits.userInterfaceIdiom, horizontalSizeClass: traits.horizontalSizeClass)
  }

  /// 键盘在顶部那一行和按键四周的内边距（dc.html `kbPad`）：手机上方 6、两侧 3、下方 4，iPad 为 6 / 8 / 4。
  var padding: NSDirectionalEdgeInsets {
    switch self {
    case .phone: NSDirectionalEdgeInsets(top: 6, leading: 3, bottom: 4, trailing: 3)
    case .tablet: NSDirectionalEdgeInsets(top: 6, leading: 8, bottom: 4, trailing: 8)
    }
  }

  /// 顶部那一行与第一排按键之间的间隔（dc.html `kbGap`）：手机 11，iPad 9。
  ///
  /// 设计稿的各排按键之间也用同样的间隔，这里则与 Android 一样保留用户的排间距（`touch_row_spacing_tenths`，取值 4-10，默认 7）：共享的取值范围放不下 11，而且用户调过的间距应当保持不变。
  var topRowGap: CGFloat { self == .tablet ? 9 : 11 }

  /// 默认键盘高度下一排按键的高度。
  ///
  /// 竖屏用设计稿的按键高度（`KeyboardHeightPercent.portraitKeyHeight`）：手机 42pt，iPad 54pt。设计稿没有画横屏键盘，所以横屏沿用以前按系统尺寸做的横屏键盘的按键高度：手机 34pt，其中手写为 40pt，让书写区仍然够用；iPad 为更高的 73pt，和系统键盘一样随更宽的屏幕变大。
  func keyHeight(landscape: Bool, handwriting: Bool) -> CGFloat {
    guard landscape else { return KeyboardHeightPercent.portraitKeyHeight(tablet: self == .tablet) }
    switch self {
    case .phone: return handwriting ? 40 : 34
    case .tablet: return 73
    }
  }

  /// 键盘按几排按键来排布：四排；iPad 显示数字行时为五排，因为那是整整多出的一排，而不是从字母行里挤出来的。
  func keyRows(numberRow: Bool) -> Int { self == .tablet && numberRow ? 5 : 4 }

  /// 用户调整之前的键盘高度，按设计稿的结构逐项相加：上边距、顶部那一行、它下面的间隔、各排按键连同排间距，以及下边距。顶部那一行收起（`topRow` 为 0，「显示方式」为隐藏时）时间隔也一起去掉，与 `applyKeyboardMetrics` 把那段间隔设为 0 一致，否则多出的高度会被分给按键，组字开始和结束时按键高度跟着跳。
  func keyboardHeight(topRow: CGFloat, rowSpacing: CGFloat, landscape: Bool, handwriting: Bool,
                      numberRow: Bool = false) -> CGFloat {
    padding.top + topRow + (topRow > 0 ? topRowGap : 0)
      + KeyboardHeightPercent.keyBlockHeight(
        keyHeight: keyHeight(landscape: landscape, handwriting: handwriting), rows: keyRows(numberRow: numberRow),
        rowSpacing: rowSpacing)
      + padding.bottom
  }

  /// Whether the third letter row carries the comma and full-stop keys, as the iPad system keyboard does.
  var showsLetterRowPunctuation: Bool { self == .tablet }

  /// Whether the keyboard can carry the digit row and Tab key of a full-size keyboard (`KeyboardLayoutPreference.tabletFullKeys`). A phone-width keyboard has no room for either.
  var canShowFullKeys: Bool { self == .tablet }
}

/// iPad 键盘的 26 键布局（dc.html 里的 `pad` 各行），跟随 iPadOS 系统键盘而不是手机的布局：⌫ 在第一行末尾，第二行缩进 2.5% 开始、以 return 结尾，第三行在 z–m 和 ，。 两端各有一个 ⇧，底行为 123 | 中 | space | 123 | ⌄，没有单独的 return。权重是相对一个字母键宽度的份额，底行则与设计稿的 flex 一样是相对本行的份额。
///
/// 只用于全宽平板键盘的字母行。iPad 的数字行和 Tab 仍在上方（`KeyboardLayoutPreference.tabletFullKeys`），符号层、九键框架、手写板、假名键和大千注音各行沿用手机上的行。
enum TabletLetterLayout {
  /// 第一行末尾的 ⌫。
  static let deleteWeight: CGFloat = 1.3
  /// 第二行末尾的 return。
  static let returnWeight: CGFloat = 1.75
  /// 第三行的每个 ⇧。
  static let shiftWeight: CGFloat = 1.4
  /// 第二行开头的缩进，以按键宽度的份额计。
  static let middleRowLeadingInset: CGFloat = 0.025

  /// 底行的 123 键，空格键两侧各一个。
  static let layerWeight: CGFloat = 1.5
  /// 中 / 英 切换键。
  static let languageWeight: CGFloat = 1.2
  /// 地球键，宿主要求时才有；设计稿没有画它，所以放在 中 后面，占一个普通键的份额。
  static let globeWeight: CGFloat = 1
  /// 空格键。
  static let spaceWeight: CGFloat = 6.4
  /// ⌄，收起键盘。
  static let dismissWeight: CGFloat = 1.2
  /// 收起键的键面：设计稿用 15pt 文字画这个字符，而不是图标。
  static let dismissFace = "⌄"
}

/// 横屏分离式键盘（`KeyboardLayoutPreference.tabletSplit`）的规则：什么时候分、哪些布局分、从哪里分、中缝多宽。
///
/// 只有平板形态（regular 宽度的 iPad）横屏且开关打开时才分。iPad 的浮动键盘、Slide Over 和台前调度里的窄窗口已经是手机形态，竖屏的平板键盘也不分，它们都保持原样。
///
/// 分的是 26 键字母布局（全拼、各种双拼含微软双拼的 `;` 键、五笔、英文、日文罗马字、韩文和其他字母方案）、它们的 123 符号页，以及 iPad 的数字行和 Tab 那一排；九键、笔画、手写、注音大千和日文假名九键不分，符号、表情、剪贴板等面板也不分。每一排（包括底部功能键那一排）在中间插入一段不接触摸的中缝，键盘高度不变。
enum KeyboardSplitLayout {
  /// 中缝占键区宽度的比例：左右两半最里面的两个键之间空出这么宽。
  static let gapRatio: CGFloat = 0.25

  /// 分离式键盘此刻是否生效。`enabled` 只在平板横屏时才读，手机和竖屏不必去读 App Group。
  static func isActive(formFactor: KeyboardFormFactor, landscape: Bool, enabled: @autoclosure () -> Bool) -> Bool {
    formFactor == .tablet && landscape && enabled()
  }

  /// 当前显示的键区是不是会分开的字母布局。英文和本地输入模式总是画 26 键字母；九键、笔画、假名九键、注音大千和手写画的是各自的键区，不分。
  static func splitsLayout(scheme: ChineseInputScheme, chinese: Bool, localMode: Bool) -> Bool {
    guard chinese, !localMode else { return true }
    switch scheme {
    case .nineKey, .stroke, .japaneseNineKey, .zhuyin, .handwriting: return false
    default: return true
    }
  }

  /// 一排有 `characterKeys` 个字符键（字母、数字、符号，以及平板第三排末尾的逗号句号）时左半分到几个：从中间分，奇数时左半多一个。Tab、Shift 和删除这些功能键不计数，留在它们原来那一侧的外沿。
  ///
  /// 这样得到的正好是触摸打字的左右手分工：qwert | yuiop、asdfg | hjkl（微软双拼多出的 `;` 归右半）、zxcvb | nm，。；数字行和符号页是 5 | 5。
  static func leftKeyCount(_ characterKeys: Int) -> Int { (max(characterKeys, 0) + 1) / 2 }

  /// 中缝视图的宽度：键区宽度的 `gapRatio`，减去中缝两侧各一个键距，使两半最里面的键之间正好空出 `gapRatio`。
  static func gapViewWidth(rowWidth: CGFloat, keySpacing: CGFloat) -> CGFloat {
    max(0, rowWidth * gapRatio - 2 * keySpacing)
  }
}

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

  /// Keyboard height before the composition line, gloss lines and the user's own adjustment.
  ///
  /// Tablet keys are much wider than phone keys, so keeping the phone height there leaves them as flat slabs that are hard to aim at. Unlike the phone, an iPad keyboard gets taller in landscape, as the system keyboard does, because the screen gets wider there.
  ///
  /// The portrait tablet height gives the design's 54pt iPad keys (dc.html L1559, `keyH`) with the default margins and row spacing; landscape keeps the taller keys the system keyboard has there, which the design does not draw.
  ///
  /// The tablet digit row is a whole extra row of keys, so it adds a row's height rather than squeezing the letters.
  func baseHeight(landscape: Bool, handwriting: Bool, numberRow: Bool = false) -> CGFloat {
    switch self {
    case .phone:
      // Handwriting keeps a small allowance in landscape so the writing canvas stays usable.
      guard landscape else { return 260 }
      return handwriting ? 240 : 216
    case .tablet:
      let base: CGFloat = landscape ? 372 : 296
      return numberRow ? base + (landscape ? 68 : 61) : base
    }
  }

  /// Whether the third letter row carries the comma and full-stop keys, as the iPad system keyboard does.
  var showsLetterRowPunctuation: Bool { self == .tablet }

  /// Whether the keyboard can carry the digit row and Tab key of a full-size keyboard (`KeyboardLayoutPreference.tabletFullKeys`). A phone-width keyboard has no room for either.
  var canShowFullKeys: Bool { self == .tablet }
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

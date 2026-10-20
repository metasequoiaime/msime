import UIKit

/// 候选条自己的颜色：client-services 为全局主题（内置主题，或自定义主题的基础主题、导入的包和取色器）解析出的 `candidate` 调色板，使用 `candidate_theme` 要求的模式。
///
/// 在 iOS 上候选条是键盘的一部分，所以平时用键盘调色板绘制（选中的候选用强调色，不填充；见 dc.html `mobCands`）。只有打开 iOS 开关（`followsDesktop`）时，解析出的候选调色板才替换这些颜色，也就是桌面候选窗用的同一套调色板。这个开关和云候选开关一样存在 App Group 里；主题和颜色留在共享文档中，与桌面同步。
struct CandidatePalette: Equatable {
  var text: UIColor
  var number: UIColor
  /// 提示和翻译：用导入包的翻译颜色，没有则用序号颜色，与 client-services 解析 `secondary` 的方式一致。
  var secondary: UIColor
  var accent: UIColor
  var selected: UIColor
  var hover: UIColor
  var surface: UIColor
  var border: UIColor
  var selectedText: UIColor? = nil
  var selectedTranslation: UIColor? = nil
  var preeditText: UIColor? = nil
  var selectedBar: UIColor? = nil
  var showSelectedBar = false
  var layout: CandidateSkinLayout? = nil

  static let followsDesktopKey = "candidate_palette_follows_desktop"
  static var defaults: UserDefaults { InputSchemePreference.appGroupDefaults }
  static var followsDesktop: Bool { defaults.bool(forKey: followsDesktopKey) }

  static let themes: [(id: String, title: String)] = [("follow", "跟随系统"), ("light", "浅色"), ("dark", "深色")]
  /// The `custom_theme.candidate_colors` slots the App edits; the document may carry more (number, accent, selected, border), which are honoured but left to the desktop.
  static let editableColors: [(slot: String, title: String)] = [("text", "文字"), ("surface", "背景"), ("hover", "首选高亮")]

  /// 开关关闭或主题未提供候选色时返回 nil；成功绘制面板后才允许使用贴图配色。
  static func active(in preferences: [String: Any]?, systemDark: Bool, skinsRoot: URL? = ExternalCandidateSkin.defaultRoot,
                     canvasPanelDrawn: Bool = false) -> CandidatePalette? {
    guard followsDesktop, let resolved = resolveTheme(preferences, systemDark: systemDark, skinsRoot: skinsRoot, canvasPanelDrawn: canvasPanelDrawn), resolved.candidate != nil
    else { return nil }
    return palette(resolved)
  }

  /// The strip `preferences` draws with the switch on. A slot the theme leaves to the platform takes the keyboard's colour for it, so `system` draws the native keyboard strip.
  static func resolve(_ preferences: [String: Any]?, systemDark: Bool,
                      skinsRoot: URL? = ExternalCandidateSkin.defaultRoot) -> CandidatePalette {
    palette(resolveTheme(preferences, systemDark: systemDark, skinsRoot: skinsRoot) ?? .system)
  }

  static func resolveTheme(_ preferences: [String: Any]?, systemDark: Bool,
                           skinsRoot: URL? = ExternalCandidateSkin.defaultRoot, canvasPanelDrawn: Bool = false) -> ResolvedTheme? {
    let dark = isDark(candidateTheme: preferences?["candidate_theme"] as? String, appMode: preferences?["theme"] as? String,
                      systemDark: systemDark)
    // 所选主题属于另一种明暗时候选条和键盘一样按跟随系统画。
    return ResolvedTheme.resolve(globalTheme: GlobalThemeCatalog.effectiveId(GlobalThemePreference.theme(in: preferences), dark: dark),
                                 customTheme: GlobalThemePreference.customTheme(in: preferences),
                                 dark: dark, skinsRoot: skinsRoot, canvasPanelDrawn: canvasPanelDrawn)
  }

  static func palette(_ resolved: ResolvedTheme) -> CandidatePalette {
    let keyboard = KeyboardTheme(id: resolved.id, palette: resolved.keyboard, appearance: resolved.appearance, design: nil)
    let candidate = resolved.candidate
    return CandidatePalette(
      text: candidate?.text ?? keyboard.keyForeground,
      number: candidate?.number ?? keyboard.secondary,
      secondary: candidate?.secondary ?? candidate?.number ?? keyboard.secondary,
      accent: candidate?.accent ?? keyboard.accent,
      selected: candidate?.selected ?? keyboard.accentSoft,
      hover: candidate?.hover ?? .clear,
      surface: candidate?.surface ?? keyboard.background,
      border: candidate?.border ?? .clear, selectedText: candidate?.selectedText,
      selectedTranslation: resolved.candidateDetails?.selectedTranslation, preeditText: resolved.candidateDetails?.preeditText,
      selectedBar: resolved.candidateDetails?.selectedBar, showSelectedBar: candidate?.showSelectedBar ?? false, layout: resolved.candidateLayout)
  }

  /// `candidate_theme` first, then the app's light or dark mode (`theme`), then the system.
  static func isDark(candidateTheme: String?, appMode: String?, systemDark: Bool) -> Bool {
    switch candidateTheme {
    case "dark": return true
    case "light": return false
    default: break
    }
    switch appMode {
    case "dark": return true
    case "light": return false
    default: return systemDark
    }
  }
}


/// 共享出厂版式的触屏映射；保留长度单位，在当前字号下换算，不改变键盘的横排与触控下限。
struct CandidateSkinLayout: Equatable {
  struct Length: Equatable {
    let value: CGFloat
    let em: Bool
    init?(_ raw: Any?) {
      guard let raw = raw as? [String: Any], let value = CandidateSkinLayout.number(raw["value"], maximum: 500),
            let em = raw["em"] as? Bool else { return nil }
      self.value = value
      self.em = em
    }
    func dip(fontSize: CGFloat) -> CGFloat { em ? (value * fontSize).rounded() : value }
  }
  let padding: Length?
  let itemGap: Length?
  let itemExtraHeight: Length?
  let itemPaddingRight: Length?
  let itemMinWidth: Length?
  let selectedBarWidth: Length?
  let selectedBarHeightEm: CGFloat?
  let selectedBarInside: Bool?
  let itemRadius: CGFloat?
  let borderWidth: CGFloat?
  let fitContent: Bool

  init?(_ raw: Any?) {
    guard let raw = raw as? [String: Any], let geometry = (raw["horizontal"] ?? raw["geometry"]) as? [String: Any] else { return nil }
    padding = Length(geometry["padding"])
    itemGap = Length(geometry["itemGap"])
    itemExtraHeight = Length(geometry["itemExtraHeight"])
    itemPaddingRight = Length(geometry["itemPaddingRight"])
    itemMinWidth = Length(geometry["itemMinWidth"])
    selectedBarWidth = Length(geometry["selectedBarWidth"])
    selectedBarHeightEm = Self.number(geometry["selectedBarHeightEm"], maximum: 10)
    selectedBarInside = geometry["selectedBarInside"] as? Bool
    itemRadius = Self.number(raw["itemCornerRadiusDip"], maximum: 32)
    borderWidth = Self.number(raw["borderWidthDip"], maximum: 10)
    fitContent = raw["fitContent"] as? Bool ?? false
  }

  static func number(_ raw: Any?, maximum: CGFloat) -> CGFloat? {
    guard let value = raw as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID() else { return nil }
    let result = CGFloat(value.doubleValue)
    return result.isFinite && result >= 0 && result <= maximum ? result : nil
  }

  func horizontalGap(fontSize: CGFloat, fallback: CGFloat = 6) -> CGFloat {
    itemGap.map { $0.dip(fontSize: fontSize) + (fitContent ? 0 : 6) } ?? fallback
  }

  func chipInsets(fontSize: CGFloat, fallback: NSDirectionalEdgeInsets) -> NSDirectionalEdgeInsets {
    let vertical = itemExtraHeight.map { min(fallback.top, $0.dip(fontSize: fontSize) / 2) } ?? fallback.top
    return NSDirectionalEdgeInsets(top: vertical, leading: itemPaddingRight?.dip(fontSize: fontSize) ?? fallback.leading,
                                   bottom: vertical, trailing: itemPaddingRight?.dip(fontSize: fontSize) ?? fallback.trailing)
  }
}

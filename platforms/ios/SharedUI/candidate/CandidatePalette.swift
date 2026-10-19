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
      selectedBar: resolved.candidateDetails?.selectedBar, showSelectedBar: candidate?.showSelectedBar ?? false)
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

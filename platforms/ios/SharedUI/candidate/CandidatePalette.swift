import UIKit

/// The candidate strip's own colours: the `candidate` palette client-core resolves for the global theme (a built-in theme, or the custom theme's base, imported package and colour pickers), in the mode `candidate_theme` asks for.
///
/// On iOS the strip is part of the keyboard, so it normally draws with the keyboard palette (the selected candidate in the accent, no fill; dc.html `mobCands`). The resolved candidate palette only replaces those colours when the iOS switch (`followsDesktop`) is on, the same palette the desktop candidate window draws. The switch lives in the App Group, like the cloud candidate switch; the theme and colours stay in the shared document and sync with the desktop.
struct CandidatePalette: Equatable {
  var text: UIColor
  var number: UIColor
  var accent: UIColor
  var selected: UIColor
  var hover: UIColor
  var surface: UIColor
  var border: UIColor

  static let followsDesktopKey = "candidate_palette_follows_desktop"
  static var defaults: UserDefaults { UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard }
  static var followsDesktop: Bool { defaults.bool(forKey: followsDesktopKey) }

  static let themes: [(id: String, title: String)] = [("follow", "跟随系统"), ("light", "浅色"), ("dark", "深色")]
  /// The `custom_theme.candidate_colors` slots the App edits; the document may carry more (number, accent, selected, border), which are honoured but left to the desktop.
  static let editableColors: [(slot: String, title: String)] = [("text", "文字"), ("surface", "背景"), ("hover", "首选高亮")]

  /// The palette for `preferences`, or nil while the strip draws with the keyboard palette: the switch is off, or the theme resolves no candidate colours (`system`).
  static func active(in preferences: [String: Any]?, systemDark: Bool) -> CandidatePalette? {
    guard followsDesktop, let resolved = resolveTheme(preferences, systemDark: systemDark), resolved.candidate != nil
    else { return nil }
    return palette(resolved)
  }

  /// The strip `preferences` draws with the switch on. A slot the theme leaves to the platform takes the keyboard's colour for it, so `system` draws the native keyboard strip.
  static func resolve(_ preferences: [String: Any]?, systemDark: Bool,
                      skinsRoot: URL? = ExternalCandidateSkin.defaultRoot) -> CandidatePalette {
    palette(resolveTheme(preferences, systemDark: systemDark, skinsRoot: skinsRoot) ?? .system)
  }

  static func resolveTheme(_ preferences: [String: Any]?, systemDark: Bool,
                           skinsRoot: URL? = ExternalCandidateSkin.defaultRoot) -> ResolvedTheme? {
    let dark = isDark(candidateTheme: preferences?["candidate_theme"] as? String, appMode: preferences?["theme"] as? String,
                      systemDark: systemDark)
    return ResolvedTheme.resolve(globalTheme: GlobalThemePreference.theme(in: preferences),
                                 customTheme: GlobalThemePreference.customTheme(in: preferences),
                                 dark: dark, skinsRoot: skinsRoot)
  }

  static func palette(_ resolved: ResolvedTheme) -> CandidatePalette {
    let keyboard = KeyboardTheme(id: resolved.id, palette: resolved.keyboard, appearance: resolved.appearance, design: nil)
    let candidate = resolved.candidate
    return CandidatePalette(
      text: candidate?.text ?? keyboard.keyForeground,
      number: candidate?.number ?? keyboard.secondary,
      accent: candidate?.accent ?? keyboard.accent,
      selected: candidate?.selected ?? keyboard.accentSoft,
      hover: candidate?.hover ?? .clear,
      surface: candidate?.surface ?? keyboard.background,
      border: candidate?.border ?? .clear)
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

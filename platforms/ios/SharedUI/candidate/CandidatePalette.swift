import UIKit

/// 「候选皮肤」 on the candidate strip: the four built-in desktop candidate skins or one imported into the shared skins folder (see ExternalCandidateSkin), `candidate_theme` and the candidate colour overrides from the shared document.
///
/// The strip normally follows the keyboard skin, because on iOS it is part of the keyboard rather than a window of its own. `candidate_skin` cannot say whether the user chose it, since every document carries the default `willow_green`, so the desktop palette only replaces the keyboard skin's colours when the iOS switch (`followsDesktop`) is on. The switch lives in the App Group, like the cloud candidate switch; the skin, theme and colours stay in the shared document and sync with the desktop.
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

  /// Ids and titles published once by client-core's `BUILTIN_SKINS`.
  static let skins: [(id: String, title: String)] = [
    ("fluent", "Fluent"), ("wechat", "微信绿"), ("graphite", "石墨"), ("willow_green", "杨柳青"),
  ]
  static let defaultSkin = "willow_green"
  static let themes: [(id: String, title: String)] = [("follow", "跟随系统"), ("light", "浅色"), ("dark", "深色")]
  /// The overrides the App edits; the document may carry more (accent, selected, border), which are honoured but left to the desktop.
  static let editableColors: [(key: String, title: String)] = [
    ("candidate_text_color", "文字"), ("candidate_surface_color", "背景"), ("candidate_hover_color", "首选高亮"),
  ]

  /// The palette for `preferences`, or nil while the strip follows the keyboard skin.
  static func active(in preferences: [String: Any]?, systemDark: Bool) -> CandidatePalette? {
    followsDesktop ? resolve(preferences, systemDark: systemDark) : nil
  }

  static func resolve(_ preferences: [String: Any]?, systemDark: Bool,
                      skinsRoot: URL? = ExternalCandidateSkin.defaultRoot) -> CandidatePalette {
    let values = preferences ?? [:]
    let dark = isDark(candidateTheme: values["candidate_theme"] as? String, globalTheme: values["theme"] as? String,
                      systemDark: systemDark)
    let skin = values["candidate_skin"] as? String ?? defaultSkin
    var palette = skins.contains { $0.id == skin }
      ? builtIn(skin, dark: dark)
      : external(skin, dark: dark, root: skinsRoot) ?? builtIn(defaultSkin, dark: dark)
    func custom(_ key: String) -> UIColor? { color(hex: values[key] as? String) }
    if let text = custom("candidate_text_color") {
      palette.text = text
      // An explicit text colour also sets the number colour, half-transparent, unless that is set too.
      palette.number = text.withAlphaComponent(CGFloat(0x9d) / 255)
    }
    if let number = custom("candidate_number_color") { palette.number = number }
    if let accent = custom("candidate_accent_color") { palette.accent = accent }
    if let selected = custom("candidate_selected_color") { palette.selected = selected }
    if let hover = custom("candidate_hover_color") { palette.hover = hover }
    // Linux writes an installed skin's surface as `candidate_background_color`; either name sets the surface.
    if let surface = custom("candidate_background_color") ?? custom("candidate_surface_color") { palette.surface = surface }
    if let border = custom("candidate_border_color") { palette.border = border }
    return palette
  }

  /// An imported skin: the built-in skin it extends, then the colours it declares for this theme. Nil when the package is missing, invalid, or does not support the horizontal strip in this theme, which leaves the shared default.
  static func external(_ id: String, dark: Bool, root: URL?) -> CandidatePalette? {
    guard let package = ExternalCandidateSkin.load(id, dark: dark, root: root) else { return nil }
    var palette = builtIn(package.base, dark: dark)
    let colors = package.candidate[dark ? "dark" : "light"] ?? [:]
    if let text = color(hex: colors["text"]) { palette.text = text }
    if let number = color(hex: colors["number"]) { palette.number = number }
    if let accent = color(hex: colors["accent"]) { palette.accent = accent }
    if let selected = color(hex: colors["selected"]) { palette.selected = selected }
    if let hover = color(hex: colors["hover"]) { palette.hover = hover }
    if let surface = color(hex: colors["surface"]) { palette.surface = surface }
    if let border = borderColor(colors["border"]) { palette.border = border }
    return palette
  }

  /// A skin border may also carry alpha (`#RRGGBBAA`) or be `transparent`, as the desktop candidate windows accept.
  static func borderColor(_ value: String?) -> UIColor? {
    guard let value else { return nil }
    if value == "transparent" { return .clear }
    if let opaque = color(hex: value) { return opaque }
    guard value.count == 9, value.first == "#", let rgba = UInt32(value.dropFirst(), radix: 16) else { return nil }
    return argb((rgba & 0xff) << 24 | rgba >> 8)
  }

  /// `candidate_theme` first, then the global `theme`, then the system.
  static func isDark(candidateTheme: String?, globalTheme: String?, systemDark: Bool) -> Bool {
    switch candidateTheme {
    case "dark": return true
    case "light": return false
    default: break
    }
    switch globalTheme {
    case "dark": return true
    case "light": return false
    default: return systemDark
    }
  }

  /// `#RRGGBB` only, the form client-core validates; anything else is ignored.
  static func color(hex: String?) -> UIColor? {
    guard let hex, hex.count == 7, hex.first == "#", let value = UInt32(hex.dropFirst(), radix: 16) else { return nil }
    return argb(0xff00_0000 | value)
  }

  static func hex(_ color: UIColor) -> String {
    var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
    color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
    let channel = { (value: CGFloat) in Int((SharedNumber.clamped(value, to: 0...1) * 255).rounded()) }
    return String(format: "#%02x%02x%02x", channel(red), channel(green), channel(blue))
  }

  /// The same values as Android's `CandidateAppearance`, which follow the Windows candidate skins.
  static func builtIn(_ id: String, dark: Bool) -> CandidatePalette {
    switch id {
    case "fluent":
      dark ? make(0xffe9e8e8, 0xffe9e89d, 0x2e9b9b9b, 0xb93e3e3e, 0xff414141, 0xff202020)
        : make(0xff1a1a1a, 0x8c1a1a1a, 0x1f000000, 0xffe8e8e8, 0xffececec, 0xffffffff)
    case "wechat":
      dark ? make(0xffb7b7b7, 0xff858585, 0xff292929, 0xff07c160, 0x5207c160, 0xff151515)
        : make(0xff333333, 0xff757575, 0xffdedede, 0xff07c160, 0x2407c160, 0xfff7f7f7)
    case "graphite":
      dark ? make(0xffaeb6c2, 0xff707987, 0xff30353b, 0x00000000, 0x0effffff, 0xff1c1f23)
        : make(0xff586476, 0xff8993a1, 0xffe2e5e9, 0x00000000, 0x0e1f2937, 0xfffbfbfc)
    case "willow_green":
      dark ? make(0xffd8dbd8, 0xffa6aba7, 0x00000000, 0xff65c98d, 0x3865c98d, 0xff2d2f2e)
        : make(0xff343936, 0xff686f6a, 0x00000000, 0xff58b980, 0x2958b980, 0xfff4f5f3)
    default:
      builtIn(defaultSkin, dark: dark)
    }
  }

  private static func make(_ text: UInt32, _ number: UInt32, _ border: UInt32, _ selected: UInt32,
                           _ hover: UInt32, _ surface: UInt32) -> CandidatePalette {
    CandidatePalette(text: argb(text), number: argb(number), accent: argb(selected == 0 ? text : selected),
                     selected: argb(selected), hover: argb(hover), surface: argb(surface), border: argb(border))
  }

  private static func argb(_ value: UInt32) -> UIColor {
    UIColor(red: CGFloat((value >> 16) & 0xff) / 255, green: CGFloat((value >> 8) & 0xff) / 255,
            blue: CGFloat(value & 0xff) / 255, alpha: CGFloat(value >> 24) / 255)
  }
}

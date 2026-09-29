import UIKit

/// How the touch keyboard draws one global theme: the keyboard palette client-core resolved for it, or UIKit's own tokens when it resolved none (`system`, or a custom theme with no base and no design), plus the keyboard design of a custom theme that has one.
///
/// A design is drawn in full (photo, gradient, pattern, key shape and material); every other theme draws flat keys in the design's native geometry (dc.html L1555-1558).
struct KeyboardTheme: Equatable {
  let id: String
  /// The resolved `keyboard` palette; nil draws the native tokens.
  let palette: ThemeKeyboardPalette?
  /// The fixed mode of the theme's surfaces, nil to follow the keyboard's mode.
  let appearance: UIUserInterfaceStyle?
  /// `custom_theme.keyboard`, only while the custom theme is the one drawn.
  let design: CustomKeyboardSkin?

  init(id: String, palette: ThemeKeyboardPalette?, appearance: UIUserInterfaceStyle?, design: CustomKeyboardSkin?) {
    self.id = id
    self.palette = palette
    self.appearance = appearance
    self.design = id == GlobalThemeCatalog.customId ? design?.normalized : nil
  }

  /// The custom theme drawn from `design` alone, for previews of a design that is not applied yet.
  static func designed(_ design: CustomKeyboardSkin) -> KeyboardTheme {
    KeyboardTheme(id: GlobalThemeCatalog.customId, palette: nil, appearance: nil, design: design)
  }

  static let system = KeyboardTheme(id: GlobalThemeCatalog.systemId, palette: nil, appearance: nil, design: nil)

  /// Theme `id` as `document` configures it. The keyboard palette does not depend on the mode, so this resolves once for both. Without a document the App Group design stands in for `custom_theme`, as the keyboard sees it before its session loads.
  static func resolve(_ id: String, document: [String: Any]?) -> KeyboardTheme {
    var custom = GlobalThemePreference.customTheme(in: document)
    let design: CustomKeyboardSkin?
    if document == nil {
      design = CustomKeyboardSkinStore.stored
      if let design, let value = CustomKeyboardSkin.documentValue(design) { custom["keyboard"] = value }
    } else {
      design = GlobalThemePreference.design(in: document)
    }
    guard let resolved = ResolvedTheme.resolve(globalTheme: id, customTheme: custom, dark: false) else {
      return KeyboardTheme(id: id, palette: nil, appearance: nil, design: design)
    }
    return KeyboardTheme(id: resolved.id, palette: resolved.keyboard, appearance: resolved.appearance, design: design)
  }

  /// The selected theme of `document`, or of the App Group without one.
  static func resolve(document: [String: Any]?) -> KeyboardTheme {
    resolve(document == nil ? GlobalThemePreference.selected : GlobalThemePreference.theme(in: document), document: document)
  }

  private static let lock = NSLock()
  private static var stored: KeyboardTheme?

  /// The theme the keyboard draws now. The keyboard sets it from its session's document whenever the document or the selection changes (`reload`); until then, and in the app, it is resolved from the shared document once.
  static var current: KeyboardTheme {
    lock.lock()
    let theme = stored
    lock.unlock()
    return theme ?? reload(MetasequoiaInputSessionBridge.loadSharedPreferences())
  }

  /// Resolve the selected theme of `document` again and make it `current`.
  @discardableResult
  static func reload(_ document: [String: Any]?) -> KeyboardTheme {
    let theme = resolve(document: document)
    lock.lock()
    stored = theme
    lock.unlock()
    return theme
  }

  var title: String { GlobalThemeCatalog.title(id) }
  var isCustom: Bool { id == GlobalThemeCatalog.customId }

  var background: UIColor {
    if let design { return CustomKeyboardSkin.color(design.background) }
    return palette?.background ?? NativeKeyboardTokens.background
  }
  /// Letter keys and the space bar (`kb.key`).
  var keyBackground: UIColor {
    if let design { return CustomKeyboardSkin.color(design.keyBackground).withAlphaComponent(CGFloat(design.keyOpacity ?? 1)) }
    return palette?.key ?? NativeKeyboardTokens.key
  }
  /// Shift, delete, 123, the symbol and language keys (`kb.spec`).
  var functionKeyBackground: UIColor {
    if design != nil { return keyBackground }
    return palette?.functionKey ?? NativeKeyboardTokens.functionKey
  }
  var keyForeground: UIColor {
    if let design { return CustomKeyboardSkin.color(design.keyForeground) }
    return palette?.text ?? NativeKeyboardTokens.text
  }
  /// Key hints, the space bar label, candidate numbers (`kb.sub`).
  var secondary: UIColor {
    palette?.secondary ?? (design.map { CustomKeyboardSkin.color($0.keyForeground).withAlphaComponent(0.6) } ?? NativeKeyboardTokens.secondary)
  }
  /// The selected candidate, hints and toggled marks.
  var accent: UIColor {
    if let design { return CustomKeyboardSkin.color(design.accent) }
    return palette?.accent ?? NativeKeyboardTokens.accent
  }
  /// The emphasized key (return while composing). It is not a theme colour: the design fills it with the platform accent in every theme (dc.html L2211, `KeyboardThemePalette` in client-core); a keyboard design keeps its own action colour.
  var actionBackground: UIColor {
    design.map { CustomKeyboardSkin.color($0.actionBackground) } ?? NativeKeyboardTokens.accent
  }
  /// The design writes white on the accent in both modes, which measures 2.3:1 on the dark accent `#5FBF84`; the dark mode takes black instead so the label stays readable (4.5:1 and up).
  var actionForeground: UIColor {
    if let design { return CustomKeyboardSkin.color(CustomKeyboardSkin.readableText(on: design.actionBackground)) }
    return UIColor { $0.userInterfaceStyle == .dark ? .black : .white }
  }
  /// The theme accent at 20%: the fallback for the candidate strip's `selected` slot.
  var accentSoft: UIColor {
    if design != nil || palette != nil { return accent.withAlphaComponent(0.2) }
    return NativeKeyboardTokens.accentSoft
  }
  /// A switched-on tile's fill. Like the return key it is not a theme colour: the design draws `tileOn` from the platform tokens in every theme (`k.accentSoft`, dc.html `tileOn`); a keyboard design keeps a tint of its own accent.
  var toggleBackground: UIColor {
    design == nil ? NativeKeyboardTokens.accentSoft : accentSoft
  }
  /// A switched-on tile's label: the platform accent (`k.accentText`), or a keyboard design's own accent.
  var toggleForeground: UIColor {
    design == nil ? NativeKeyboardTokens.accent : accent
  }

  /// A design's own radius, else the design's native key radius: 5 on a phone, 7 on an iPad.
  var cornerRadius: CGFloat {
    if let design { return CGFloat(design.cornerRadius) }
    return UIDevice.current.userInterfaceIdiom == .pad ? 7 : 5
  }
  var borderWidth: CGFloat { design.map { CGFloat($0.borderWidth) } ?? 0 }
  var borderColor: UIColor {
    if let rgb = design?.customBorderColor { return CustomKeyboardSkin.color(rgb) }
    return accent.withAlphaComponent(0.28)
  }
  /// The key's drop shadow, alpha included: a design's own, else the native key edge `0 1px 0 rgba(0,0,0,.3)` (`.6` dark).
  var shadowColor: UIColor {
    if let design { return UIColor.black.withAlphaComponent(CGFloat(design.shadow)) }
    return UIColor { traits in
      UIColor.black.withAlphaComponent(CGFloat(traits.userInterfaceStyle == .dark
        ? NativeKeyboardTokens.keyShadowOpacity.dark : NativeKeyboardTokens.keyShadowOpacity.light))
    }
  }
  var hasShadow: Bool { design.map { $0.shadow > 0 } ?? true }
  var shadowRadius: CGFloat { design == nil ? 0 : 3 }
  var shadowOffset: CGFloat { design == nil ? 1 : 2 }
  var usesMonospacedFont: Bool { design?.monospaced ?? false }
  var pattern: Int { design?.pattern ?? 0 }
}

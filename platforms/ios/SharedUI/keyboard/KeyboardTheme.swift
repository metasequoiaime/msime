import UIKit

/// 触屏键盘如何绘制一个全局主题：使用 client-core 为它解析出的键盘配色；没有解析出配色时（`system`，或既无基础主题也无设计的自定义主题），使用应用主题所在季节的跟随系统配色（`SeasonKeyboardTokens`），应用主题也解析不了时退回 UIKit 自带 token 作为经典兜底；此外还有自定义主题自带的键盘设计。
///
/// A design is drawn in full (photo, gradient, pattern, key shape and material); every other theme draws flat keys in the design's native geometry (dc.html L1555-1558).
struct KeyboardTheme: Equatable {
  let id: String
  /// 解析出的 `keyboard` 配色；主题没有解析出配色时为季节配色；为 nil 时绘制原生 token。
  let palette: ThemeKeyboardPalette?
  /// The fixed mode of the theme's surfaces, nil to follow the keyboard's mode.
  let appearance: UIUserInterfaceStyle?
  /// `custom_theme.keyboard`, only while the custom theme is the one drawn.
  let design: CustomKeyboardSkin?

  init(id: String, palette: ThemeKeyboardPalette?, appearance: UIUserInterfaceStyle?, design: CustomKeyboardSkin?) {
    self.id = id
    self.appearance = appearance
    self.design = id == GlobalThemeCatalog.customId ? design?.normalized : nil
    self.palette = palette ?? (self.design == nil && SeasonKeyboardTokens.isAvailable ? SeasonKeyboardTokens.palette : nil)
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
  /// 键盘是否用 UIKit 自带 token 绘制：没有解析出的配色、没有设计，也没有可用于跟随系统季节配色的应用主题，因此只有经典兜底会让背景保持透明。iOS 26 有时会在第三方键盘上方、扩展窗口之外画出一条系统自己的键盘底板，扩展画什么都盖不住它。使用原生 token 的键盘让背景保持透明，同一块底板就能透出来，那一条看上去就成了键盘的一部分。
  var drawsNativeBackground: Bool { palette == nil && design == nil }

  /// 键盘进程可能存活好几天，足够水杉四季换到下一个季节，或者应用换选另一个应用主题。键盘在 `viewWillAppear` 里读取 `current` 之前调用它；季节不在别处缓存，跟随系统配色、`logoCircle` 和 `logoMark` 每次解析时都会读它。
  static func refreshSeason() {
    AppThemePalette.refresh()
  }

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
  /// 强调键（组字时的回车）：主题自己的主色；跟随系统配色时为季节主色；键盘设计时为它的操作色；什么都没解析出来时为经典绿色。
  var actionBackground: UIColor {
    if let design { return CustomKeyboardSkin.color(design.actionBackground) }
    return palette?.accent ?? NativeKeyboardTokens.accent
  }
  /// `actionBackground` 上的文字：配色自己的 `on_accent`，跟随系统配色在浅色下为白色、深色下为 Rust 算出的 mix(accent 25%, #000)。经典兜底在深色主色 `#5FBF84` 上用黑字，因为设计稿的白字在那里对比度只有 2.3:1。
  var actionForeground: UIColor {
    if let design { return CustomKeyboardSkin.color(CustomKeyboardSkin.readableText(on: design.actionBackground)) }
    if let palette { return palette.onAccent }
    return UIColor { $0.userInterfaceStyle == .dark ? .black : .white }
  }
  /// 画在 `accent` 填充上的图形，例如语音面板的麦克风：配色自己的 `on_accent`；键盘设计时为其主色上可读的文字色；经典主色上与 `actionForeground` 一样用白或黑。
  var onAccent: UIColor {
    if let design { return CustomKeyboardSkin.color(CustomKeyboardSkin.readableText(on: design.accent)) }
    if let palette { return palette.onAccent }
    return UIColor { $0.userInterfaceStyle == .dark ? .black : .white }
  }
  /// 作为浅色调的主题主色：凡是有配色的主题，浅色下透明度 `22`、深色下 `40`（即设计稿的 `accentSoft`，与 Android 的 `KeyboardSkin` 一致）；什么都没解析出来时为经典的 `.14` / `.26` 绿色；键盘设计时为其主色的 20%。它是候选栏 `selected` 槽位的兜底。
  var accentSoft: UIColor {
    if design != nil { return accent.withAlphaComponent(0.2) }
    guard let palette else { return NativeKeyboardTokens.accentSoft }
    // 尽量用共享或静态颜色：候选栏配色会做相等比较，而新建的动态颜色永远不等于另一个颜色。
    if palette == SeasonKeyboardTokens.palette { return SeasonKeyboardTokens.accentSoft }
    let soft = { (dark: Bool) in palette.accent.withAlphaComponent(dark ? 0x40 / 255 : 0x22 / 255) }
    if let appearance { return soft(appearance == .dark) }
    return UIColor { soft($0.userInterfaceStyle == .dark).resolvedColor(with: $0) }
  }
  /// 开启状态磁贴的文字、图标与角标：当前主题的主色。
  var toggleForeground: UIColor { accent }
  /// 激活的工具栏按钮（其面板已打开）背后的底板：当前主题的 `accentSoft`。
  var toolbarActiveBackground: UIColor { accentSoft }

  /// 不论哪种模式表面都是深色的主题 id，它们的分隔线使用白色细线。
  private static let darkSurfaceIds: Set<String> = ["shuishan", "night", "ink"]

  /// 分隔线、虚线辅助线、未激活的页码圆点和未选中的圆环（设计稿的 `kbHair`）：深色键盘上为白色 .14，浅色键盘上为黑色 .12。主题在深色模式下、固定深色外观下，或属于 `darkSurfaceIds` 时算深色；键盘设计在背景相对亮度低于 0.5 时算深色。
  var hairline: UIColor {
    let light = UIColor(white: 0, alpha: 0.12), dark = UIColor(white: 1, alpha: 0.14)
    if let design { return CustomKeyboardSkin.luminance(design.background) < 0.5 ? dark : light }
    let alwaysDark = Self.darkSurfaceIds.contains(id), appearance = self.appearance
    return UIColor { traits in alwaysDark || Self.isDark(traits, appearance) ? dark : light }
  }

  /// 工具栏 logo 的圆底：不论皮肤，都取应用主题季节的 mix(accent 14% light / 22% dark, card)；应用主题解析不了时为 `#FFFFFF` / `#1C1C1E` 上的经典绿色。
  var logoCircle: UIColor { Self.logoCircleColor }
  /// 圆底里的 logo 图形：应用主题季节的 mix(accent 82%, #000)；应用主题解析不了时为经典绿色。
  var logoMark: UIColor { Self.logoMarkColor }

  private static let logoCircleColor = MetasequoiaTheme.mixUIColor(14, 22)
  private static let logoMarkColor = UIColor { traits in
    AppThemePalette.mix(MetasequoiaTheme.accentUIColor.resolvedColor(with: traits), 82, .black)
  }

  /// `traits` 是否绘制为深色，主题的固定外观优先于 trait 自身的样式。
  private static func isDark(_ traits: UITraitCollection, _ appearance: UIUserInterfaceStyle?) -> Bool {
    (appearance ?? traits.userInterfaceStyle) == .dark
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
  /// 按键投影，含透明度：优先用设计自带的，否则为按键边缘 `0 1px 0 rgba(38,62,44,.3)`（浅色）和 `rgba(0,0,0,.55)`（深色）（`NativeKeyboardTokens.keyShadowColor`）。绘制类按键单元格的面板会连同 `shadowOffset` 和 `shadowRadius` 一起复用它。
  var keyShadowColor: UIColor {
    if let design { return UIColor.black.withAlphaComponent(CGFloat(design.shadow)) }
    return Self.nativeKeyShadowColor
  }
  /// 以按键绘制和皮肤预览读取的名字暴露的 `keyShadowColor`。
  var shadowColor: UIColor { keyShadowColor }

  private static let nativeKeyShadowColor = UIColor { traits in
    let dark = traits.userInterfaceStyle == .dark
    return NativeKeyboardTokens.keyShadowColor.resolvedColor(with: traits).withAlphaComponent(CGFloat(dark
      ? NativeKeyboardTokens.keyShadowOpacity.dark : NativeKeyboardTokens.keyShadowOpacity.light))
  }
  var hasShadow: Bool { design.map { $0.shadow > 0 } ?? true }
  var shadowRadius: CGFloat { design == nil ? NativeKeyboardTokens.keyShadowRadius : 3 }
  var shadowOffset: CGFloat { design == nil ? NativeKeyboardTokens.keyShadowOffset : 2 }
  var usesMonospacedFont: Bool { design?.monospaced ?? false }
  var pattern: Int { design?.pattern ?? 0 }
}

import UIKit

@_silgen_name("msime_client_theme_catalog")
private func msimeThemeCatalog() -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_resolve_theme")
private func msimeResolveTheme(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_string_free")
private func msimeThemeStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// The ABI envelope `{"ok": true, "value": …}`, or nil for an error or a missing reply.
private func themeReply(_ raw: UnsafeMutablePointer<CChar>?) -> Any? {
  guard let raw else { return nil }
  defer { msimeThemeStringFree(raw) }
  guard let reply = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any],
        reply["ok"] as? Bool == true else { return nil }
  return reply["value"]
}

/// A colour client-core publishes: `#RRGGBB` or `#RRGGBBAA`, alpha last. Anything else reads as unset, which means the native token.
enum ThemeColor {
  static func parse(_ value: Any?) -> UIColor? {
    guard let value = value as? String, value.first == "#", value.count == 7 || value.count == 9,
          let bits = UInt32(value.dropFirst(), radix: 16) else { return nil }
    let rgba = value.count == 7 ? bits << 8 | 0xff : bits
    return UIColor(red: CGFloat(rgba >> 24 & 0xff) / 255, green: CGFloat(rgba >> 16 & 0xff) / 255,
                   blue: CGFloat(rgba >> 8 & 0xff) / 255, alpha: CGFloat(rgba & 0xff) / 255)
  }

  /// `#RRGGBB`, the form the candidate colour pickers store.
  static func hex(_ color: UIColor) -> String {
    var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
    color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
    let channel = { (value: CGFloat) in Int((SharedNumber.clamped(value, to: 0...1) * 255).rounded()) }
    return String(format: "#%02X%02X%02X", channel(red), channel(green), channel(blue))
  }
}

/// 不透明的 `0xRRGGBB` 颜色。
private func rgbColor(_ value: UInt32, alpha: CGFloat = 1) -> UIColor {
  UIColor(red: CGFloat(value >> 16 & 0xff) / 255, green: CGFloat(value >> 8 & 0xff) / 255,
          blue: CGFloat(value & 0xff) / 255, alpha: alpha)
}

/// 每种模式各一个 `0xRRGGBB` 颜色。
private func adaptiveColor(_ light: UInt32, _ dark: UInt32, lightAlpha: CGFloat = 1, darkAlpha: CGFloat = 1) -> UIColor {
  UIColor { traits in
    traits.userInterfaceStyle == .dark ? rgbColor(dark, alpha: darkAlpha) : rgbColor(light, alpha: lightAlpha)
  }
}

/// 设计稿里 UIKit 自身的键盘配色 token（dc.html L1555-1558）：应用主题无法解析时 `system` 主题画的经典兜底配色（见 `SeasonKeyboardTokens`），也是已解析但槽位为 null 时的回退值。
enum NativeKeyboardTokens {
  static let background = adaptiveColor(0xD1D4DB, 0x2B2B2D)
  static let key = adaptiveColor(0xFFFFFF, 0x6B6B6E)
  static let functionKey = adaptiveColor(0xABB0BB, 0x464648)
  static let text = adaptiveColor(0x000000, 0xFFFFFF)
  static let secondary = adaptiveColor(0x8A8A8E, 0x8E8E93)
  /// The brand accent: the return key while composing, the selected candidate, switched-on tiles and pills.
  static let accent = adaptiveColor(0x2C7A4B, 0x5FBF84)
  static let accentSoft = adaptiveColor(0x2C7A4B, 0x5FBF84, lightAlpha: 0.14, darkAlpha: 0.26)
  /// 除键盘设计主题外，所有主题里按键的底边：浅色为 `0 1px 0 rgba(38,62,44,.3)`，深色为 `0 1px 0 rgba(0,0,0,.55)`。颜色本身不透明，各模式的透明度由 `keyShadowOpacity` 承载。
  static let keyShadowColor = adaptiveColor(0x263E2C, 0x000000)
  static let keyShadowOpacity: (light: Float, dark: Float) = (0.3, 0.55)
  /// 底边紧贴在按键正下方，不做模糊。
  static let keyShadowOffset: CGFloat = 1
  static let keyShadowRadius: CGFloat = 0
}

/// 「原生」皮肤（`native`）的键盘 token：照 iOS 自带键盘画，不跟季节。背景不画，交给 `KeyboardInputView`（`UIInputView` 的 `.keyboard` 样式）自带的系统键盘底板：iOS 26 起是 Liquid Glass，更早的系统是系统模糊材质；这里的 `background` 只给没有系统底板的预览用。
///
/// iOS 26 起的按键取值是在 iOS 27 模拟器上对系统键盘截图量的：浅色下底板 `#E2E3E8`、字母键和功能键同为 `#FFFFFF`，深色下底板 `#222223`、按键同为 `#464646`，没有底边阴影，iPhone 上圆角约 6pt。更早的系统沿用 `NativeKeyboardTokens` 的经典 UIKit 取值，底边阴影换成不带品牌绿的黑色。
enum SystemKeyboardTokens {
  /// 是否画 iOS 26 起的系统键盘：底板是 Liquid Glass，按键扁平、同色、无阴影。
  static var drawsLiquidGlassKeyboard: Bool {
    if #available(iOS 26.0, *) { return true }
    return false
  }

  static let background = drawsLiquidGlassKeyboard ? adaptiveColor(0xE2E3E8, 0x222223) : NativeKeyboardTokens.background
  static let key = drawsLiquidGlassKeyboard ? adaptiveColor(0xFFFFFF, 0x464646) : NativeKeyboardTokens.key
  static let functionKey = drawsLiquidGlassKeyboard ? key : NativeKeyboardTokens.functionKey
  /// iPhone 上量得约 6pt；iPad 没有量过，按经典取值比例（5 比 7）放大。
  static var cornerRadius: CGFloat { UIDevice.current.userInterfaceIdiom == .pad ? 8 : 6 }
  /// 回车键和选中候选用的系统蓝：UIKit `systemBlue` 的增强对比度取值（浅色 `#0040DD`、深色 `#409CFF`）。常规的 `#007AFF` 写在浅色键盘底板上对比度不到 2:1，候选文字看不清。
  static let accent = adaptiveColor(0x0040DD, 0x409CFF)
  /// 系统蓝上的文字：浅色下白色；深色下的蓝偏亮，白字对比度不到 3:1，与经典兜底一样改用黑字。
  static let onAccent = adaptiveColor(0xFFFFFF, 0x000000)
  static let accentSoft = adaptiveColor(0x0040DD, 0x409CFF, lightAlpha: 0.14, darkAlpha: 0.26)
  /// 工具栏 logo 的圆底：与其他皮肤同一个配方 mix(强调色 14% 浅色 / 22% 深色, 卡片底)，只是强调色换成系统蓝、卡片底取系统的 `#FFFFFF` / `#1C1C1E`，不跟季节。
  static let logoCircle = UIColor { traits in
    let dark = traits.userInterfaceStyle == .dark
    return AppThemePalette.mix(accent.resolvedColor(with: traits), dark ? 22 : 14, rgbColor(dark ? 0x1C1C1E : 0xFFFFFF))
  }
  /// 圆底里的 logo 图形：mix(系统蓝 82%, #000)，白色折线描边画在它上面依然清楚。
  static let logoMark = UIColor { traits in
    AppThemePalette.mix(accent.resolvedColor(with: traits), 82, .black)
  }
  /// iOS 26 之前的按键底边：中性黑色，浅色 .3、深色 .55，与 `NativeKeyboardTokens` 的透明度相同。
  static let keyShadowColor = UIColor { traits in
    UIColor.black.withAlphaComponent(traits.userInterfaceStyle == .dark ? 0.55 : 0.3)
  }
}

/// 应用主题当前季节下的 `system`（跟随系统）键盘（设计 token §1.4，Android `AppThemePalette.keyboard*`）：面板、字母键和功能键由季节强调色与页面色混合而成，文字为黑或白，提示文字用设计稿的 `kbSub`，回车键用强调色填充。
///
/// 每种颜色都是动态色，UIKit 解析时才读取 `AppThemePalette.resolved(dark:)`，所以用 `KeyboardTheme.refreshSeason()` 刷新季节后，下一次重绘即可生效，不必重建主题。应用主题无法解析时键盘改画 `NativeKeyboardTokens`（`isAvailable` 为 false），因此各 provider 自己回退到这些 token 的分支只兜住会话中途解析失效的主题库。
enum SeasonKeyboardTokens {
  /// 应用主题能否解析；能解析时才用这套配色取代原生 token。
  static var isAvailable: Bool { AppThemePalette.resolved(dark: false) != nil }

  static let palette = ThemeKeyboardPalette(
    background: derivedColor(\.background, fallback: NativeKeyboardTokens.background),
    key: derivedColor(\.key, fallback: NativeKeyboardTokens.key),
    functionKey: derivedColor(\.functionKey, fallback: NativeKeyboardTokens.functionKey),
    text: adaptiveColor(0x000000, 0xFFFFFF),
    secondary: adaptiveColor(0x5A6B5D, 0x93A596),
    accent: UIColor { traits in
      AppThemePalette.resolved(dark: traits.userInterfaceStyle == .dark)?.accent
        ?? NativeKeyboardTokens.accent.resolvedColor(with: traits)
    },
    // 浅色强调色上用白色；深色下用 Rust 的 `on_accent`，即 mix(accent 25%, #000)，因为白色压在深色模式偏浅的强调色上对比度不到 2:1。
    onAccent: UIColor { traits in
      guard traits.userInterfaceStyle == .dark else { return .white }
      return AppThemePalette.resolved(dark: true)?.onAccent ?? .black
    })

  /// 季节强调色，浅色下透明度为 `22`，深色下为 `40`，对应 Rust 的 `accent_soft`。只共享一个实例，这样由这套配色构建出的配色和主题仍然判等。
  static let accentSoft = UIColor { traits in
    palette.accent.resolvedColor(with: traits).withAlphaComponent(traits.userInterfaceStyle == .dark ? 0x40 / 255 : 0x22 / 255)
  }

  /// 一种模式下的三种混合表面色，连同混合时所用的季节一起保存，季节刷新后会重新混合。
  private struct Surfaces {
    let season: AppThemePalette.Resolved
    let background, key, functionKey: UIColor
  }

  private static let lock = NSLock()
  /// 下标 0 为浅色，1 为深色。每个按键每次重绘都要解析这些颜色，而 `AppThemePalette.mix` 要格式化字符串，所以每个季节只混合一次。
  private static var surfaces: [Surfaces?] = [nil, nil]

  private static func derivedColor(_ slot: KeyPath<Surfaces, UIColor>, fallback: UIColor) -> UIColor {
    UIColor { traits in
      mixed(dark: traits.userInterfaceStyle == .dark)?[keyPath: slot] ?? fallback.resolvedColor(with: traits)
    }
  }

  private static func mixed(dark: Bool) -> Surfaces? {
    guard let season = AppThemePalette.resolved(dark: dark) else { return nil }
    let index = dark ? 1 : 0
    return lock.withLock {
      if let cached = surfaces[index], cached.season == season { return cached }
      let accent = season.accent
      let made = dark
        ? Surfaces(season: season, background: AppThemePalette.mix(accent, 10, rgbColor(0x161716)),
                   key: AppThemePalette.mix(accent, 10, rgbColor(0x3A3C3A)),
                   functionKey: AppThemePalette.mix(accent, 14, rgbColor(0x262826)))
        : Surfaces(season: season, background: AppThemePalette.mix(accent, 12, season.background),
                   key: AppThemePalette.mix(season.background, 25, .white),
                   functionKey: AppThemePalette.mix(accent, 24, season.background))
      surfaces[index] = made
      return made
    }
  }
}

/// The touch keyboard slots of a resolved theme (`KeyboardThemePalette`).
struct ThemeKeyboardPalette: Equatable {
  var background: UIColor
  var key: UIColor
  var functionKey: UIColor
  var text: UIColor
  var secondary: UIColor
  /// The selected candidate's text in the strip, drawn with no fill.
  var accent: UIColor
  /// Text on anything filled with `accent`.
  var onAccent: UIColor

  init(background: UIColor, key: UIColor, functionKey: UIColor, text: UIColor, secondary: UIColor,
       accent: UIColor, onAccent: UIColor) {
    (self.background, self.key, self.functionKey, self.text) = (background, key, functionKey, text)
    (self.secondary, self.accent, self.onAccent) = (secondary, accent, onAccent)
  }

  init?(_ value: Any?) {
    guard let value = value as? [String: Any],
          let background = ThemeColor.parse(value["background"]), let key = ThemeColor.parse(value["key"]),
          let functionKey = ThemeColor.parse(value["function_key"]), let text = ThemeColor.parse(value["text"]),
          let secondary = ThemeColor.parse(value["secondary"]), let accent = ThemeColor.parse(value["accent"]),
          let onAccent = ThemeColor.parse(value["on_accent"]) else { return nil }
    (self.background, self.key, self.functionKey, self.text) = (background, key, functionKey, text)
    (self.secondary, self.accent, self.onAccent) = (secondary, accent, onAccent)
  }
}

/// The candidate slots of a resolved theme (`CandidateThemePalette`); a nil slot is the native token.
struct ThemeCandidatePalette: Equatable {
  var surface, border, text, number, secondary, accent, selected, selectedText, selectedNumber, hover: UIColor?
  var showSelectedBar: Bool?

  init?(_ value: Any?) {
    guard let value = value as? [String: Any] else { return nil }
    surface = ThemeColor.parse(value["surface"])
    border = ThemeColor.parse(value["border"])
    text = ThemeColor.parse(value["text"])
    number = ThemeColor.parse(value["number"])
    secondary = ThemeColor.parse(value["secondary"])
    accent = ThemeColor.parse(value["accent"])
    selected = ThemeColor.parse(value["selected"])
    selectedText = ThemeColor.parse(value["selected_text"])
    selectedNumber = ThemeColor.parse(value["selected_number"])
    hover = ThemeColor.parse(value["hover"])
    showSelectedBar = value["show_selected_bar"] as? Bool
  }
}

private func themeAppearance(_ value: Any?) -> UIUserInterfaceStyle? {
  switch value as? String {
  case "dark": .dark
  case "light": .light
  default: nil
  }
}

/// One theme of `msime_client_theme_catalog`, in picker order. The ids, titles and colours come from client-core; the host keeps no copy.
struct GlobalThemeEntry: Equatable {
  struct Preview: Equatable { let background, panel, accent, text: UIColor }
  let id: String
  let title: String
  /// Nil for `system` and `custom`, which follow the mode.
  let appearance: UIUserInterfaceStyle?
  let preview: Preview?
  let keyboard: ThemeKeyboardPalette?
}

enum GlobalThemeCatalog {
  /// 有自己规则的三个 id：`system`（跟随系统）画季节配色，`native`（原生）画系统键盘的 token，`custom` 叠在一个底上。其余 id 都是内置主题。
  static let systemId = "system"
  static let nativeId = "native"
  static let customId = "custom"

  static let entries: [GlobalThemeEntry] = {
    guard let value = themeReply(msimeThemeCatalog()) as? [String: Any],
          let themes = value["themes"] as? [[String: Any]] else { return [] }
    return themes.compactMap { theme in
      guard let id = theme["id"] as? String, let title = theme["title"] as? String else { return nil }
      let preview = (theme["preview"] as? [String: Any]).flatMap { preview -> GlobalThemeEntry.Preview? in
        guard let background = ThemeColor.parse(preview["background"]), let panel = ThemeColor.parse(preview["panel"]),
              let accent = ThemeColor.parse(preview["accent"]), let text = ThemeColor.parse(preview["text"]) else { return nil }
        return GlobalThemeEntry.Preview(background: background, panel: panel, accent: accent, text: text)
      }
      return GlobalThemeEntry(id: id, title: title, appearance: themeAppearance(theme["appearance"]),
                              preview: preview, keyboard: ThemeKeyboardPalette(theme["keyboard"]))
    }
  }()

  static var ids: [String] { entries.map(\.id) }
  static func entry(_ id: String) -> GlobalThemeEntry? { entries.first { $0.id == id } }
  static func title(_ id: String) -> String { entry(id)?.title ?? id }
  static func contains(_ id: String?) -> Bool { id.map { entry($0) != nil } ?? false }
  /// 自定义主题能画在其上的主题：`system` 或内置主题；`custom` 和只在部分宿主上提供的 `native` 不行（client-core `GlobalTheme::is_base`）。
  static func isBase(_ id: String?) -> Bool { contains(id) && id != customId && id != nativeId }
}

/// 候选皮肤包放进自定义主题的哪个槽位：它清单 `base` 的明暗（client-core `theme::skin_appearance`）。包也只在这个明暗下绘制。
enum CandidateSkinSlot: Equatable {
  case light
  case dark
  /// `system` 底的包两种模式都画，两个槽位都放。
  case both

  init(base: String) {
    switch GlobalThemeCatalog.entry(base)?.appearance {
    case .dark: self = .dark
    case .light: self = .light
    default: self = .both
    }
  }
}

/// `msime_client_resolve_theme`'s answer: the palettes a host draws for a theme in one mode.
struct ResolvedTheme: Equatable {
  let id: String
  let source: String
  /// The fixed mode of the theme's surfaces; nil follows the host's mode.
  let appearance: UIUserInterfaceStyle?
  let candidate: ThemeCandidatePalette?
  let keyboard: ThemeKeyboardPalette?
  let candidateSkin: String?

  /// The native tokens, which is also what a refused request draws.
  static let system = ResolvedTheme(id: GlobalThemeCatalog.systemId, source: "system", appearance: nil,
                                    candidate: nil, keyboard: nil, candidateSkin: nil)

  private static let lock = NSLock()
  private static var cache: [Data: ResolvedTheme] = [:]

  /// Theme responses can retain custom skin metadata and photo bytes. A keyboard extension has a
  /// much smaller memory budget than the containing app, so discard reusable resolutions when UIKit
  /// reports pressure; the next appearance resolves the active theme again.
  static func clearCacheForMemoryPressure() {
    lock.lock()
    cache.removeAll(keepingCapacity: false)
    lock.unlock()
  }

  /// Resolve `globalTheme` over `customTheme` (the document's `custom_theme` object) for the horizontal strip. Reads the package from disk when the custom theme names one, so call it when the theme, the mode or the package changes and keep the answer, never while drawing.
  static func resolve(globalTheme: String, customTheme: [String: Any]?, dark: Bool,
                      skinsRoot: URL? = ExternalCandidateSkin.defaultRoot) -> ResolvedTheme? {
    var request: [String: Any] = ["global_theme": globalTheme, "dark": dark, "layout": "horizontal"]
    if let customTheme, !customTheme.isEmpty { request["custom_theme"] = customTheme }
    if let skinsRoot { request["skins_directory"] = skinsRoot.path }
    guard JSONSerialization.isValidJSONObject(request),
          let body = try? JSONSerialization.data(withJSONObject: request, options: [.sortedKeys]) else { return nil }
    // 磁盘上的皮肤包也是答案的一部分，所以两个槽位指名的包，其清单的修改时间和大小都进缓存键：只看 `candidate_skin` 的话，替换了深色槽位的包，深色模式还会拿到旧答案。
    var key = body
    if let skinsRoot {
      for package in GlobalThemePreference.candidateSkins(in: customTheme ?? [:]) {
        let manifest = skinsRoot.appendingPathComponent(package, isDirectory: true).appendingPathComponent("skin.toml")
        let attributes = try? FileManager.default.attributesOfItem(atPath: manifest.path)
        key.append(Data("\u{0}\(package)\u{0}\((attributes?[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0)\u{0}\(attributes?[.size] ?? 0)".utf8))
      }
    }
    lock.lock()
    let cached = cache[key]
    lock.unlock()
    if let cached { return cached }
    let raw = body.withUnsafeBytes { bytes in
      msimeResolveTheme(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(bytes.count))
    }
    guard let value = themeReply(raw) as? [String: Any], let id = value["id"] as? String,
          let source = value["source"] as? String else { return nil }
    let resolved = ResolvedTheme(id: id, source: source, appearance: themeAppearance(value["appearance"]),
                                 candidate: ThemeCandidatePalette(value["candidate"]),
                                 keyboard: ThemeKeyboardPalette(value["keyboard"]),
                                 candidateSkin: value["candidate_skin"] as? String)
    lock.lock()
    // A handful of themes and modes are in play at once; a design photo makes a key large, so the cache stays small.
    if cache.count >= 16 { cache.removeAll() }
    cache[key] = resolved
    lock.unlock()
    return resolved
  }
}

/// `global_theme` and `custom_theme` in the shared document, with the App Group copy the keyboard reads before the document loads.
///
/// Tauri 插件写 App Group 的 `globalTheme`（八个主题 id 之一）和 `customKeyboardSkin.v1`（键盘设计，没有时不存在）；键盘每次重新加载都用文档覆盖这两项。这里的写入先改文档，文档接受了改动才改 App Group。
enum GlobalThemePreference {
  static let key = "globalTheme"
  /// 自定义主题的两个候选皮肤槽位：`candidate_skin` 是浅色模式的，也是深色槽位没设时深色模式用的；`candidate_skin_dark` 是深色模式的（client-core `CustomTheme::candidate_skin_for`）。
  static let candidateSkinKey = "candidate_skin"
  static let candidateSkinDarkKey = "candidate_skin_dark"

  /// 槽位 `key` 里的皮肤包 id，空字符串和缺省一样按没设。
  private static func skin(_ custom: [String: Any], _ key: String) -> String? {
    (custom[key] as? String).flatMap { $0.isEmpty ? nil : $0 }
  }

  /// `dark` 模式下画哪款皮肤包：深色模式先取 `candidate_skin_dark`，没设时与浅色模式一样取 `candidate_skin`。包画不画还要看它的底是否属于这个模式，由 `msime_client_resolve_theme` 判断。
  static func candidateSkin(in custom: [String: Any], dark: Bool) -> String? {
    let light = skin(custom, candidateSkinKey)
    return dark ? skin(custom, candidateSkinDarkKey) ?? light : light
  }

  /// 两个槽位指名的皮肤包，浅色槽位在前，同一款只出现一次。
  static func candidateSkins(in custom: [String: Any]) -> [String] {
    var skins: [String] = []
    for key in [candidateSkinKey, candidateSkinDarkKey] {
      if let id = skin(custom, key), !skins.contains(id) { skins.append(id) }
    }
    return skins
  }

  /// The App Group copy; an id the catalog does not list reads as the default.
  static var selected: String {
    let stored = KeyboardFeedbackPreference.defaults.string(forKey: key)
    return GlobalThemeCatalog.contains(stored) ? stored! : GlobalThemeCatalog.systemId
  }

  static func theme(in document: [String: Any]?) -> String {
    let id = document?["global_theme"] as? String
    return GlobalThemeCatalog.contains(id) ? id! : GlobalThemeCatalog.systemId
  }

  static func customTheme(in document: [String: Any]?) -> [String: Any] {
    document?["custom_theme"] as? [String: Any] ?? [:]
  }

  static func base(in document: [String: Any]?) -> String {
    let base = customTheme(in: document)["base"] as? String
    return GlobalThemeCatalog.isBase(base) ? base! : GlobalThemeCatalog.systemId
  }

  /// The keyboard design `custom_theme.keyboard` holds, or nil when there is none.
  static func design(in document: [String: Any]?) -> CustomKeyboardSkin? {
    guard let value = customTheme(in: document)["keyboard"] as? [String: Any],
          JSONSerialization.isValidJSONObject(value),
          let data = try? JSONSerialization.data(withJSONObject: value) else { return nil }
    return (try? JSONDecoder().decode(CustomKeyboardSkin.self, from: data))?.normalized
  }

  /// Select a theme as it stands; choosing `custom` keeps its base, package, pickers and design.
  static func selecting(_ id: String) -> (inout [String: Any]) -> Void {
    { $0["global_theme"] = id }
  }

  /// Move to `custom` for an edit to it. When another theme was on screen it becomes the base and the package is cleared, so what the user was looking at stays underneath the edit; while `custom` is already selected only the edit changes (THEME_CONTRACT section 5).
  static func customizing(_ edit: @escaping (inout [String: Any]) -> Void) -> (inout [String: Any]) -> Void {
    { document in
      var custom = customTheme(in: document)
      let current = theme(in: document)
      if current != GlobalThemeCatalog.customId {
        // `native` 不能当底，从它开始自定义时和跟随系统一样不写底。
        if !GlobalThemeCatalog.isBase(current) || current == GlobalThemeCatalog.systemId { custom.removeValue(forKey: "base") } else { custom["base"] = current }
        // 两个槽位一起清：只清 `candidate_skin` 会让深色槽位的皮肤留在新的自定义主题里，深色模式继续画它。
        custom.removeValue(forKey: candidateSkinKey)
        custom.removeValue(forKey: candidateSkinDarkKey)
      }
      edit(&custom)
      document["custom_theme"] = custom
      document["global_theme"] = GlobalThemeCatalog.customId
    }
  }

  /// Apply a keyboard design (the editor's 使用, a community, saved or AI design, or a trial); nil when the design cannot be encoded.
  static func applyingDesign(_ design: CustomKeyboardSkin) -> ((inout [String: Any]) -> Void)? {
    guard let value = CustomKeyboardSkin.documentValue(design) else { return nil }
    return customizing { $0["keyboard"] = value }
  }

  /// Keep an edited design in the custom theme without selecting it: the editor saves as the user drags, and only 使用皮肤 moves to the custom theme (`applyingDesign`). Nil when the design cannot be encoded.
  static func storingDesign(_ design: CustomKeyboardSkin) -> ((inout [String: Any]) -> Void)? {
    guard let value = CustomKeyboardSkin.documentValue(design) else { return nil }
    return { document in
      var custom = customTheme(in: document)
      custom["keyboard"] = value
      document["custom_theme"] = custom
    }
  }

  /// Pick one candidate colour (`custom_theme.candidate_colors`), `#RRGGBB`.
  static func pickingCandidateColor(_ slot: String, hex: String) -> (inout [String: Any]) -> Void {
    customizing { custom in
      var colors = custom["candidate_colors"] as? [String: Any] ?? [:]
      colors[slot] = hex
      custom["candidate_colors"] = colors
    }
  }

  /// Clear the candidate colour pickers this app edits (`CandidatePalette.editableColors`), leaving the selection and the slots only other hosts edit (number, accent, selected, border) as they are.
  static func clearingCandidateColors(_ document: inout [String: Any]) {
    var custom = customTheme(in: document)
    guard var colors = custom["candidate_colors"] as? [String: Any] else { return }
    CandidatePalette.editableColors.forEach { colors.removeValue(forKey: $0.slot) }
    if colors.isEmpty { custom.removeValue(forKey: "candidate_colors") } else { custom["candidate_colors"] = colors }
    document["custom_theme"] = custom
  }

  /// 选用一款导入的皮肤包：自定义主题画在包自己的底上（THEME_CONTRACT section 5），包放进它所属的槽位，与共享设置页的 `applyCandidateSkin` 同一条规则。深色皮肤写 `candidate_skin_dark`，浅色皮肤写 `candidate_skin`，`system` 底的两个都写。
  ///
  /// 写深色槽位时，原来放在 `candidate_skin` 里的深色皮肤一并清掉。写浅色槽位时，如果深色槽位还空着、原来的 `candidate_skin` 是深色或跟随系统的皮肤，先把它挪到深色槽位：只设过一款深色皮肤的旧文档把它存在 `candidate_skin` 里，深色模式靠回退取到它，直接覆盖会让它悄悄消失。`slotOf` 给出已装皮肤的槽位，nil 表示不知道（比如包已经不在了）：这种皮肤哪个模式都画不出来，直接覆盖，不挪进深色槽位。
  static func applyingPackage(_ id: String, base: String,
                              slotOf: @escaping (String) -> CandidateSkinSlot?) -> (inout [String: Any]) -> Void {
    { document in
      var custom = customTheme(in: document)
      let slot = CandidateSkinSlot(base: base)
      let previous = skin(custom, candidateSkinKey)
      let previousDark = skin(custom, candidateSkinDarkKey)
      if slot != .light { custom[candidateSkinDarkKey] = id }
      // 新的深色皮肤取代旧文档放在 `candidate_skin` 里的深色皮肤：浅色模式本来就不画它，留着只会让它看起来还在用。
      if slot == .dark, let previous, slotOf(previous) == .dark { custom.removeValue(forKey: candidateSkinKey) }
      if slot != .dark {
        if slot == .light, previousDark == nil, let previous, let previousSlot = slotOf(previous), previousSlot != .light {
          custom[candidateSkinDarkKey] = previous
        }
        custom[candidateSkinKey] = id
      }
      if GlobalThemeCatalog.isBase(base), base != GlobalThemeCatalog.systemId { custom["base"] = base } else { custom.removeValue(forKey: "base") }
      document["custom_theme"] = custom
      document["global_theme"] = GlobalThemeCatalog.customId
    }
  }

  /// 不再使用皮肤包 `id`：只清掉放着它的槽位，另一个槽位、取色器和键盘设计都不动（共享设置页的 `removeCandidateSkin`）。取下的是最后一款皮肤时底改回跟随系统（不写 `base`）：底是应用皮肤时写进来的包 base，两个槽位都空以后 client-core 在两种明暗下都用它，留着深色皮肤的 `night` 会让浅色模式变成深色。
  static func removingPackage(_ id: String) -> (inout [String: Any]) -> Void {
    { document in
      var custom = customTheme(in: document)
      var changed = false
      for key in [candidateSkinKey, candidateSkinDarkKey] where custom[key] as? String == id {
        custom.removeValue(forKey: key)
        changed = true
      }
      guard changed else { return }
      if candidateSkins(in: custom).isEmpty { custom.removeValue(forKey: "base") }
      document["custom_theme"] = custom
    }
  }

  /// Change the theme fields of the shared document, then copy what the document now holds to the App Group; false when the document refused the change, which leaves the App Group alone.
  @discardableResult
  static func update(stateRoot: URL? = nil, _ mapping: (inout [String: Any]) -> Void) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, mapping) else { return false }
    if let document = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: stateRoot) {
      mirror(document)
      KeyboardTheme.reload(document)
    }
    return true
  }

  /// Select a theme as it stands.
  @discardableResult
  static func save(_ id: String, stateRoot: URL? = nil) -> Bool {
    update(stateRoot: stateRoot, selecting(id))
  }

  /// Apply a keyboard design to the custom theme and select it.
  @discardableResult
  static func apply(_ design: CustomKeyboardSkin, stateRoot: URL? = nil) -> Bool {
    guard let mapping = applyingDesign(design) else { return false }
    return update(stateRoot: stateRoot, mapping)
  }

  /// Copy the document's theme and design over the App Group, as the plugin does; true when either changed.
  @discardableResult
  static func mirror(_ document: [String: Any]) -> Bool {
    var changed = false
    if let id = document["global_theme"] as? String, GlobalThemeCatalog.contains(id), id != selected {
      KeyboardFeedbackPreference.defaults.set(id, forKey: key)
      changed = true
    }
    let design = design(in: document)
    let stored = KeyboardFeedbackPreference.defaults.data(forKey: CustomKeyboardSkinStore.key)
    if let design {
      if stored == nil || design != CustomKeyboardSkinStore.current {
        CustomKeyboardSkinStore.save(design)
        changed = true
      }
    } else if stored != nil {
      KeyboardFeedbackPreference.defaults.removeObject(forKey: CustomKeyboardSkinStore.key)
      changed = true
    }
    return changed
  }
}

extension CustomKeyboardSkin {
  /// The design as the shared document's `custom_theme.keyboard` holds it: the same camelCase fields, the photo as base64.
  static func documentValue(_ design: CustomKeyboardSkin) -> [String: Any]? {
    guard let data = try? JSONEncoder().encode(design.normalized) else { return nil }
    return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
  }
}

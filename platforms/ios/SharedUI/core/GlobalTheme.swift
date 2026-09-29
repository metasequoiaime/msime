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

/// UIKit's own keyboard tokens from the design (dc.html L1555-1558), drawn for the `system` theme and wherever a resolved slot is null.
enum NativeKeyboardTokens {
  private static func adaptive(_ light: UInt32, _ dark: UInt32, lightAlpha: CGFloat = 1, darkAlpha: CGFloat = 1) -> UIColor {
    UIColor { traits in
      let isDark = traits.userInterfaceStyle == .dark
      let value = isDark ? dark : light
      return UIColor(red: CGFloat(value >> 16 & 0xff) / 255, green: CGFloat(value >> 8 & 0xff) / 255,
                     blue: CGFloat(value & 0xff) / 255, alpha: isDark ? darkAlpha : lightAlpha)
    }
  }

  static let background = adaptive(0xD1D4DB, 0x2B2B2D)
  static let key = adaptive(0xFFFFFF, 0x6B6B6E)
  static let functionKey = adaptive(0xABB0BB, 0x464648)
  static let text = adaptive(0x000000, 0xFFFFFF)
  static let secondary = adaptive(0x8A8A8E, 0x8E8E93)
  /// The brand accent: the return key while composing, the selected candidate, switched-on tiles and pills.
  static let accent = adaptive(0x2C7A4B, 0x5FBF84)
  static let accentSoft = adaptive(0x2C7A4B, 0x5FBF84, lightAlpha: 0.14, darkAlpha: 0.26)
  /// The key's bottom edge, `0 1px 0 rgba(0,0,0,.3)` light and `.6` dark.
  static let keyShadowOpacity: (light: Float, dark: Float) = (0.3, 0.6)
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
  /// The two ids with their own rules in THEME_CONTRACT section 5: `system` draws the native tokens and `custom` is layered over a base. Every other id is a built-in theme.
  static let systemId = "system"
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
  /// What a custom theme may be drawn over: `system` or a built-in theme.
  static func isBase(_ id: String?) -> Bool { contains(id) && id != customId }
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

  /// Resolve `globalTheme` over `customTheme` (the document's `custom_theme` object) for the horizontal strip. Reads the package from disk when the custom theme names one, so call it when the theme, the mode or the package changes and keep the answer, never while drawing.
  static func resolve(globalTheme: String, customTheme: [String: Any]?, dark: Bool,
                      skinsRoot: URL? = ExternalCandidateSkin.defaultRoot) -> ResolvedTheme? {
    var request: [String: Any] = ["global_theme": globalTheme, "dark": dark, "layout": "horizontal"]
    if let customTheme, !customTheme.isEmpty { request["custom_theme"] = customTheme }
    if let skinsRoot { request["skins_directory"] = skinsRoot.path }
    guard JSONSerialization.isValidJSONObject(request),
          let body = try? JSONSerialization.data(withJSONObject: request, options: [.sortedKeys]) else { return nil }
    // The package on disk is part of the answer, so its manifest's change time is part of the key.
    var key = body
    if let package = customTheme?["candidate_skin"] as? String, let skinsRoot,
       let attributes = try? FileManager.default.attributesOfItem(
         atPath: skinsRoot.appendingPathComponent(package, isDirectory: true).appendingPathComponent("skin.toml").path) {
      key.append(Data("\u{0}\((attributes[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0)\u{0}\(attributes[.size] ?? 0)".utf8))
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
/// The Tauri plugin writes the App Group `globalTheme` (one of the seven ids) and `customKeyboardSkin.v1` (the design, absent when there is none); the keyboard copies the document over both on every reload. Writers here change the document first and the App Group only when the document took the change.
enum GlobalThemePreference {
  static let key = "globalTheme"

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
        if current == GlobalThemeCatalog.systemId { custom.removeValue(forKey: "base") } else { custom["base"] = current }
        custom.removeValue(forKey: "candidate_skin")
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

  /// Draw an imported package: the custom theme over the package's own base, with this package (THEME_CONTRACT section 5).
  static func applyingPackage(_ id: String, base: String) -> (inout [String: Any]) -> Void {
    { document in
      var custom = customTheme(in: document)
      custom["candidate_skin"] = id
      if GlobalThemeCatalog.isBase(base), base != GlobalThemeCatalog.systemId { custom["base"] = base } else { custom.removeValue(forKey: "base") }
      document["custom_theme"] = custom
      document["global_theme"] = GlobalThemeCatalog.customId
    }
  }

  /// 自定义主题不使用外部皮肤: the custom theme keeps its base, pickers and design.
  static func clearingPackage(_ document: inout [String: Any]) {
    var custom = customTheme(in: document)
    guard custom.removeValue(forKey: "candidate_skin") != nil else { return }
    document["custom_theme"] = custom
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

import Foundation
import UIKit

@_silgen_name("msime_client_app_theme_catalog")
private func msimeAppThemeCatalog() -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_resolve_app_theme")
private func msimeResolveAppTheme(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_string_free")
private func msimeAppThemeStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// ABI 信封 `{"ok": true, "value": …}`；出错或没有回复时返回 nil。
private func appThemeReply(_ raw: UnsafeMutablePointer<CChar>?) -> Any? {
  guard let raw else { return nil }
  defer { msimeAppThemeStringFree(raw) }
  guard let reply = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any],
        reply["ok"] as? Bool == true else { return nil }
  return reply["value"]
}

/// 应用主题（水杉四季 / 春芽 / 夏荫 / 秋杉 / 冬雪）：宿主界面和跟随系统的键盘皮肤用的季节色。
///
/// 季节、种子色和按月份取季的规则都在 Rust 里（`msime_client_resolve_app_theme`、`msime_client_app_theme_catalog`）；这个类型只保存用户的选择，缓存当月浅色和深色两套解析结果，并移植 Android 的 `color-mix`，让派生色与设计稿逐位一致。选择存在 App Group 里，键盘扩展因此画出同一个季节；iOS 的设置同步目前不携带它（Android 经 client-core 的 `android_local` 同步 `general.app_theme`），换设备要重选。解析失败时所有读取方都退回经典绿色 token，所以库缺失或损坏也不会让任何颜色落空。
enum AppThemePalette {
  /// 解析后主题的一种模式：`{id, season, accent, accent_soft, on_accent, background, card, hair}`。
  struct Resolved: Equatable {
    var id: String
    /// 实际绘制的季节：`spring`、`summer`、`autumn` 或 `winter`。
    var season: String
    var accent: UIColor
    /// 透明度为 `22`（浅色）或 `40`（深色）的主色。
    var accentSoft: UIColor
    /// 主色填充上的文字色：浅色模式为白色，深色模式为 `mix(accent 25%, #000)`。
    var onAccent: UIColor
    var background: UIColor
    var card: UIColor
    /// Rust 给出的分隔线颜色；深色模式下由 `MetasequoiaTheme.hair` 覆盖。
    var hair: UIColor
  }

  /// 选择器里的一个主题，按选择器顺序排列。
  struct CatalogEntry: Identifiable {
    var id: String
    var title: String
    /// 水杉四季为 true，它的季节随月份变化。
    var seasonal: Bool
    var lightAccent: UIColor
    var darkAccent: UIColor
  }

  static let storageKey = "general.app_theme"
  static let defaultID = "siji"
  static let didChange = Notification.Name("AppThemePalette.didChange")

  private struct State {
    var loaded = false
    var light: Resolved?
    var dark: Resolved?
    var version = 0
    var catalog: [CatalogEntry]?
  }

  private static let lock = NSLock()
  private static var state = State()

  private static var defaults: UserDefaults? { UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier) }

  /// 绘制的主题或季节变化时递增；以它为 key 的视图会重建，让动态颜色重新解析。
  static var version: Int { lock.withLock { state.version } }

  /// 已保存的主题 id，没有保存时为 `siji`。
  static var selectedID: String {
    guard let stored = defaults?.string(forKey: storageKey), !stored.isEmpty else { return defaultID }
    return stored
  }

  /// 某一模式的颜色，首次使用时按当月解析；库无法解析主题时为 nil，调用方改画经典 token。
  static func resolved(dark: Bool) -> Resolved? {
    lock.lock()
    defer { lock.unlock() }
    if !state.loaded {
      let pair = resolvePair(month: currentMonth())
      state.light = pair?.light
      state.dark = pair?.dark
      state.loaded = true
    }
    return dark ? state.dark : state.light
  }

  /// 按 `month` 解析已保存主题的两种模式。主题或季节与已绘制的不同时，递增 `version` 并在主线程发出 `didChange`；场景每次激活都会调用它，让水杉四季随月份换季。
  static func refresh(month: Int = Calendar.current.component(.month, from: Date())) {
    let pair = resolvePair(month: month)
    let changed: Bool = lock.withLock {
      let wasLoaded = state.loaded
      let before = state.light
      state.light = pair?.light
      state.dark = pair?.dark
      state.loaded = true
      guard wasLoaded, before?.id != pair?.light.id || before?.season != pair?.light.season else { return false }
      state.version += 1
      return true
    }
    guard changed else { return }
    if Thread.isMainThread {
      NotificationCenter.default.post(name: didChange, object: nil)
    } else {
      DispatchQueue.main.async { NotificationCenter.default.post(name: didChange, object: nil) }
    }
  }

  /// 保存所选主题，并重绘所有跟随它的界面。
  static func select(_ id: String) {
    defaults?.set(id, forKey: storageKey)
    refresh()
  }

  /// 清除所选主题，让应用和键盘回到水杉四季，供重置所有设置使用。
  static func resetToDefault() {
    defaults?.removeObject(forKey: storageKey)
    refresh()
  }

  /// 来自 client-core 的选择器条目，id、标题和颜色都由它管理。
  static func catalog() -> [CatalogEntry] {
    lock.lock()
    defer { lock.unlock() }
    if let cached = state.catalog { return cached }
    guard let value = appThemeReply(msimeAppThemeCatalog()) as? [String: Any],
          let themes = value["app_themes"] as? [[String: Any]] else { return [] }
    let entries = themes.compactMap { theme -> CatalogEntry? in
      guard let id = theme["id"] as? String, let title = theme["title"] as? String,
            let light = theme["light"] as? [String: Any], let dark = theme["dark"] as? [String: Any],
            let lightAccent = (light["accent"] as? String).flatMap(hexColor),
            let darkAccent = (dark["accent"] as? String).flatMap(hexColor) else { return nil }
      return CatalogEntry(id: id, title: title, seasonal: theme["seasonal"] as? Bool ?? false,
                          lightAccent: lightAccent, darkAccent: darkAccent)
    }
    state.catalog = entries
    return entries
  }

  /// 选择器里显示的季节中文名。
  static func seasonTitle(_ season: String) -> String {
    switch season {
    case "spring": return "春芽"
    case "summer": return "夏荫"
    case "autumn": return "秋杉"
    case "winter": return "冬雪"
    default: return season
    }
  }

  /// 契约颜色，`#RRGGBB` 或末尾带透明度的 `#RRGGBBAA`；其他格式一律为 nil。
  static func hexColor(_ s: String) -> UIColor? {
    guard s.first == "#", s.count == 7 || s.count == 9, s.dropFirst().allSatisfy(\.isHexDigit),
          let bits = UInt32(s.dropFirst(), radix: 16) else { return nil }
    let rgba = s.count == 7 ? bits << 8 | 0xff : bits
    return UIColor(red: CGFloat(rgba >> 24 & 0xff) / 255, green: CGFloat(rgba >> 16 & 0xff) / 255,
                   blue: CGFloat(rgba >> 8 & 0xff) / 255, alpha: CGFloat(rgba & 0xff) / 255)
  }

  /// CSS `color-mix(in srgb, a percentA%, b)`，移植自 Android 的 `AppThemePalette.mix`：包括透明度在内的每个通道都在 0–255 范围上线性混合，再按 Chrome 打印设计稿 token 表的方式量化（0–1 通道值先取 6 位有效数字，再乘 255 后四舍五入）。直接取整会让恰好落在 .5 上的通道和设计稿差一级。
  static func mix(_ a: UIColor, _ percentA: Double, _ b: UIColor) -> UIColor {
    let weight = SharedNumber.clamped(percentA, to: 0...100)
    let first = channels(a), second = channels(b)
    var mixed = [CGFloat](repeating: 0, count: 4)
    for index in 0..<4 {
      let exact = (first[index] * weight + second[index] * (100 - weight)) / 100
      mixed[index] = CGFloat(quantize(exact)) / 255
    }
    return UIColor(red: mixed[0], green: mixed[1], blue: mixed[2], alpha: mixed[3])
  }

  // MARK: - Private

  private static func currentMonth() -> Int { Calendar.current.component(.month, from: Date()) }

  /// 已保存主题的两种模式，保存的 id 不再能解析时退回默认主题；两者都解析不了，或两次回复的主题或季节不一致时为 nil。
  private static func resolvePair(month: Int) -> (light: Resolved, dark: Resolved)? {
    let id = selectedID
    if let pair = resolvePair(id: id, month: month) { return pair }
    return id == defaultID ? nil : resolvePair(id: defaultID, month: month)
  }

  private static func resolvePair(id: String, month: Int) -> (light: Resolved, dark: Resolved)? {
    guard let light = resolve(id: id, month: month, dark: false),
          let dark = resolve(id: id, month: month, dark: true),
          light.id == dark.id, light.season == dark.season else { return nil }
    return (light, dark)
  }

  private static func resolve(id: String, month: Int, dark: Bool) -> Resolved? {
    let request: [String: Any] = ["app_theme": id, "month": month, "dark": dark]
    guard let body = try? JSONSerialization.data(withJSONObject: request) else { return nil }
    let raw = body.withUnsafeBytes { bytes in
      msimeResolveAppTheme(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(bytes.count))
    }
    guard let value = appThemeReply(raw) as? [String: Any],
          let resolvedID = value["id"] as? String, let season = value["season"] as? String else { return nil }
    func color(_ key: String) -> UIColor? { (value[key] as? String).flatMap(hexColor) }
    guard let accent = color("accent"), let accentSoft = color("accent_soft"), let onAccent = color("on_accent"),
          let background = color("background"), let card = color("card"), let hair = color("hair") else { return nil }
    return Resolved(id: resolvedID, season: season, accent: accent, accentSoft: accentSoft, onAccent: onAccent,
                    background: background, card: card, hair: hair)
  }

  /// 0–255 范围上的 R、G、B、A，各自取整到整数级，与 Android 打包成 int 的颜色一致。
  private static func channels(_ color: UIColor) -> [Double] {
    var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
    color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
    return [red, green, blue, alpha].map { (SharedNumber.clamped(Double($0), to: 0...1) * 255).rounded() }
  }

  /// 按 Chrome 打印方式得到的一个 0–255 通道：0–1 值取 6 位有效数字，再乘 255 后四舍五入。`%.5e` 对精确的二进制值取整，与 Java 的 `BigDecimal(double).round(MathContext(6))` 一致。
  private static func quantize(_ channel: Double) -> Int {
    let unit = channel / 255
    let printed = Double(String(format: "%.5e", unit)) ?? unit
    return SharedNumber.clamped(Int((printed * 255).rounded(.toNearestOrAwayFromZero)), to: 0...255)
  }
}

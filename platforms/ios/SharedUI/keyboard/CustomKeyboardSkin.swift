import UIKit
import ImageIO
import Darwin

enum SkinKeyShape: String, Codable, CaseIterable, Sendable {
  case rounded, capsule, ticket, pebble
  var title: String { switch self { case .rounded: return "圆角"; case .capsule: return "胶囊"; case .ticket: return "票券"; case .pebble: return "卵石" } }
}
enum SkinKeyMaterial: String, Codable, CaseIterable, Sendable {
  case flat, raised, glass, paper
  var title: String { switch self { case .flat: return "哑光"; case .raised: return "立体"; case .glass: return "玻璃"; case .paper: return "纸张" } }
}

struct CustomKeyboardSkin: Codable, Equatable, Hashable, Sendable {
  var background: UInt32 = 0xE8F0EB
  var keyBackground: UInt32 = 0xFFFFFF
  var keyForeground: UInt32 = 0x17251D
  var accent: UInt32 = 0x185C47
  var actionBackground: UInt32 = 0x185C47
  var cornerRadius: Double = 8
  var borderWidth: Double = 0
  var shadow: Double = 0
  var pattern: Int = 0
  var monospaced = false
  // Optional fields preserve decoding of existing v1 designs.
  var keyShape: SkinKeyShape?
  var keyMaterial: SkinKeyMaterial?
  var keyOpacity: Double?
  var gradientEnd: UInt32?
  var gradientHorizontal: Bool?
  var patternOpacity: Double?
  var customBorderColor: UInt32?
  var photo: Data?
  var photoShade: Double?
  var photoPosition: Double?


  var normalized: Self {
    var result = self
    result.background &= 0xFFFFFF
    result.keyBackground &= 0xFFFFFF
    result.keyForeground &= 0xFFFFFF
    result.accent &= 0xFFFFFF
    result.actionBackground &= 0xFFFFFF
    result.cornerRadius = cornerRadius.isFinite ? min(20, max(0, cornerRadius)) : 8
    result.borderWidth = borderWidth.isFinite ? min(2, max(0, borderWidth)) : 0
    result.shadow = shadow.isFinite ? min(0.4, max(0, shadow)) : 0
    result.keyOpacity = keyOpacity.map { $0.isFinite ? min(1, max(0.25, $0)) : 1 }
    result.gradientEnd = gradientEnd.map { $0 & 0xFFFFFF }
    result.customBorderColor = customBorderColor.map { $0 & 0xFFFFFF }
    result.patternOpacity = patternOpacity.map { $0.isFinite ? min(0.5, max(0, $0)) : 0.15 }
    result.photoShade = photoShade.map { $0.isFinite ? min(0.8, max(0, $0)) : 0.25 }
    result.photoPosition = photoPosition.map { $0.isFinite ? min(1, max(0, $0)) : 0.5 }
    if let photo, photo.count > 512_000 { result.photo = nil }
    result.pattern = (0...3).contains(pattern) ? pattern : 0
    return result
  }

  static func color(_ rgb: UInt32) -> UIColor {
    UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
            blue: CGFloat(rgb & 255) / 255, alpha: 1)
  }

  static func rgb(_ color: UIColor) -> UInt32 {
    var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
    guard color.getRed(&r, green: &g, blue: &b, alpha: &a) else { return 0 }
    return UInt32((min(1, max(0, r)) * 255).rounded()) << 16
      | UInt32((min(1, max(0, g)) * 255).rounded()) << 8
      | UInt32((min(1, max(0, b)) * 255).rounded())
  }

  static func luminance(_ rgb: UInt32) -> Double {
    func channel(_ value: UInt32) -> Double {
      let c = Double(value & 255) / 255
      return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4)
    }
    return 0.2126 * channel(rgb >> 16) + 0.7152 * channel(rgb >> 8) + 0.0722 * channel(rgb)
  }

  static func readableText(on rgb: UInt32) -> UInt32 {
    luminance(rgb) > 0.179 ? 0x000000 : 0xFFFFFF
  }

  static func contrast(_ first: UInt32, _ second: UInt32) -> Double {
    let a = luminance(first), b = luminance(second)
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)
  }

  var hasReadableText: Bool {
    Self.contrast(keyForeground, keyBackground) >= 4.5
      && Self.contrast(accent, background) >= 4.5
      && Self.contrast(accent, keyBackground) >= 4.5
      && (gradientEnd.map { Self.contrast(accent, $0) >= 4.5 } ?? true)
  }
}

enum CustomKeyboardSkinStore {
  static let key = "customKeyboardSkin.v1"
  private static let cache = Cache()
  static var current: CustomKeyboardSkin { cache.load() }
  /// The saved design, or nil when there is none: `current` falls back to the editor's starting design, which is not one the user applied.
  static var stored: CustomKeyboardSkin? {
    KeyboardFeedbackPreference.defaults.data(forKey: key) == nil ? nil : cache.load()
  }
  static func save(_ skin: CustomKeyboardSkin) {
    guard let data = try? JSONEncoder().encode(skin.normalized) else { return }
    KeyboardFeedbackPreference.defaults.set(data, forKey: key)
  }

  private final class Cache: @unchecked Sendable {
    private let lock = NSLock()
    private var previousData: Data?
    private var value = CustomKeyboardSkin()
    func load() -> CustomKeyboardSkin {
      let data = KeyboardFeedbackPreference.defaults.data(forKey: key)
      lock.lock()
      defer { lock.unlock() }
      if data != previousData {
        previousData = data
        value = data.flatMap { try? JSONDecoder().decode(CustomKeyboardSkin.self, from: $0) }?.normalized
          ?? CustomKeyboardSkin()
      }
      return value
    }
  }
}

struct SavedKeyboardSkin: Codable, Identifiable, Equatable {
  var id = UUID()
  var name: String
  var design: CustomKeyboardSkin
}

enum CustomSkinLibrary {
  // Keep the multi-photo library out of preferences, which the keyboard reads on each key.
  private static func file(in directory: URL? = nil) -> URL {
    let root = directory ?? FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier)
      ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    return root.appendingPathComponent("CustomSkins", isDirectory: true).appendingPathComponent("library.json")
  }
  private static func rejectsSymlinkAncestors(_ path: URL) -> Bool {
    SafePath.hasRefusedSymbolicLink(path)
  }
  static var designs: [SavedKeyboardSkin] { designs(in: nil) }
  static func designs(in directory: URL?) -> [SavedKeyboardSkin] {
    let file = file(in: directory)
    guard !rejectsSymlinkAncestors(file),
          let size = try? file.resourceValues(forKeys: [.fileSizeKey]).fileSize,
          size <= 9_000_000,
          let data = try? BoundedFileReader.read(from: file, maximumBytes: 9_000_000),
          let items = try? JSONDecoder().decode([SavedKeyboardSkin].self, from: data) else { return [] }
    guard items.count <= 12 else { return [] }
    return Array(items.prefix(12)).map { item in
      var item = item
      item.design = item.design.normalized
      return item
    }
  }
  #if DEBUG
  /// Empties the library. Only the UI suite's reset argument calls this: the library lives in the
  /// app group, which survives uninstalling the app, so a test that leaks a skin has no other way
  /// of getting back to a known state.
  static func removeAll() { try? FileManager.default.removeItem(at: file()) }
  #endif
  @discardableResult
  static func save(_ items: [SavedKeyboardSkin], in directory: URL? = nil) -> Bool {
    let file = file(in: directory)
    guard !rejectsSymlinkAncestors(file) else { return false }
    let items = items.prefix(12).map { item in
      var item = item
      item.name = String(item.name.trimmingCharacters(in: .whitespacesAndNewlines).prefix(32))
      item.design = item.design.normalized
      return item
    }
    do {
      let data = try JSONEncoder().encode(items)
      try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
      try data.write(to: file, options: .atomic)
      return true
    } catch { return false }
  }
}

extension CustomKeyboardSkin {
  static var templates: [(String, CustomKeyboardSkin)] {
    var paper = Self()
    paper.background = 0xE3D6BD; paper.keyBackground = 0xFFF5DF; paper.keyForeground = 0x382A1C
    paper.accent = 0x53391F; paper.actionBackground = 0x53391F
    paper.cornerRadius = 4; paper.borderWidth = 1; paper.shadow = 0.3; paper.monospaced = true; paper.pattern = 1
    paper.keyShape = .ticket; paper.keyMaterial = .paper
    var night = Self()
    night.background = 0x151022; night.gradientEnd = 0x30224A; night.keyBackground = 0x291E40
    night.keyForeground = 0xFFFFFF; night.accent = 0xD4BBFF; night.actionBackground = 0x69469B
    night.borderWidth = 1; night.customBorderColor = 0xA987E8; night.pattern = 1
    night.keyShape = .rounded; night.keyMaterial = .glass
    var peach = Self()
    peach.background = 0xFFE0D0; peach.gradientEnd = 0xF9D6E5; peach.keyBackground = 0xFFF8EE
    peach.keyForeground = 0x51283A; peach.accent = 0x84334F; peach.actionBackground = 0x84334F
    peach.cornerRadius = 18; peach.shadow = 0.15; peach.pattern = 3
    peach.keyShape = .pebble; peach.keyMaterial = .raised
    var blue = Self()
    blue.background = 0xDCEAF8; blue.gradientEnd = 0xDDEFE9; blue.gradientHorizontal = true
    blue.accent = 0x224E75; blue.actionBackground = 0x224E75; blue.borderWidth = 0.5
    var grid = night
    grid.background = 0x102438; grid.gradientEnd = nil; grid.keyBackground = 0x17354F
    grid.accent = 0xA2D8FA; grid.actionBackground = 0x285D84; grid.cornerRadius = 2
    grid.monospaced = true; grid.pattern = 2; grid.customBorderColor = 0x548CAA
    return [("水杉留白", Self()), ("复古纸感", paper), ("紫夜星光", night), ("奶油桃桃", peach), ("海盐渐变", blue), ("工程蓝图", grid)] + curatedTemplates
  }
}

// Decode only a bounded thumbnail, even when the chosen original is a large panorama.
enum SkinPhotoData {
  static let maximumSourceBytes = 16 * 1024 * 1024

  static func sourceData(at url: URL) -> Data? {
    try? BoundedFileReader.read(from: url, maximumBytes: maximumSourceBytes)
  }

  static func thumbnail(at url: URL) -> Data? {
    guard let data = sourceData(at: url),
          let source = CGImageSourceCreateWithData(data as CFData, nil),
          let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: 1024,
            kCGImageSourceShouldCacheImmediately: true
          ] as CFDictionary) else { return nil }
    let image = UIImage(cgImage: cg)
    for quality in [0.8, 0.6, 0.4, 0.2] {
      if let data = image.jpegData(compressionQuality: quality), data.count <= 512_000 { return data }
    }
    return nil
  }

  // Synced or downloaded photos skip `thumbnail`; never decode them at full size in the extension.
  static func image(from data: Data, maxPixelSize: Int = 1536) -> UIImage? {
    guard let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
          let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
            kCGImageSourceShouldCacheImmediately: true
          ] as CFDictionary) else { return nil }
    return UIImage(cgImage: cg)
  }
}

import ImageIO
import UIKit
import UniformTypeIdentifiers

/// 表情面板里的水杉娘表情包：随键盘打包的 `stickers/shuishan`，66 张 320px 透明 WebP 加一份 `manifest.json`，Android 和 HarmonyOS 的键盘读的是仓库里同一份。
///
/// 键盘扩展不能把图片直接插进别的应用的输入框，所以点一张是把它转成 PNG 放进系统剪贴板，由用户在输入框里长按粘贴；写剪贴板要求键盘有完全访问。
enum KeyboardStickerCatalog {
  struct Sticker: Equatable, Sendable {
    let url: URL
    /// 这张的含义，例如「开心」；无障碍标签和复制后的提示用它。
    let name: String
  }

  static let pack = "水杉娘"
  static let maximumStickers = 256
  static let maximumNameLength = 16
  static let maximumManifestBytes = 64 * 1024
  /// 单张贴纸文件的上限；随包的都在 40KB 以内。
  static let maximumImageBytes = 512 * 1024
  /// 一格在手机竖屏上至少这么宽；更宽的键盘按它增加列数。
  static let cellWidth: CGFloat = 72
  static let minimumColumns = 5

  /// 随键盘打包的表情包，读不到或清单无效时为空，面板也就不显示这个分类。
  static let bundled: [Sticker] = {
    guard let resources = Bundle.main.resourceURL else { return [] }
    return (try? load(directory: resources.appendingPathComponent("stickers/shuishan", isDirectory: true))) ?? []
  }()

  static func columns(width: CGFloat) -> Int { max(minimumColumns, Int(width / cellWidth)) }

  /// 读清单并逐条校验：文件名只能是两位数字加 `.webp`，名字非空且不超过 16 个字符，条目数有上限；有任何一条不合格就整份不用。
  static func load(directory: URL) throws -> [Sticker] {
    let data = try BoundedFileReader.read(
      from: directory.appendingPathComponent("manifest.json"), maximumBytes: maximumManifestBytes)
    return try parse(manifest: data, directory: directory)
  }

  static func parse(manifest data: Data, directory: URL) throws -> [Sticker] {
    guard let root = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          let entries = root["stickers"] as? [[String: Any]],
          !entries.isEmpty, entries.count <= maximumStickers else { throw KeyboardStickerCatalogError.invalidManifest }
    var seen = Set<String>()
    return try entries.map { entry in
      guard let file = entry["file"] as? String, validFile(file), seen.insert(file).inserted,
            let name = (entry["name"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
            !name.isEmpty, name.count <= maximumNameLength
      else { throw KeyboardStickerCatalogError.invalidManifest }
      return Sticker(url: directory.appendingPathComponent(file), name: name)
    }
  }

  static func validFile(_ file: String) -> Bool {
    let scalars = Array(file.unicodeScalars)
    return scalars.count == 7 && file.hasSuffix(".webp")
      && scalars.prefix(2).allSatisfy { ("0"..."9").contains($0) }
  }

  /// 按显示边长（pt）和屏幕倍率下采样解码，不把整张 320px 图展开再缩。
  static func thumbnail(_ sticker: Sticker, side: CGFloat, scale: CGFloat) -> UIImage? {
    let scale = scale > 0 ? scale : 3
    guard let data = try? BoundedFileReader.read(from: sticker.url, maximumBytes: maximumImageBytes),
          let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
    let options: [CFString: Any] = [
      kCGImageSourceCreateThumbnailFromImageAlways: true,
      kCGImageSourceCreateThumbnailWithTransform: true,
      kCGImageSourceShouldCacheImmediately: true,
      kCGImageSourceThumbnailMaxPixelSize: max(1, Int((side * scale).rounded(.up))),
    ]
    return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary).map {
      UIImage(cgImage: $0, scale: scale, orientation: .up)
    }
  }

  /// 原尺寸 PNG：聊天应用普遍认 PNG，未必认 WebP，透明底也要留着。
  static func pngData(_ sticker: Sticker) -> Data? {
    guard let data = try? BoundedFileReader.read(from: sticker.url, maximumBytes: maximumImageBytes),
          let source = CGImageSourceCreateWithData(data as CFData, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
    let output = NSMutableData()
    guard let destination = CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil)
    else { return nil }
    CGImageDestinationAddImage(destination, image, nil)
    return CGImageDestinationFinalize(destination) ? output as Data : nil
  }

  static let pasteboardType = UTType.png.identifier

  /// 复制到剪贴板之后在候选栏显示的提示。
  static func copiedMessage(_ sticker: Sticker) -> String { "已复制\(pack)「\(sticker.name)」，长按输入框粘贴发送" }
  static let needsFullAccessMessage = "复制表情包需要在设置里允许完全访问"
  static let copyFailedMessage = "表情包复制失败，请重试"
}

enum KeyboardStickerCatalogError: Error {
  case invalidManifest
}

import UIKit
import ImageIO

@_silgen_name("msime_client_skin_package")
private func canvasPackage(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_skin_resource")
private func canvasResource(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func canvasStringFree(_ value: UnsafeMutablePointer<CChar>?)

extension ExternalCandidateSkin {
  /// 一次主题重载的不可变资源快照；普通候选条与展开面板共用，不在逐键路径中解码。
  final class Canvas: Equatable {
    struct Frames {
      let images: [CGImage]
      let times: [NSNumber]
      let duration: TimeInterval
      /// 0 为无限；ImageIO 已将 GIF 的重复次数换成总轮数。
      let loops: UInt

      static func decode(_ data: Data, remaining: inout Int) -> Frames? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let count = CGImageSourceGetCount(source)
        guard (1...120).contains(count) else { return nil }
        let container = CGImageSourceCopyProperties(source, nil) as? [String: Any] ?? [:]
        let animation = container["{PNG}"] as? [String: Any]
          ?? container["{GIF}"] as? [String: Any] ?? container["{WebP}"] as? [String: Any] ?? [:]
        let isGIF = (CGImageSourceGetType(source) as String?) == "com.compuserve.gif"
        let loops = (animation["LoopCount"] as? NSNumber)?.uintValue ?? (isGIF ? 1 : 0)
        var images: [CGImage] = [], starts: [Double] = [], elapsed: Double = 0
        for index in 0..<count {
          let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [String: Any] ?? [:]
          let width = (properties[kCGImagePropertyPixelWidth as String] as? NSNumber)?.intValue ?? 0
          let height = (properties[kCGImagePropertyPixelHeight as String] as? NSNumber)?.intValue ?? 0
          guard (1...2048).contains(width), (1...2048).contains(height), width * height <= remaining,
                let image = CGImageSourceCreateImageAtIndex(source, index,
                  [kCGImageSourceShouldCacheImmediately: true] as CFDictionary) else { return nil }
          remaining -= width * height
          images.append(image)
          starts.append(elapsed)
          let frame = properties["{PNG}"] as? [String: Any]
            ?? properties["{GIF}"] as? [String: Any] ?? properties["{WebP}"] as? [String: Any] ?? [:]
          let delay = (frame["UnclampedDelayTime"] as? NSNumber)?.doubleValue
            ?? (frame["DelayTime"] as? NSNumber)?.doubleValue ?? 0
          let ms = delay.isFinite && delay > 0 ? (delay * 1000).rounded() : 0
          elapsed += ms > 10 ? ms / 1000 : 0.1
        }
        return Frames(images: images, times: (starts.map { NSNumber(value: $0 / elapsed) } + [1]),
                      duration: elapsed, loops: loops)
      }

      func cropped(to rect: CGRect) -> Frames? {
        guard rect.width > 0, rect.height > 0 else { return nil }
        let cropped = images.compactMap { $0.cropping(to: rect) }
        guard cropped.count == images.count else { return nil }
        return Frames(images: cropped, times: times, duration: duration, loops: loops)
      }
    }

    struct Decoration {
      let frames: Frames
      let anchor: String
      let offsetX, offsetY, width, height: CGFloat
      let zIndex: Int
    }

    let tiles: [Frames?]
    let widths: [CGFloat]
    let fill: Bool
    let contentInsets: UIEdgeInsets
    let outerInsets: UIEdgeInsets
    let decorations: [Decoration]
    let itemCornerRadius: CGFloat
    let lineGap: CGFloat
    var panelLoaded: Bool { tiles.count == 9 }

    init(tiles: [Frames?], widths: [CGFloat], fill: Bool, contentInsets: UIEdgeInsets,
         outerInsets: UIEdgeInsets, decorations: [Decoration], itemCornerRadius: CGFloat, lineGap: CGFloat) {
      self.tiles = tiles
      self.widths = widths
      self.fill = fill
      self.contentInsets = contentInsets
      self.outerInsets = outerInsets
      self.decorations = decorations
      self.itemCornerRadius = itemCornerRadius
      self.lineGap = lineGap
    }

    static func == (lhs: Canvas, rhs: Canvas) -> Bool { lhs === rhs }

    /// 源切片与目标边宽都按比例夹紧，窄面板不会出现负宽度。
    static func cuts(_ size: CGFloat, start: CGFloat, end: CGFloat, origin: CGFloat = 0) -> [CGFloat] {
      let size = max(0, size), sum = start + end
      let scale = sum > size && sum > 0 ? size / sum : 1
      return [origin, origin + start * scale, origin + size - end * scale, origin + size]
    }

    static func sides(_ value: Any?, fallback: [CGFloat]) -> [CGFloat] {
      guard let raw = value as? [NSNumber], [1, 2, 4].contains(raw.count) else { return fallback }
      let values = raw.map { CGFloat($0.doubleValue) }
      switch values.count {
      case 1: return Array(repeating: values[0], count: 4)
      case 2: return [values[0], values[1], values[0], values[1]]
      default: return values
      }
    }
  }

  /// 只从共享层取得已校验的清单和资源；文件访问与图片解码必须在后台队列完成。
  static func canvas(_ id: String, dark: Bool, root: URL? = defaultRoot) -> Canvas? {
    guard let root else { return nil }
    func reply(_ request: [String: Any], resource: Bool) -> [String: Any]? {
      guard let body = try? JSONSerialization.data(withJSONObject: request) else { return nil }
      return body.withUnsafeBytes { bytes in
        let pointer = bytes.bindMemory(to: UInt8.self).baseAddress
        guard let raw = resource ? canvasResource(pointer, UInt(bytes.count)) : canvasPackage(pointer, UInt(bytes.count)) else { return nil }
        defer { canvasStringFree(raw) }
        guard let envelope = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any],
              envelope["ok"] as? Bool == true else { return nil }
        return envelope["value"] as? [String: Any]
      }
    }
    guard let package = reply(["directory": root.path, "id": id], resource: false),
          package["canvas"] as? Bool == true else { return nil }
    var remaining = 8_000_000, loaded: [String: Canvas.Frames] = [:]
    func load(_ relative: String) -> Canvas.Frames? {
      if let frames = loaded[relative] { return frames }
      guard let value = reply(["directory": root.path, "id": id, "relative": relative, "kind": "image"], resource: true),
            let bytes = value["bytes"] as? [NSNumber],
            let frames = Canvas.Frames.decode(Data(bytes.map { UInt8(truncating: $0) }), remaining: &remaining) else { return nil }
      loaded[relative] = frames
      return frames
    }
    func number(_ entry: [String: Any], _ key: String, fallback: CGFloat = 0) -> CGFloat {
      (entry[key] as? NSNumber).map { CGFloat($0.doubleValue) } ?? fallback
    }
    func insets(_ key: String) -> UIEdgeInsets {
      let value = package[key] as? [String: Any] ?? [:]
      return UIEdgeInsets(top: number(value, "top"), left: number(value, "left"),
                          bottom: number(value, "bottom"), right: number(value, "right"))
    }
    var tiles: [Canvas.Frames?] = [], widths: [CGFloat] = [0, 0, 0, 0], fill = true
    if let border = package["borderImage"] as? [String: Any] {
      let slices = Canvas.sides(border["stretchDip"] ?? border["sliceDip"], fallback: [0, 0, 0, 0])
      widths = Canvas.sides(border["widthDip"], fallback: slices)
      fill = border["fill"] as? Bool ?? true
      let path = (!dark ? border["lightSource"] as? String : nil) ?? border["source"] as? String
      if let path, let frames = load(path), let image = frames.images.first {
        let x = Canvas.cuts(CGFloat(image.width), start: slices[3], end: slices[1])
        let y = Canvas.cuts(CGFloat(image.height), start: slices[0], end: slices[2])
        for row in 0..<3 {
          for col in 0..<3 {
            tiles.append(frames.cropped(to: CGRect(x: x[col], y: y[row], width: x[col + 1] - x[col], height: y[row + 1] - y[row])))
          }
        }
      }
    }
    let entries = package["decorations"] as? [[String: Any]] ?? []
    let decorations: [Canvas.Decoration] = entries.enumerated().sorted {
      let left = ($0.element["zIndex"] as? Int) ?? 2, right = ($1.element["zIndex"] as? Int) ?? 2
      return left == right ? $0.offset < $1.offset : left < right
    }.compactMap { _, entry in
      guard let path = entry["image"] as? String, let frames = load(path) else { return nil }
      return Canvas.Decoration(frames: frames, anchor: entry["anchor"] as? String ?? "top-right",
        offsetX: number(entry, "offsetXDip"), offsetY: number(entry, "offsetYDip"),
        width: number(entry, "widthDip"), height: number(entry, "heightDip"), zIndex: (entry["zIndex"] as? Int) ?? 2)
    }
    guard !tiles.isEmpty || !decorations.isEmpty else { return nil }
    var content = insets("contentInsetsDip")
    content.top = max(content.top, widths[0]); content.right = max(content.right, widths[1])
    content.bottom = max(content.bottom, widths[2]); content.left = max(content.left, widths[3])
    return Canvas(tiles: tiles, widths: widths, fill: fill, contentInsets: content,
                  outerInsets: insets("outerInsetsDip"), decorations: decorations,
                  itemCornerRadius: number(package, "itemCornerRadiusDip", fallback: 9), lineGap: number(package, "lineGapDip"))
  }
}

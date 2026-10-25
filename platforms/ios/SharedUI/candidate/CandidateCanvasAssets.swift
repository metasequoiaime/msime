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

    struct Word: Equatable {
      let text: String
      var badge = ""
      var highlighted = false
    }

    /// 只消费共享扫描器发布的有界声明，不在宿主重新解析清单颜色。
    struct CandidateOverlay {
      let index: Int
      let fontSize, minFontSize, maxWidth, offsetX, offsetY: CGFloat
      let color: UIColor
      let alignH, alignV: String
      let stripBadge: Bool

      init?(_ raw: Any?) {
        guard let entry = raw as? [String: Any] else { return nil }
        func number(_ key: String, _ range: ClosedRange<Double>) -> CGFloat? {
          guard let value = entry[key] as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(),
                value.doubleValue.isFinite, range.contains(value.doubleValue) else { return nil }
          return CGFloat(value.doubleValue)
        }
        guard let index = SharedNumber.strictInt(entry["index"]), (0...100).contains(index),
              let font = number("fontSizeDip", 0...200), font > 0,
              let minimum = number("minFontSizeDip", 0...200), minimum > 0,
              let width = number("maxWidthDip", 0...2000),
              let x = number("offsetXDip", -2000...2000), let y = number("offsetYDip", -2000...2000),
              let horizontal = entry["alignH"] as? String, ["left", "center", "right"].contains(horizontal),
              let vertical = entry["alignV"] as? String, ["top", "center", "bottom"].contains(vertical),
              let strip = entry["stripBadge"] as? NSNumber, CFGetTypeID(strip) == CFBooleanGetTypeID(),
              let color = entry["resolvedColor"] as? String, color.utf8.count == 9, color.first == "#",
              color.dropFirst().utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0) }),
              let rgba = UInt32(color.dropFirst(), radix: 16) else { return nil }
        self.index = index
        fontSize = font; minFontSize = minimum; maxWidth = width; offsetX = x; offsetY = y
        alignH = horizontal; alignV = vertical; stripBadge = strip.boolValue
        self.color = UIColor(red: CGFloat((rgba >> 24) & 255) / 255, green: CGFloat((rgba >> 16) & 255) / 255,
          blue: CGFloat((rgba >> 8) & 255) / 255, alpha: CGFloat(rgba & 255) / 255)
      }

      func word(_ words: [Word]) -> String {
        let seat = index == 0 ? words.firstIndex(where: \.highlighted) : index - 1
        guard let seat, words.indices.contains(seat) else { return "" }
        return words[seat].text + (stripBadge ? "" : words[seat].badge)
      }
    }

    struct Banner {
      let frames: Frames
      let width, height: CGFloat
      let align: String
      let candidate: CandidateOverlay?
    }

    let tiles: [Frames?]
    let widths: [CGFloat]
    let fill: Bool
    let contentInsets: UIEdgeInsets
    let outerInsets: UIEdgeInsets
    let decorations: [Decoration]
    let itemCornerRadius: CGFloat
    let lineGap: CGFloat
    let banner: Banner?
    var panelLoaded: Bool { tiles.count == 9 }

    init(tiles: [Frames?], widths: [CGFloat], fill: Bool, contentInsets: UIEdgeInsets,
         outerInsets: UIEdgeInsets, decorations: [Decoration], itemCornerRadius: CGFloat, lineGap: CGFloat, banner: Banner? = nil) {
      self.tiles = tiles
      self.widths = widths
      self.fill = fill
      self.contentInsets = contentInsets
      self.outerInsets = outerInsets
      self.decorations = decorations
      self.itemCornerRadius = itemCornerRadius
      self.lineGap = lineGap
      self.banner = banner
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
    guard let package = reply(["directory": root.path, "id": id], resource: false) else { return nil }
    let isCanvas = package["canvas"] as? Bool == true
    var remaining = 8_000_000, loaded: [String: Canvas.Frames] = [:]
    func load(_ relative: String) -> Canvas.Frames? {
      if let frames = loaded[relative] { return frames }
      // base64 rather than the default `bytes` array: for a 1 MiB image the array costs about 64 MiB while the shared layer builds it and about 40 MiB more while JSONSerialization turns it back into bytes, either of which is more than the keyboard extension holds for everything else; base64 costs about 5 MiB and 3 MiB.
      guard let value = reply(["directory": root.path, "id": id, "relative": relative, "kind": "image", "encoding": "base64"],
                              resource: true),
            let encoded = value["bytesBase64"] as? String, let bytes = Data(base64Encoded: encoded),
            let frames = Canvas.Frames.decode(bytes, remaining: &remaining) else { return nil }
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
    if isCanvas, let border = package["borderImage"] as? [String: Any] {
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
    let entries = isCanvas ? package["decorations"] as? [[String: Any]] ?? [] : []
    let decorations: [Canvas.Decoration] = entries.enumerated().sorted {
      let left = ($0.element["zIndex"] as? Int) ?? 2, right = ($1.element["zIndex"] as? Int) ?? 2
      return left == right ? $0.offset < $1.offset : left < right
    }.compactMap { _, entry in
      guard let path = entry["image"] as? String, let frames = load(path) else { return nil }
      return Canvas.Decoration(frames: frames, anchor: entry["anchor"] as? String ?? "top-right",
        offsetX: number(entry, "offsetXDip"), offsetY: number(entry, "offsetYDip"),
        width: number(entry, "widthDip"), height: number(entry, "heightDip"), zIndex: (entry["zIndex"] as? Int) ?? 2)
    }
    var banner: Canvas.Banner?
    let width = number(package, "decorationWidthDip"), height = number(package, "decorationTopDip")
    if width > 0, height > 0, let path = package["decorationImage"] as? String, let frames = load(path) {
      banner = Canvas.Banner(frames: frames, width: width, height: height,
        align: package["decorationAlign"] as? String ?? "right", candidate: Canvas.CandidateOverlay(package["decorationCandidate"]))
    }
    guard !tiles.isEmpty || !decorations.isEmpty || banner != nil else { return nil }
    var content = isCanvas ? insets("contentInsetsDip") : .zero
    var outer = isCanvas ? insets("outerInsetsDip") : .zero
    // 传统装饰带与画布留白共用统一缩放，不增加键盘的固定高度。
    outer.top = max(outer.top, banner?.height ?? 0)
    content.top = max(content.top, widths[0]); content.right = max(content.right, widths[1])
    content.bottom = max(content.bottom, widths[2]); content.left = max(content.left, widths[3])
    return Canvas(tiles: tiles, widths: widths, fill: fill, contentInsets: content,
                  outerInsets: outer, decorations: decorations,
                  itemCornerRadius: number(package, "itemCornerRadiusDip", fallback: 9), lineGap: isCanvas ? number(package, "lineGapDip") : 0, banner: banner)
  }
}

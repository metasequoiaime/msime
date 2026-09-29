import AppKit
import SwiftUI

@main enum EmojiFlowTest {
  @MainActor static func main() {
    let measure: (String, CGFloat) -> CGSize = { text, size in
      CGSize(width: CGFloat(text.count) * size, height: size)
    }
    let texts = ["aaaaa", "bbbbbb", "cccccccc", "ddddd", "eeeeeee", "fffff"]
    let cells = MacEmojiFlow.cells(texts: texts, width: 200, measure: measure)
    assert(cells.map(\.row) == [0, 0, 1, 1, 2, 2])
    assert(cells.map { $0.rect.width } == [80, 92, 116, 80, 104, 80])
    assert(cells[1].rect.minX == 84 && cells[2].rect.minX == 0)
    assert(cells[2].rect.minY == MacEmojiFlow.height + MacEmojiFlow.rowGap)
    assert(MacEmojiFlow.vertical(from: 1, direction: 1, cells: cells) == 3)
    assert(MacEmojiFlow.vertical(from: 2, direction: -1, cells: cells) == 0)
    assert(MacEmojiFlow.vertical(from: 0, direction: -1, cells: cells) == 0)
    assert(MacEmojiFlow.vertical(from: 5, direction: 1, cells: cells) == 5)
    assert(MacEmojiFlow.vertical(from: -1, direction: 1, cells: cells) == nil)
    assert(MacEmojiFlow.cells(texts: [], width: 200).isEmpty)
    for width: CGFloat in [0, -1, .infinity, .nan] {
      assert(MacEmojiFlow.cells(texts: texts, width: width).isEmpty)
    }
    let long = MacEmojiFlow.cells(texts: [String(repeating: "synthetic", count: 100)], width: 120, measure: measure)[0]
    assert(long.rect.width == 120 && long.fontSize == MacEmojiFlow.minimumFontSize)
    let short = MacEmojiFlow.cells(texts: [";-)", "abcdef"], width: 200, measure: measure)
    assert(short[0].fontSize == 28 && short[1].fontSize == 12)
    let near = MacEmojiFlow.cells(texts: ["aaaaa", "bbbbb"], width: 163.8, measure: measure)
    assert(near[1].row == 0) // Windows half-unit wrapping tolerance, scaled to native points.
    let wrapped = MacEmojiFlow.cells(texts: ["aaaaa", "bbbbb"], width: 163.5, measure: measure)
    assert(wrapped[1].row == 1)
    for width: CGFloat in [1, 50, 200, 500] {
      let layout = MacEmojiFlow.cells(texts: texts, width: width, measure: measure)
      for cell in layout { assert(cell.rect.minX >= 0 && cell.rect.maxX <= width + 0.5 * MacEmojiFlow.scale) }
      for index in 1..<layout.count where layout[index].row == layout[index - 1].row {
        assert(layout[index].rect.minX >= layout[index - 1].rect.maxX + MacEmojiFlow.gap)
      }
    }
    // Native font measurement and actual SwiftUI rendering, without an installed input method.
    _ = NSApplication.shared
    let native = MacEmojiFlow.cells(texts: texts, width: 200)
    assert(native.count == texts.count && native.allSatisfy { $0.rect.width > 0 })
    let items = texts.map { MacEmojiCatalogItem(text: $0, annotation: "synthetic fixture", group: "fixture") }
    let renderer = ImageRenderer(content: MacEmojiFlowGrid(items: items, cells: cells, width: 200,
      palette: MacEmojiPalette(light: true), selected: { $0 == 3 }, identity: { $0 }, copy: { _ in })
      .background(Color.white))
    renderer.scale = 3
    guard let image = renderer.cgImage else { fatalError("Flow render failed") }
    assert(image.width == 600 && image.height == Int(ceil(cells.last!.rect.maxY * 3)))
    var pixels = [UInt8](repeating: 0, count: image.width * image.height * 4)
    pixels.withUnsafeMutableBytes { buffer in
      let context = CGContext(data: buffer.baseAddress, width: image.width, height: image.height,
        bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
      context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
    }
    let selectedRect = cells[3].rect
    let pixel = (Int((selectedRect.minY + 12) * 3) * image.width + Int((selectedRect.minX + 12) * 3)) * 4
    for (channel, expected) in [223, 232, 229].enumerated() {
      assert(abs(Int(pixels[pixel + channel]) - expected) <= 2)
    }
    print("Emoji flow geometry, fitting, wrapping, navigation and native rendering passed")
  }
}

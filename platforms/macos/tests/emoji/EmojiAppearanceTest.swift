import AppKit
import SwiftUI

@main enum EmojiAppearanceTest {
  @MainActor static func main() {
    _ = NSApplication.shared
    for light in [false, true] {
      let palette = MacEmojiPalette(light: light)
      let expected: [UInt32] = light
        ? [0xF7F7FA, 0x202027, 0x686873, 0xDFE8E5, 0xCEDED7, 0x2C7A4B]
        : [0x202027, 0xF5F5F7, 0xAFAFB7, 0x3B3B44, 0x555560, 0x5FBF84]
      assert([palette.background, palette.text, palette.muted, palette.selected, palette.pressed, palette.accent] == expected)
      for rgb in expected {
        let renderer = ImageRenderer(content: Rectangle().fill(MacEmojiPalette.color(rgb)).frame(width: 8, height: 8))
        guard let image = renderer.cgImage else {
          fatalError("Palette rendering unavailable")
        }
        var pixels = [UInt8](repeating: 0, count: 8 * 8 * 4)
        pixels.withUnsafeMutableBytes { buffer in
          let context = CGContext(data: buffer.baseAddress, width: 8, height: 8, bitsPerComponent: 8,
            bytesPerRow: 32, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
          context.draw(image, in: CGRect(x: 0, y: 0, width: 8, height: 8))
        }
        let center = (4 * 8 + 4) * 4
        let actual = pixels[center..<(center + 3)].map(Int.init)
        let channels = [Int((rgb >> 16) & 255), Int((rgb >> 8) & 255), Int(rgb & 255)]
        assert(zip(actual, channels).allSatisfy { abs($0 - $1) <= 1 })
      }
      assert(palette.cellFill(hovered: false, isPressed: false) == nil)
      assert(palette.cellFill(hovered: true, isPressed: false) == palette.selected)
      for hovered in [false, true] {
        assert(palette.cellFill(hovered: hovered, isPressed: true) == palette.pressed)
      }
    }
    let appearance = MacEmojiAppearance()
    assert(appearance.colorScheme == nil)
    appearance.apply(["theme": "light"])
    assert(appearance.colorScheme == .light)
    appearance.apply(["theme": "light", "emoji_theme": "dark"])
    assert(appearance.colorScheme == .dark)
    appearance.apply(["theme": "dark", "emoji_theme": "light"])
    assert(appearance.colorScheme == .light)
    appearance.apply(["theme": "dark", "emoji_theme": "follow"])
    assert(appearance.colorScheme == .dark)
    appearance.apply(["theme": "system"])
    assert(appearance.colorScheme == nil)
    appearance.apply(["theme": "dark", "emoji_theme": "follow"])
    assert(appearance.colorScheme == .dark)
    for invalid: NSDictionary in [[:], ["theme": "invalid"], ["theme": 123], ["theme": "dark"]] {
      appearance.apply(invalid)
      assert(appearance.colorScheme == nil || appearance.colorScheme == .dark)
    }
    print("Emoji appearance checks passed")
  }
}

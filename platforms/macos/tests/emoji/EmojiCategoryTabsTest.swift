import AppKit
import SwiftUI

@main enum EmojiCategoryTabsTest {
  @MainActor static func main() throws {
    let names = ["Smileys", "People", "Animals", "Food", "Travel", "Activities", "Objects", "Symbols", "Flags"]
    assert(names.map { MacEmojiCategoryIcons.emoji("fixture \($0) group") } == ["😀", "🧑", "🐾", "🍕", "🚗", "🎉", "💡", "❤", "🏳"])
    assert(MacEmojiCategoryIcons.emoji("smileys") == "☺")
    assert(MacEmojiCategoryIcons.emoji("Smileys People") == "😀")
    let emoji = MacEmojiCategoryIcons.emojiTabs(names)
    assert(emoji.count == 10 && emoji[0].id == .recent && emoji[0].icon == "⏱")
    assert(emoji[1].id == .group("Smileys"))
    let groups = [MacEmojiSymbolGroup(parent: "fixture-a", title: "one"),
      MacEmojiSymbolGroup(parent: "fixture-a", title: "two"), .init(parent: "fixture-b", title: "three")]
    var requests: [String] = []
    let symbols = try MacEmojiCategoryIcons.symbolTabs(groups) { parent in
      requests.append(parent); return parent == "fixture-a" ? "+" : "="
    }
    assert(requests == ["fixture-a", "fixture-b"] && symbols.map(\.icon) == ["+", "="])
    let empty = try MacEmojiCategoryIcons.symbolTabs(groups, first: { _ in nil })
    assert(empty.isEmpty)
    do {
      _ = try MacEmojiCategoryIcons.symbolTabs(groups) { _ in throw NSError(domain: "Synthetic", code: 1) }
      assertionFailure("symbol error swallowed")
    } catch { }
    // 插件包排在内置分类之后，每个包一个分类，同名的包也分开；颜文字组和空组不提供图标。
    let plugins = [
      MacEmojiPluginSymbolGroup(pack: "fixture-b", packName: "Shared", tab: .kaomoji, title: "k", keywords: "", items: ["(^_^)"]),
      MacEmojiPluginSymbolGroup(pack: "fixture-b", packName: "Shared", tab: .symbols, title: "empty", keywords: "", items: []),
      MacEmojiPluginSymbolGroup(pack: "fixture-b", packName: "Shared", tab: .symbols, title: "one", keywords: "", items: ["★", "☆"]),
      MacEmojiPluginSymbolGroup(pack: "fixture-b", packName: "Shared", tab: .symbols, title: "two", keywords: "", items: ["♠"]),
      MacEmojiPluginSymbolGroup(pack: "fixture-c", packName: "Shared", tab: .symbols, title: "one", keywords: "", items: ["♣"]),
      MacEmojiPluginSymbolGroup(pack: "fixture-d", packName: "fixture-a", tab: .kaomoji, title: "only", keywords: "", items: ["(-_-)"])]
    let pluginTabs = MacEmojiCategoryIcons.pluginSymbolTabs(plugins)
    assert(pluginTabs.map(\.id) == [MacEmojiPluginSymbolGroup.parentID(pack: "fixture-b"), MacEmojiPluginSymbolGroup.parentID(pack: "fixture-c")])
    assert(pluginTabs.map(\.title) == ["Shared", "Shared"] && pluginTabs.map(\.icon) == ["★", "♣"])
    let merged = symbols + pluginTabs
    assert(Set(merged.map(\.id)).count == merged.count && !merged.prefix(symbols.count).contains { MacEmojiPluginSymbolGroup.isParentID($0.id) })
    assert(MacEmojiCategoryIcons.pluginSymbolTabs([]).isEmpty)
    let idle = MacEmojiSymbolGroup.queryFilters(search: "", parent: "fixture-a", group: "one")
    assert(idle.parent == "fixture-a" && idle.group == "one")
    let searching = MacEmojiSymbolGroup.queryFilters(search: "synthetic", parent: "fixture-a", group: "one")
    assert(searching.parent.isEmpty && searching.group.isEmpty)
    assert(MacEmojiCategoryIcons.width(available: 300, count: 10) == 30)
    assert(MacEmojiCategoryIcons.width(available: 300, count: 2) == 52 * 2.0 / 3)
    assert(MacEmojiCategoryIcons.width(available: 300, count: 0) == 0)
    assert(MacEmojiCategoryIcons.width(available: .nan, count: 2) == 0)
    _ = NSApplication.shared
    let tabs = (0..<10).map { MacEmojiCategoryTab(id: $0, title: "synthetic-\($0)", icon: String($0)) }
    for light in [false, true] {
      let palette = MacEmojiPalette(light: light)
      let renderer = ImageRenderer(content: MacEmojiCategoryTabs(tabs: tabs, selected: 9, palette: palette, select: { _ in })
        .frame(width: 300).background(Color.white))
      renderer.scale = 3
      guard let image = renderer.cgImage else { fatalError("Category tabs render unavailable") }
      assert(image.width == 900 && image.height == 116)
      var pixels = [UInt8](repeating: 0, count: image.width * image.height * 4)
      pixels.withUnsafeMutableBytes { buffer in
        let context = CGContext(data: buffer.baseAddress, width: image.width, height: image.height,
          bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
      }
      for index in 0..<10 {
        let pixel = (113 * image.width + index * 90 + 45) * 4
        let color: UInt32 = index == 9 ? palette.accent : 0xFFFFFF
        for channel in 0..<3 {
          assert(abs(Int(pixels[pixel + channel]) - Int((color >> ((2 - channel) * 8)) & 255)) <= 2)
        }
      }
    }
    print("Category icons, symbol metadata, global search, widths and selected rendering passed")
  }
}

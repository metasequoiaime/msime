import SwiftUI

struct MacEmojiCategoryTab<ID: Hashable & Sendable>: Sendable {
  let id: ID
  let title: String
  let icon: String
}

enum MacEmojiCategoryIcons {
  static func emoji(_ title: String) -> String {
    for (name, icon) in [("Smileys", "😀"), ("People", "🧑"), ("Animals", "🐾"), ("Food", "🍕"),
      ("Travel", "🚗"), ("Activities", "🎉"), ("Objects", "💡"), ("Symbols", "❤"), ("Flags", "🏳")] {
      if title.contains(name) { return icon }
    }
    return "☺"
  }
  static func emojiTabs(_ groups: [String]) -> [MacEmojiCategoryTab<MacEmojiSectionChoice>] {
    [.init(id: .recent, title: "最近使用", icon: "⏱")] + groups.map {
      .init(id: .group($0), title: $0, icon: emoji($0))
    }
  }
  static func symbolTabs(_ groups: [MacEmojiSymbolGroup], first: (String) throws -> String?) throws -> [MacEmojiCategoryTab<String>] {
    try MacEmojiSymbolGroup.parents(groups).compactMap { parent in
      guard let icon = try first(parent), !icon.isEmpty else { return nil }
      return .init(id: parent, title: parent, icon: icon)
    }
  }
  /// 符号页的插件上级分类，每个插件包一个，按返回顺序（包名排序）排在内置分类之后。图标取该包 `symbols` 组的第一个符号，没有符号的包不出现。
  static func pluginSymbolTabs(_ groups: [MacEmojiPluginSymbolGroup]) -> [MacEmojiCategoryTab<String>] {
    var seen = Set<String>()
    return groups.compactMap { group in
      guard group.tab == .symbols, let icon = group.items.first, seen.insert(group.pack).inserted else { return nil }
      return .init(id: MacEmojiPluginSymbolGroup.parentID(pack: group.pack), title: group.packName, icon: icon)
    }
  }
  static func width(available: CGFloat, count: Int) -> CGFloat {
    guard available.isFinite, available > 0, count > 0 else { return 0 }
    return min(52 * 2 / 3, available / CGFloat(count))
  }
}

struct MacEmojiCategoryTabs<ID: Hashable & Sendable>: View {
  let tabs: [MacEmojiCategoryTab<ID>]
  let selected: ID
  let palette: MacEmojiPalette
  let select: (ID) -> Void
  var body: some View {
    GeometryReader { geometry in
      let width = MacEmojiCategoryIcons.width(available: geometry.size.width, count: tabs.count)
      HStack(spacing: 0) {
        ForEach(tabs, id: \.id) { tab in
          Button { select(tab.id) } label: {
            Text(tab.icon).font(.system(size: 22 * 2 / 3))
              .foregroundStyle(MacEmojiPalette.color(palette.text)).lineLimit(1)
              .frame(width: width, height: 58 * 2 / 3).clipped()
          }
          .buttonStyle(MacEmojiCategoryTabStyle(selected: tab.id == selected, palette: palette))
          .help(tab.title).accessibilityLabel(tab.title)
          .accessibilityAddTraits(tab.id == selected ? .isSelected : [])
        }
      }
    }.frame(height: 58 * 2 / 3)
  }
}

private struct MacEmojiCategoryTabStyle: ButtonStyle {
  let selected: Bool
  let palette: MacEmojiPalette
  func makeBody(configuration: Configuration) -> some View {
    Cell(configuration: configuration, selected: selected, palette: palette)
  }
  private struct Cell: View {
    let configuration: ButtonStyleConfiguration
    let selected: Bool
    let palette: MacEmojiPalette
    @State private var hovered = false
    var body: some View {
      configuration.label
        .background(!selected && (hovered || configuration.isPressed)
          ? MacEmojiPalette.color(palette.background == 0xF7F7FA ? 0xE9E7ED : 0x303038) : .clear,
          in: RoundedRectangle(cornerRadius: 4))
        .overlay(alignment: .bottom) {
          if selected {
            RoundedRectangle(cornerRadius: 4 / 3).fill(MacEmojiPalette.color(palette.accent))
              .frame(width: 22 * 2 / 3, height: 2)
          }
        }
        .contentShape(Rectangle()).onHover { hovered = $0 }
    }
  }
}

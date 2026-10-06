import SwiftUI

struct MacEmojiDetailSection<Content: View>: View {
  let title: String
  let palette: MacEmojiPalette
  @ViewBuilder let content: () -> Content
  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text(title).font(.system(size: 18 * 2 / 3, weight: .semibold))
        .foregroundStyle(MacEmojiPalette.color(palette.text)).lineLimit(1)
        .frame(height: MacEmojiSymbolSections.titleHeight)
        .padding(.leading, 4 * 2 / 3)
        .frame(maxWidth: .infinity, alignment: .leading).clipped()
      content()
    }.padding(.bottom, MacEmojiSymbolSections.bottomPadding)
  }
}

struct MacEmojiSymbolSection {
  struct Identity: Hashable { let start: Int }
  let title: String
  let start: Int
  var items: [MacEmojiCatalogItem]
  var id: Identity { Identity(start: start) }
}

enum MacEmojiSymbolSections {
  static let titleHeight: CGFloat = 48 * 2 / 3
  static let bottomPadding: CGFloat = 18 * 2 / 3

  /// Preserve contiguous catalog runs and global indices, including repeated titles.
  static func split(_ items: [MacEmojiCatalogItem]) -> [MacEmojiSymbolSection] {
    var sections: [MacEmojiSymbolSection] = []
    for (index, item) in items.enumerated() {
      if sections.last?.title == item.group {
        sections[sections.count - 1].items.append(item)
      } else {
        sections.append(.init(title: item.group, start: index, items: [item]))
      }
    }
    return sections
  }

  /// 颜文字页的分组：内置颜文字为 All，插件 `kaomoji` 组依次排在后面；没有内置命中时不出现 All。`start` 是在拼接后列表里的全局下标。
  static func kaomoji(builtIn: [MacEmojiCatalogItem], plugins: [MacEmojiPluginSymbolGroup], search: String) -> [MacEmojiSymbolSection] {
    var sections: [MacEmojiSymbolSection] = []
    func append(_ title: String, _ items: [MacEmojiCatalogItem]) {
      sections.append(.init(title: title, start: (sections.last.map { $0.start + $0.items.count }) ?? 0, items: items))
    }
    if !builtIn.isEmpty { append("All", builtIn) }
    for group in MacEmojiPluginSymbolGroup.kaomojiSections(plugins, search: search) { append(group.title, group.items) }
    return sections
  }

  static func height(_ section: MacEmojiSymbolSection) -> CGFloat {
    titleHeight + MacEmojiGridMetrics.height(count: section.items.count) + bottomPadding
  }
}

struct MacEmojiSymbolSectionsView: View {
  let items: [MacEmojiCatalogItem]
  let palette: MacEmojiPalette
  let selectedIndex: Int
  let copy: (Int) -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      ForEach(MacEmojiSymbolSections.split(items), id: \.id) { section in
        MacEmojiDetailSection(title: section.title, palette: palette) {
          MacEmojiGrid(items: section.items, palette: palette,
            selected: { selectedIndex == section.start + $0 },
            identity: { section.start + $0 }, copy: { copy(section.start + $0) })
        }
      }
    }.frame(width: MacEmojiGridMetrics.width, alignment: .leading)
  }
}

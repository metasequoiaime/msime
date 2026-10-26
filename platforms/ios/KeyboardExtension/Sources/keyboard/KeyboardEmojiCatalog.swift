import Foundation
import CoreFoundation

enum KeyboardEmojiCatalogError: Error {
  case invalidPage
}

/// Bounded host model for the Engine-owned Emoji catalog.
///
/// Android, iOS and the desktop panel all read the same verified `msime-others.db` through the shared
/// host API. The keyboard keeps only the currently visible category page and never opens SQLite.
enum KeyboardEmojiCatalog {
  static let columns = 8
  static let pageSize = 64
  static let recentLimit = 24
  static let maximumItems = 4_096
  static let maximumCursor = 8_192

  struct Category: Equatable, Sendable {
    let group: String
    let title: String
    /// The shared catalog the group belongs to: empty for Emoji, `kaomoji` for 颜文字.
    var catalog = ""
    /// Letters the Engine matches against each Emoji's pinyin and keywords; a search spans every group, so its `group` is empty.
    var search = ""

    var isKaomoji: Bool { catalog == "kaomoji" }
  }

  struct Item: Equatable, Sendable {
    let text: String
    let annotation: String
    let group: String
  }

  struct Page: Equatable, Sendable {
    let items: [Item]
    let nextOffset: Int
    let complete: Bool
  }

  /// Unicode group order. Database row order interleaves Symbols and Flags.
  static let categories = [
    Category(group: "Smileys and emotion", title: "笑脸"),
    Category(group: "People and body", title: "人物"),
    Category(group: "Animals and nature", title: "动物"),
    Category(group: "Food and drink", title: "食物"),
    Category(group: "Travel and places", title: "旅行"),
    Category(group: "Activities", title: "活动"),
    Category(group: "Objects", title: "物品"),
    Category(group: "Symbols", title: "符号"),
    Category(group: "Flags", title: "旗帜"),
  ]

  /// The kaomoji catalog has no groups of its own; the shared catalog answers every row under `All`.
  static let kaomoji = Category(group: "All", title: "颜文字", catalog: "kaomoji")

  /// The longest search the letter pad accepts; the longest pinyin syllable run worth typing for one Emoji is far shorter.
  static let maximumSearchLength = 32

  /// A search for `query`, lowercased and limited to ASCII letters, or `nil` when nothing is left to search for. It spans every Emoji group, or with `kaomoji` the kaomoji catalog, whose keywords are pinyin and English too.
  static func search(_ query: String, kaomoji: Bool = false) -> Category? {
    let letters = String(query.lowercased().unicodeScalars.filter { ("a"..."z").contains($0) }.prefix(maximumSearchLength))
    guard !letters.isEmpty else { return nil }
    return kaomoji
      ? Category(group: Self.kaomoji.group, title: "搜索", catalog: Self.kaomoji.catalog, search: letters)
      : Category(group: "", title: "搜索", search: letters)
  }

  static func loadPage(resources: String, category: Category, offset: Int) throws -> Page {
    guard offset >= 0, offset <= maximumCursor else { throw KeyboardEmojiCatalogError.invalidPage }
    let request = try JSONSerialization.data(withJSONObject: [
      "category": category.catalog,
      "group": category.group,
      "search": category.search,
      "offset": offset,
      "limit": pageSize,
      "cursor": true,
    ])
    return try decodePage(
      try MetasequoiaInputSessionBridge.emojiCatalog(request: request, resources: resources),
      category: category,
      requestedOffset: offset)
  }

  static func decodePage(
    _ value: [String: Any], category: Category, requestedOffset: Int
  ) throws -> Page {
    guard requestedOffset >= 0, requestedOffset <= maximumCursor,
          let rows = value["items"] as? [[String: Any]], rows.count <= pageSize,
          let nextOffset = SharedNumber.nonnegativeInt(value["next_offset"]),
          let complete = value["complete"] as? Bool else {
      throw KeyboardEmojiCatalogError.invalidPage
    }
    guard nextOffset >= requestedOffset,
          nextOffset <= requestedOffset + pageSize,
          nextOffset <= maximumCursor,
          complete || nextOffset > requestedOffset else {
      throw KeyboardEmojiCatalogError.invalidPage
    }
    let items = try rows.map { row -> Item in
      guard let text = row["text"] as? String,
            let annotation = row["annotation"] as? String,
            let group = row["group"] as? String,
            validText(text, kaomoji: category.isKaomoji), validAnnotation(annotation),
            category.group.isEmpty || group == category.group else {
        throw KeyboardEmojiCatalogError.invalidPage
      }
      return Item(text: text, annotation: annotation, group: group)
    }
    return Page(items: items, nextOffset: nextOffset, complete: complete)
  }

  /// Chinese names for the Engine's symbol parents; a parent the catalog adds later shows under its own name.
  static let symbolParentTitles = [
    "Punctuation": "标点", "Math": "数学", "Currency": "货币", "Arrows and lines": "箭头",
    "Stars and shapes": "形状", "Hearts": "爱心", "Letters": "字母", "Games": "游戏",
    "Culture": "文化", "Animals and nature": "自然", "People and activity": "人物", "More": "更多",
  ]
  /// The most rows one parent may hold; the largest shipped parent, Letters, has 567.
  static let maximumSymbols = 2_048

  /// The Engine's symbol parents in catalog order, each once.
  static func symbolParents(resources: String) throws -> [String] {
    let request = try JSONSerialization.data(withJSONObject: ["list_symbol_groups": true, "limit": 1])
    let value = try MetasequoiaInputSessionBridge.emojiCatalog(request: request, resources: resources)
    return try decodeSymbolParents(value)
  }

  static func decodeSymbolParents(_ value: [String: Any]) throws -> [String] {
    guard let rows = value["symbol_groups"] as? [[String: Any]], rows.count <= 1_024 else {
      throw KeyboardEmojiCatalogError.invalidPage
    }
    var parents: [String] = []
    for row in rows {
      guard let parent = row["parent"] as? String, !parent.isEmpty, validAnnotation(parent) else { continue }
      if !parents.contains(parent) { parents.append(parent) }
    }
    return parents
  }

  /// Every symbol under one parent, in catalog order. A symbol filed under two of the parent's subgroups appears once.
  static func loadSymbols(resources: String, parent: String) throws -> [String] {
    try collectSymbols { offset in
      let request = try JSONSerialization.data(withJSONObject: [
        "category": "symbols", "parent": parent, "offset": offset, "limit": 255, "cursor": true,
      ])
      return try MetasequoiaInputSessionBridge.emojiCatalog(request: request, resources: resources)
    }
  }

  /// The most symbols one search shows: a single Engine page, since a short query such as one letter matches most of the catalog and the first rows are the ones worth reading.
  static let maximumSymbolMatches = 255

  /// Symbols whose keywords match `query`: the catalog files each under English words, full pinyin and pinyin initials (`arrow`, `jiantou`, `jt`), so the letters the search pad types are enough. `nil` when nothing is left to search for.
  static func searchSymbols(resources: String, query: String) throws -> [String]? {
    guard let letters = search(query)?.search else { return nil }
    let request = try JSONSerialization.data(withJSONObject: [
      "category": "symbols", "search": letters, "offset": 0, "limit": maximumSymbolMatches, "cursor": true,
    ])
    return try decodeSymbolMatches(try MetasequoiaInputSessionBridge.emojiCatalog(request: request, resources: resources))
  }

  static func decodeSymbolMatches(_ value: [String: Any]) throws -> [String] {
    guard let rows = value["items"] as? [[String: Any]], rows.count <= maximumSymbolMatches else {
      throw KeyboardEmojiCatalogError.invalidPage
    }
    var symbols: [String] = []
    var seen = Set<String>()
    for row in rows {
      guard let text = row["text"] as? String, validText(text) else { throw KeyboardEmojiCatalogError.invalidPage }
      if seen.insert(text).inserted { symbols.append(text) }
    }
    return symbols
  }

  static func collectSymbols(page: (Int) throws -> [String: Any]) throws -> [String] {
    var symbols: [String] = []
    var seen = Set<String>()
    var offset = 0
    while true {
      let value = try page(offset)
      guard let rows = value["items"] as? [[String: Any]], rows.count <= 255,
            let next = SharedNumber.nonnegativeInt(value["next_offset"]),
            let complete = value["complete"] as? Bool,
            complete || next > offset else {
        throw KeyboardEmojiCatalogError.invalidPage
      }
      for row in rows {
        guard let text = row["text"] as? String, validText(text) else { throw KeyboardEmojiCatalogError.invalidPage }
        if seen.insert(text).inserted { symbols.append(text) }
      }
      guard symbols.count <= maximumSymbols else { throw KeyboardEmojiCatalogError.invalidPage }
      if complete { return symbols }
      guard next <= maximumSymbols else { throw KeyboardEmojiCatalogError.invalidPage }
      offset = next
    }
  }

  // MARK: - 符号集插件

  /// 符号集插件的一组，`msime_client_emoji_catalog_request` 在 `list_plugin_symbol_groups` 下回传，字段与接口相同。
  struct PluginSymbolGroup: Equatable, Sendable {
    let pack: String
    let packName: String
    /// `symbols` 或 `kaomoji`。
    let tab: String
    let title: String
    /// 空串表示插件没有给关键词。
    let keywords: String
    let items: [String]

    /// 搜索时这一组里 `query` 匹配的条目：条目本身含有它，或者组的关键词含有它（不分大小写），与鸿蒙的 `EmojiPanelKeyPolicy.matches` 相同。
    func matches(_ query: String) -> [String] {
      guard !query.isEmpty else { return [] }
      if keywords.lowercased().contains(query.lowercased()) { return items }
      return items.filter { $0.contains(query) }
    }
  }

  /// 单组最多接收的条目数，与插件清单的上限一致。
  static let maximumPluginGroupItems = 512

  /// 符号集插件的各组：插件目录（`<共享状态目录>/plugins`）里装着的全部符号集，包按名称、组按清单顺序。读不出来时为空，面板只显示内置内容。
  static func pluginSymbolGroups(resources: String, plugins: String) -> [PluginSymbolGroup] {
    guard NSString(string: plugins).isAbsolutePath,
          let request = try? JSONSerialization.data(withJSONObject: [
            "list_plugin_symbol_groups": true, "plugins": plugins, "limit": 1,
          ]),
          let value = try? MetasequoiaInputSessionBridge.emojiCatalog(request: request, resources: resources) else { return [] }
    return decodePluginSymbolGroups(value)
  }

  /// 校验回传的组；坏组跳过而不是整份作废，与鸿蒙的 `PluginSymbolGroupPolicy.parse` 相同。
  static func decodePluginSymbolGroups(_ value: [String: Any]) -> [PluginSymbolGroup] {
    guard let rows = value["plugin_symbol_groups"] as? [[String: Any]] else { return [] }
    return rows.compactMap { row in
      guard let pack = row["pack"] as? String, validName(pack),
            let packName = row["pack_name"] as? String, validName(packName),
            let title = row["title"] as? String, validName(title),
            let tab = row["tab"] as? String, tab == "symbols" || tab == "kaomoji",
            let items = row["items"] as? [Any] else { return nil }
      let kept = items.compactMap { $0 as? String }.filter { validText($0, kaomoji: true) && !$0.trimmingCharacters(in: .whitespaces).isEmpty }
        .prefix(maximumPluginGroupItems)
      guard !kept.isEmpty else { return nil }
      let keywords = (row["keywords"] as? String).flatMap { validAnnotation($0) ? $0 : nil } ?? ""
      return PluginSymbolGroup(pack: pack, packName: packName, tab: tab, title: title, keywords: keywords, items: Array(kept))
    }
  }

  /// 符号面板的插件分类：每个插件一个分类，以插件名为标题，内容是它全部 `symbols` 组按清单顺序首尾相接，排在内置分类之后；不与内置目录去重，与桌面的 `append_plugin_symbol_groups`、鸿蒙的 `symbolTabs` 相同。
  static func pluginSymbolCategories(_ groups: [PluginSymbolGroup]) -> [(title: String, symbols: [String])] {
    var order: [String] = []
    var categories: [String: (title: String, symbols: [String])] = [:]
    for group in groups where group.tab == "symbols" {
      if categories[group.pack] == nil {
        order.append(group.pack)
        categories[group.pack] = (group.packName, [])
      }
      categories[group.pack]?.symbols += group.items
    }
    return order.compactMap { categories[$0] }
  }

  /// 颜文字的插件组：`kaomoji` 组按回传顺序逐组排在颜文字的 All 之后。
  static func pluginKaomojiGroups(_ groups: [PluginSymbolGroup]) -> [PluginSymbolGroup] {
    groups.filter { $0.tab == "kaomoji" }
  }

  private static func validName(_ value: String) -> Bool {
    !value.isEmpty && value.utf16.count <= 256 && !value.trimmingCharacters(in: .whitespaces).isEmpty
  }

  static func validRecent(_ value: String) -> Bool {
    validText(value)
  }

  static func validRecentKaomoji(_ value: String) -> Bool {
    validText(value, kaomoji: true)
  }

  /// A kaomoji is a short line of text rather than one pictograph; the longest in the shipped catalog is 59 characters.
  private static func validText(_ value: String, kaomoji: Bool = false) -> Bool {
    let scalars = kaomoji ? 96 : 32
    return !value.isEmpty && value.unicodeScalars.count <= scalars && value.utf8.count <= scalars * 8
      && !value.unicodeScalars.contains { $0.value == 0 }
  }

  private static func validAnnotation(_ value: String) -> Bool {
    value.unicodeScalars.count <= 1_024 && value.utf8.count <= 4_096
      && !value.unicodeScalars.contains { $0.value == 0 }
  }
}

enum KeyboardEmojiRecents {
  static let key = "emojiRecents"
  private static var defaults: UserDefaults { KeyboardFeedbackPreference.defaults }

  static var stored: [String] {
    normalize(defaults.stringArray(forKey: key) ?? [])
  }

  static func record(_ emoji: String) {
    guard KeyboardEmojiCatalog.validRecent(emoji) else { return }
    var recents = stored.filter { $0 != emoji }
    recents.insert(emoji, at: 0)
    defaults.set(Array(recents.prefix(KeyboardEmojiCatalog.recentLimit)), forKey: key)
  }

  static func normalize(_ values: [String]) -> [String] {
    var seen = Set<String>()
    var output: [String] = []
    for value in values where KeyboardEmojiCatalog.validRecent(value) && seen.insert(value).inserted {
      output.append(value)
      if output.count == KeyboardEmojiCatalog.recentLimit { break }
    }
    return output
  }
}

/// Kaomoji picked in the emoji picker, newest first, kept apart from the Emoji recents.
///
/// Windows keeps one recent list for its whole panel. On iOS the Emoji recents fill an eight-column pictograph grid, and a kaomoji is a line of text that needs the kaomoji tab's wide columns, so kaomoji get their own 最近颜文字 tab beside 颜文字.
enum KeyboardKaomojiRecents {
  static let key = "kaomojiRecents"
  /// Eight rows of the two columns a phone shows.
  static let limit = 16
  private static var defaults: UserDefaults { KeyboardFeedbackPreference.defaults }

  static var stored: [String] {
    normalize(defaults.stringArray(forKey: key) ?? [])
  }

  static func record(_ kaomoji: String) {
    guard KeyboardEmojiCatalog.validRecentKaomoji(kaomoji) else { return }
    var recents = stored.filter { $0 != kaomoji }
    recents.insert(kaomoji, at: 0)
    defaults.set(Array(recents.prefix(limit)), forKey: key)
  }

  static func normalize(_ values: [String]) -> [String] {
    var seen = Set<String>()
    var output: [String] = []
    for value in values where KeyboardEmojiCatalog.validRecentKaomoji(value) && seen.insert(value).inserted {
      output.append(value)
      if output.count == limit { break }
    }
    return output
  }
}

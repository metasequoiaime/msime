import Foundation

struct MacEmojiCatalogItem: Equatable, Sendable {
  let text: String
  let annotation: String
  let group: String
}

enum MacEmojiCatalog {
  static func decode(_ response: NSDictionary) throws -> [MacEmojiCatalogItem] {
    guard response["error"] == nil, let rows = response["items"] as? [[String: Any]] else {
      throw NSError(domain: "MSIMEEmojiCatalog", code: 1)
    }
    return try rows.map { row in
      guard let text = row["text"] as? String, !text.isEmpty,
            let annotation = row["annotation"] as? String,
            let group = row["group"] as? String else {
        throw NSError(domain: "MSIMEEmojiCatalog", code: 2)
      }
      return MacEmojiCatalogItem(text: text, annotation: annotation, group: group)
    }
  }

  static func load(resources: String, search: String, category: String = "", offset: Int = 0, group: String = "", parent: String = "", limit: Int = 255) throws -> [MacEmojiCatalogItem] {
    guard offset >= 0, (1...255).contains(limit) else { throw NSError(domain: "MSIMEEmojiCatalog", code: 3) }
    return try decode(request(resources: resources, parameters: ["search": search,
      "category": category, "offset": offset, "group": group, "parent": parent, "limit": limit]))
  }

  static func loadSymbolGroups(resources: String) throws -> [MacEmojiSymbolGroup] {
    let response = try request(resources: resources, parameters: ["list_symbol_groups": true])
    guard response["error"] == nil, let rows = response["symbol_groups"] as? [[String: String]] else {
      throw NSError(domain: "MSIMEEmojiCatalog", code: 5)
    }
    let groups = try rows.map { row -> MacEmojiSymbolGroup in
      guard let parent = row["parent"], !parent.isEmpty, let title = row["title"], !title.isEmpty else {
        throw NSError(domain: "MSIMEEmojiCatalog", code: 5)
      }
      return MacEmojiSymbolGroup(parent: parent, title: title)
    }
    guard Set(groups).count == groups.count else { throw NSError(domain: "MSIMEEmojiCatalog", code: 5) }
    return groups
  }

  static func loadGroups(resources: String, category: String) throws -> [String] {
    let response = try request(resources: resources, parameters: ["category": category, "list_groups": true])
    guard response["error"] == nil, let groups = response["groups"] as? [String],
          groups.allSatisfy({ !$0.isEmpty }), Set(groups).count == groups.count else {
      throw NSError(domain: "MSIMEEmojiCatalog", code: 4)
    }
    return groups
  }

  /// 已安装符号集插件的全部组：包按名字排序、组按清单顺序。不读 `others.db`，内置目录不可用时也能列出；`plugins` 必须是绝对路径。
  static func loadPluginSymbolGroups(resources: String, plugins: String) throws -> [MacEmojiPluginSymbolGroup] {
    guard NSString(string: plugins).isAbsolutePath else { throw NSError(domain: "MSIMEEmojiCatalog", code: 6) }
    let response = try request(resources: resources,
      parameters: ["list_plugin_symbol_groups": true, "plugins": plugins], requiresCatalog: false)
    guard response["error"] == nil, let rows = response["plugin_symbol_groups"] as? [[String: Any]] else {
      throw NSError(domain: "MSIMEEmojiCatalog", code: 6)
    }
    return try rows.map { row in
      guard let pack = row["pack"] as? String, !pack.isEmpty,
            let packName = row["pack_name"] as? String, !packName.isEmpty,
            let tab = (row["tab"] as? String).flatMap(MacEmojiPluginSymbolGroup.Tab.init(rawValue:)),
            let title = row["title"] as? String, !title.isEmpty,
            let keywords = row["keywords"] as? String,
            let items = row["items"] as? [String], items.allSatisfy({ !$0.isEmpty }) else {
        throw NSError(domain: "MSIMEEmojiCatalog", code: 6)
      }
      return MacEmojiPluginSymbolGroup(pack: pack, packName: packName, tab: tab, title: title, keywords: keywords, items: items)
    }
  }

  /// 面板用的插件组：没有偏好目录或读取失败时返回空，面板只显示内置目录。
  static func installedPluginSymbolGroups(resources: String, preferencesDirectory: String) -> [MacEmojiPluginSymbolGroup] {
    guard let plugins = MacEmojiPluginSymbolGroup.directory(preferencesDirectory: preferencesDirectory) else { return [] }
    return (try? loadPluginSymbolGroups(resources: resources, plugins: plugins)) ?? []
  }

  /// `requiresCatalog` 为假时不要求 `others.db` 可读，只用于不读内置目录的请求（插件符号组）。
  static func request(resources: String, parameters: [String: Any], requiresCatalog: Bool = true) throws -> NSDictionary {
    let selector = NSSelectorFromString("emojiCatalogRequest:")
    var payload = parameters
    payload["resources"] = resources
    guard NSString(string: resources).isAbsolutePath,
          !requiresCatalog || FileManager.default.isReadableFile(atPath: URL(fileURLWithPath: resources).appendingPathComponent("others.db").path),
          let type = NSClassFromString("MSIMEClientSession") as? NSObject.Type,
          type.responds(to: selector),
          let response = type.perform(selector, with: payload as NSDictionary)?.takeUnretainedValue() as? NSDictionary else {
      throw NSError(domain: "MSIMEEmojiCatalog", code: 3)
    }
    return response
  }
}

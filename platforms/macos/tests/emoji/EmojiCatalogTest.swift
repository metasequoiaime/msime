import Foundation

@objc(MSIMEClientSession) final class StubEmojiSession: NSObject {
  static var lastRequest: NSDictionary = [:]
  static var groups: Any = ["Z", "A"]
  static var symbolGroups: Any = [["parent": "P1", "title": "Shared"], ["parent": "P2", "title": "Shared"], ["parent": "P1", "title": "Other"]]
  static var pluginGroups: NSDictionary = ["plugin_symbol_groups": [
    ["pack": "fixture-arrows", "pack_name": "Arrows", "tab": "symbols", "title": "Basic", "keywords": "arrow direction", "items": ["←", "→"]],
    ["pack": "fixture-arrows", "pack_name": "Arrows", "tab": "kaomoji", "title": "Pointing", "keywords": "", "items": ["(☞ﾟ∀ﾟ)☞"]],
    ["pack": "fixture-math", "pack_name": "Math", "tab": "symbols", "title": "Operators", "keywords": "", "items": ["±", "→"]]
  ]]
  @objc class func emojiCatalogRequest(_ request: NSDictionary) -> NSDictionary {
    lastRequest = request
    if request["list_groups"] as? Bool == true { return ["groups": groups] }
    if request["list_plugin_symbol_groups"] as? Bool == true { return pluginGroups }
    if request["list_symbol_groups"] as? Bool == true { return ["symbol_groups": symbolGroups] }
    return ["items": [["text": "😀", "annotation": "笑脸", "group": "Smileys"]]]
  }
}

@main enum EmojiCatalogTest {
  static func main() throws {
    let rows = try MacEmojiCatalog.decode(["items": [
      ["text": "😀", "annotation": "笑脸 smile", "group": "Smileys"],
      ["text": "👨‍👩‍👧", "annotation": "家庭", "group": "People"]
    ]])
    assert(rows.count == 2 && rows[1].text == "👨‍👩‍👧")
    assert(rows[0].annotation == "笑脸 smile" && rows[0].group == "Smileys")
    let empty = try MacEmojiCatalog.decode(["items": []])
    assert(empty.isEmpty)
    for invalid: NSDictionary in [["error": "unavailable"], [:], ["items": [["text": "😀"]]],
                                 ["items": [["text": "", "annotation": "", "group": ""]]]] {
      do { _ = try MacEmojiCatalog.decode(invalid); assertionFailure("accepted invalid catalog") }
      catch {}
    }
    do { _ = try MacEmojiCatalog.load(resources: "relative", search: ""); assertionFailure("accepted relative path") }
    catch {}
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    do { _ = try MacEmojiCatalog.load(resources: directory.path, search: ""); assertionFailure("accepted missing resource") }
    catch {}
    try Data().write(to: directory.appendingPathComponent("others.db"))
    for category in ["", "kaomoji", "symbols"] {
      let loaded = try MacEmojiCatalog.load(resources: directory.path, search: "synthetic-keyword", category: category)
      assert(loaded.count == 1)
      assert(StubEmojiSession.lastRequest["category"] as? String == category)
      assert(StubEmojiSession.lastRequest["search"] as? String == "synthetic-keyword")
      assert(StubEmojiSession.lastRequest["limit"] as? Int == 255)
      assert(StubEmojiSession.lastRequest["offset"] as? Int == 0)
      _ = try MacEmojiCatalog.load(resources: directory.path, search: "synthetic-keyword", category: category, offset: 510)
      assert(StubEmojiSession.lastRequest["offset"] as? Int == 510)
      _ = try MacEmojiCatalog.load(resources: directory.path, search: "match", category: category, group: "Z")
      assert(StubEmojiSession.lastRequest["group"] as? String == "Z")
      let groups = try MacEmojiCatalog.loadGroups(resources: directory.path, category: category)
      assert(groups == ["Z", "A"])
      assert(StubEmojiSession.lastRequest["category"] as? String == category)
    }
    for invalid: Any in [[""], ["duplicate", "duplicate"], 123] {
      StubEmojiSession.groups = invalid
      do { _ = try MacEmojiCatalog.loadGroups(resources: directory.path, category: ""); assertionFailure("accepted invalid groups") }
      catch {}
    }
    let hierarchy = try MacEmojiCatalog.loadSymbolGroups(resources: directory.path)
    assert(MacEmojiSymbolGroup.parents(hierarchy) == ["P1", "P2"])
    assert(MacEmojiSymbolGroup.titles(hierarchy, parent: "") == ["Shared", "Other"])
    assert(MacEmojiSymbolGroup.titles(hierarchy, parent: "P2") == ["Shared"])
    assert(MacEmojiSymbolGroup.titles(hierarchy, parent: "missing").isEmpty)
    _ = try MacEmojiCatalog.load(resources: directory.path, search: "match", category: "symbols", group: "Shared", parent: "P2")
    assert(StubEmojiSession.lastRequest["parent"] as? String == "P2")
    StubEmojiSession.symbolGroups = [["parent": "", "title": "Invalid"]]
    do { _ = try MacEmojiCatalog.loadSymbolGroups(resources: directory.path); assertionFailure("accepted empty parent") }
    catch {}
    do { _ = try MacEmojiCatalog.load(resources: directory.path, search: "", offset: -1); assertionFailure("accepted negative offset") }
    catch {}
    try pluginChecks()
    print("Emoji catalog decoding checks passed")
  }
}

extension EmojiCatalogTest {
  static func pluginChecks() throws {
    // 插件组不依赖 others.db：这个目录里没有 others.db。
    let resources = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).path
    let plugins = "/synthetic/preferences/plugins"
    let groups = try MacEmojiCatalog.loadPluginSymbolGroups(resources: resources, plugins: plugins)
    assert(StubEmojiSession.lastRequest["list_plugin_symbol_groups"] as? Bool == true)
    assert(StubEmojiSession.lastRequest["plugins"] as? String == plugins)
    assert(StubEmojiSession.lastRequest["resources"] as? String == resources)
    assert(groups.count == 3)
    assert(groups[0] == MacEmojiPluginSymbolGroup(pack: "fixture-arrows", packName: "Arrows", tab: .symbols,
      title: "Basic", keywords: "arrow direction", items: ["←", "→"]))
    assert(groups[1].tab == .kaomoji && groups[1].keywords.isEmpty)
    do { _ = try MacEmojiCatalog.loadPluginSymbolGroups(resources: resources, plugins: "relative/plugins"); assertionFailure("accepted relative plugins") }
    catch {}
    do { _ = try MacEmojiCatalog.loadPluginSymbolGroups(resources: "relative", plugins: plugins); assertionFailure("accepted relative resources") }
    catch {}

    // 目录推导：偏好目录下的 plugins；没有或相对的偏好目录没有插件，也不发请求。
    assert(MacEmojiPluginSymbolGroup.directory(preferencesDirectory: "/synthetic/preferences") == plugins)
    assert(MacEmojiPluginSymbolGroup.directory(preferencesDirectory: "") == nil)
    assert(MacEmojiPluginSymbolGroup.directory(preferencesDirectory: "relative") == nil)
    StubEmojiSession.lastRequest = [:]
    assert(MacEmojiCatalog.installedPluginSymbolGroups(resources: resources, preferencesDirectory: "").isEmpty)
    assert(StubEmojiSession.lastRequest.count == 0)
    assert(MacEmojiCatalog.installedPluginSymbolGroups(resources: resources, preferencesDirectory: "/synthetic/preferences") == groups)

    // 符号页：没有搜索词时只给选中插件包的组，内置分类下没有插件项；有搜索词时跨包匹配关键词或文本。
    let arrows = MacEmojiPluginSymbolGroup.parentID(pack: "fixture-arrows")
    assert(MacEmojiPluginSymbolGroup.isParentID(arrows) && !MacEmojiPluginSymbolGroup.isParentID("Arrows"))
    assert(MacEmojiPluginSymbolGroup.symbolItems(groups, search: "", parent: arrows)
      == [.init(text: "←", annotation: "", group: "Basic"), .init(text: "→", annotation: "", group: "Basic")])
    assert(MacEmojiPluginSymbolGroup.symbolItems(groups, search: "", parent: "P1").isEmpty)
    assert(MacEmojiPluginSymbolGroup.symbolItems(groups, search: "", parent: "").isEmpty)
    assert(MacEmojiPluginSymbolGroup.symbolItems(groups, search: "DIRECTION", parent: "").map(\.text) == ["←", "→"])
    assert(MacEmojiPluginSymbolGroup.symbolItems(groups, search: "→", parent: "").map(\.group) == ["Basic", "Operators"])
    assert(MacEmojiPluginSymbolGroup.symbolItems(groups, search: "missing", parent: "").isEmpty)
    // 颜文字组只出现在颜文字页。
    assert(MacEmojiPluginSymbolGroup.symbolItems(groups, search: "☞", parent: "").isEmpty)
    let kaomoji = MacEmojiPluginSymbolGroup.kaomojiSections(groups, search: "")
    assert(kaomoji.map(\.title) == ["Pointing"] && kaomoji[0].items.map(\.text) == ["(☞ﾟ∀ﾟ)☞"])
    assert(MacEmojiPluginSymbolGroup.kaomojiSections(groups, search: "missing").isEmpty)

    // 插件请求失败或响应不合法时，面板拿到空列表，只显示内置目录。
    let invalid: [NSDictionary] = [["error": "unavailable"], [:], ["plugin_symbol_groups": 1],
      ["plugin_symbol_groups": [["pack": "p", "pack_name": "P", "tab": "emoji", "title": "T", "keywords": "", "items": ["x"]]]],
      ["plugin_symbol_groups": [["pack": "p", "pack_name": "P", "tab": "symbols", "title": "T", "items": ["x"]]]],
      ["plugin_symbol_groups": [["pack": "p", "pack_name": "P", "tab": "symbols", "title": "T", "keywords": "", "items": [""]]]],
      ["plugin_symbol_groups": [["pack": "", "pack_name": "P", "tab": "symbols", "title": "T", "keywords": "", "items": ["x"]]]]]
    for response in invalid {
      StubEmojiSession.pluginGroups = response
      do { _ = try MacEmojiCatalog.loadPluginSymbolGroups(resources: resources, plugins: plugins); assertionFailure("accepted invalid plugin groups") }
      catch {}
      assert(MacEmojiCatalog.installedPluginSymbolGroups(resources: resources, preferencesDirectory: "/synthetic/preferences").isEmpty)
    }
    StubEmojiSession.pluginGroups = ["plugin_symbol_groups": []]
    let none = try MacEmojiCatalog.loadPluginSymbolGroups(resources: resources, plugins: plugins)
    assert(none.isEmpty)
  }
}

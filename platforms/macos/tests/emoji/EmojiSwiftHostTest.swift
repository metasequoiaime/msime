import Foundation
import SQLite3

@main enum EmojiSwiftHostTest {
  static func main() throws {
    // No stub class: dynamic selector lookup must resolve the real linked host.
    guard NSClassFromString("MSIMEClientSession") != nil else { fatalError("Native host not linked") }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-swift-host-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    var db: OpaquePointer?
    assert(sqlite3_open(directory.appendingPathComponent("others.db").path, &db) == SQLITE_OK)
    let sql = """
      CREATE TABLE emoji(emoji TEXT,category TEXT,keywords TEXT,pinyin TEXT,sort_order INTEGER);
      CREATE TABLE kaomoji_catalog(kaomoji TEXT,keywords TEXT,sort_order INTEGER);
      CREATE TABLE symbol_catalog(symbol TEXT,category TEXT,parent_category TEXT,keywords TEXT,sort_order INTEGER);
      WITH RECURSIVE n(x) AS (SELECT 0 UNION ALL SELECT x+1 FROM n WHERE x<514)
      INSERT INTO emoji SELECT CASE WHEN x<255 THEN '' ELSE 'synthetic-same' END,'fixture','match','',x FROM n;
      INSERT INTO emoji VALUES('synthetic-tail','fixture','tail','',515);
      INSERT INTO kaomoji_catalog SELECT emoji,keywords,sort_order FROM emoji;
      INSERT INTO symbol_catalog SELECT emoji,category,'parent',keywords,sort_order FROM emoji;
      """
    assert(sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK)
    assert(sqlite3_close(db) == SQLITE_OK)
    for category in ["", "kaomoji", "symbols"] {
      let all = try MacEmojiCatalog.loadAll(resources: directory.path, search: "", category: category, group: "", parent: "")
      assert(all.count == 261 && all.first?.text == "synthetic-same" && all.last?.text == "synthetic-tail")
      assert(all.filter { $0.text == "synthetic-same" }.count == 260)
      let preview = try MacEmojiCatalog.loadPrefix(resources: directory.path, search: "", category: category, limit: 18)
      assert(preview == Array(all.prefix(18)))
      let filtered = try MacEmojiCatalog.loadAll(resources: directory.path, search: "tail", category: category, group: "", parent: "")
      assert(filtered.count == 1 && filtered[0].text == "synthetic-tail")
      let none = try MacEmojiCatalog.loadAll(resources: directory.path, search: "absent-fixture", category: category, group: "", parent: "")
      assert(none.isEmpty)
      let groups = try MacEmojiCatalog.loadGroups(resources: directory.path, category: category)
      assert(groups == [category == "kaomoji" ? "All" : "fixture"])
    }
    let symbols = try MacEmojiCatalog.loadAll(resources: directory.path, search: "tail", category: "symbols", group: "fixture", parent: "parent")
    assert(symbols.count == 1)
    let otherParent = try MacEmojiCatalog.loadAll(resources: directory.path, search: "", category: "symbols", group: "fixture", parent: "missing")
    assert(otherParent.isEmpty)
    // 符号集插件经真实宿主读取：资源目录里没有 others.db 也能列出，没有插件时是空列表。
    let bare = directory.appendingPathComponent("bare-resources")
    let plugins = directory.appendingPathComponent("plugins")
    try FileManager.default.createDirectory(at: bare, withIntermediateDirectories: false)
    let empty = try MacEmojiCatalog.loadPluginSymbolGroups(resources: bare.path, plugins: plugins.path)
    assert(empty.isEmpty)
    let pack = plugins.appendingPathComponent("symbol_set/fixture-symbols")
    try FileManager.default.createDirectory(at: pack, withIntermediateDirectories: true)
    try """
      schema_version = 1
      kind = "symbol_set"
      id = "fixture-symbols"
      name = "Fixture"
      version = "1.0.0"
      license = "CC0-1.0"

      [[groups]]
      tab = "symbols"
      title = "Arrows"
      keywords = "synthetic arrow"
      items = ["→", "←"]

      [[groups]]
      tab = "kaomoji"
      title = "Happy"
      items = ["(＾▽＾)"]
      """.write(to: pack.appendingPathComponent("plugin.toml"), atomically: true, encoding: .utf8)
    let installed = try MacEmojiCatalog.loadPluginSymbolGroups(resources: bare.path, plugins: plugins.path)
    assert(installed == [
      MacEmojiPluginSymbolGroup(pack: "fixture-symbols", packName: "Fixture", tab: .symbols, title: "Arrows", keywords: "synthetic arrow", items: ["→", "←"]),
      MacEmojiPluginSymbolGroup(pack: "fixture-symbols", packName: "Fixture", tab: .kaomoji, title: "Happy", keywords: "", items: ["(＾▽＾)"])])
    let relative = try MacEmojiCatalog.request(resources: bare.path,
      parameters: ["list_plugin_symbol_groups": true, "plugins": "relative/plugins"], requiresCatalog: false)
    assert(relative["error"] != nil)
    print("Swift dynamic host lookup, cursor collection, previews, filters and plugin symbol groups passed against native SQLite")
  }
}

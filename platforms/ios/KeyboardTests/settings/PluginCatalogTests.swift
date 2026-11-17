import XCTest

/// 「插件」页的目录、标记和偏好写法，与共享设置页 `plugin-catalog-helpers.ts`、`plugin-preferences.ts` 相同；最后一组对着真的 `msime_client_plugins` 在临时状态目录里新建、列出和删除包。
@MainActor
final class PluginCatalogTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-plugins-\(UUID().uuidString)", isDirectory: true)
    XCTAssertNoThrow(try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true))
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private func package(_ kind: String, _ id: String, extra: [String: Any] = [:]) -> [String: Any] {
    var base: [String: Any] = ["kind": kind, "id": id, "name": id.uppercased(), "version": "1.0.0", "license": "CC0-1.0",
                               "author": NSNull(), "description": NSNull(), "builtin": false]
    base.merge(extra) { $1 }
    return base
  }

  /// 只列手机上起作用的类型；旋律音效包、音乐包和特效包算进「用不上」的个数，内置的不算；脚本插件的问题行不列；指令表一组总在。
  func testCatalogListsOnlyThePhoneKinds() throws {
    let value: [String: Any] = [
      "packages": [
        package("sound", "default", extra: ["mode": "keys", "builtin": true]),
        package("sound", "twinkle", extra: ["mode": "sequence", "builtin": true]),
        package("sound", "my-melody", extra: ["mode": "sequence"]),
        package("music", "rain"),
        package("effect", "sparks"),
        package("phrase_table", "addr", extra: ["phrases": [["key": "dz", "text": "北京"]]]),
        package("symbol_set", "arrows", extra: ["groups": [["tab": "symbols", "title": "箭头", "keywords": "", "items": ["→"]]]]),
        package("future_kind", "later"),
      ],
      "issues": [["kind": "script", "folder": "x", "reason": "r"], ["kind": "wordbook", "folder": "bad", "reason": "坏了"]],
    ]
    let catalog = try XCTUnwrap(PluginCatalog(json: value))
    XCTAssertEqual(catalog.packages.count, 7, "认不出的类型不读")
    XCTAssertEqual(catalog.unusedCount, 3)
    XCTAssertEqual(catalog.issues, [PluginIssue(kind: "wordbook", folder: "bad", reason: "坏了")])
    let groups = catalog.groups(missing: [])
    XCTAssertEqual(groups.map(\.kind), [.sound, .commandTable, .phraseTable, .symbolSet])
    XCTAssertEqual(groups[0].packs.map(\.id), ["default"])
    XCTAssertEqual(groups[1].packs, [])
    XCTAssertEqual(catalog.package(kind: .phraseTable, id: "addr")?.phrases, [PluginPhraseRow(key: "dz", text: "北京")])
    XCTAssertEqual(catalog.package(kind: .symbolSet, id: "arrows")?.groups.first?.items, ["→"])
    XCTAssertEqual(catalog.package(kind: .sound, id: "default")?.label, "DEFAULT（内置）")
    XCTAssertNil(PluginCatalog(json: ["issues": []]), "读不懂的回答不当作没有插件")
  }

  func testMarkersFollowTheModesAndTheScheme() {
    var preferences = PluginPreferences(document: [
      "plugins": ["command_tables": ["sig", "other"], "phrase_tables": ["addr"], "key_sound": ["pack": "clicky"],
                  "helpcode_pack_quanpin": "custom-a", "helpcode_pack_shuangpin": "custom-a"],
      "local_modes": ["command": true],
    ])
    let command = PluginPackage(kind: .commandTable, id: "other", name: "其他")
    let phrase = PluginPackage(kind: .phraseTable, id: "addr", name: "地址")
    let sound = PluginPackage(kind: .sound, id: "clicky", name: "咔嗒")
    let helpcode = PluginPackage(kind: .helpcode, id: "custom-a", name: "码表")
    func marker(_ pack: PluginPackage, sound: Bool = true, opens: Bool = true) -> String? {
      PluginCatalogPolicy.marker(pack, preferences: preferences, soundEnabled: sound, scheme: opens ? .quanpin : .japanese)
    }
    XCTAssertEqual(marker(command), "已启用 · 第 2 位")
    XCTAssertEqual(marker(phrase), "已启用 · 第 1 位")
    XCTAssertEqual(marker(command, opens: false), "已启用 · 当前方案打不开 / 指令")
    XCTAssertEqual(marker(sound), "使用中")
    XCTAssertEqual(marker(sound, sound: false), "已选 · 按键音未开")
    XCTAssertEqual(marker(helpcode), "用于全拼和双拼")
    preferences.commandMode = false
    preferences.quickPhraseMode = false
    XCTAssertEqual(marker(command), "已启用 · / 指令未开")
    XCTAssertEqual(marker(phrase), "已启用 · K 模式未开")
    XCTAssertNil(marker(PluginPackage(kind: .wordbook, id: "w", name: "词书")))
  }

  /// 缺省值与 client-core 相同：按键音是内置的 `default`，K 模式开、/ 指令关。
  func testPreferenceDefaults() {
    let preferences = PluginPreferences(document: [:])
    XCTAssertEqual(preferences.keySoundPack, "default")
    XCTAssertTrue(preferences.quickPhraseMode)
    XCTAssertFalse(preferences.commandMode)
    XCTAssertEqual(PluginCatalogPolicy.missingSelections(preferences, packages: []).map(\.key), ["sound/default"],
                   "设置 App 找不到内置音效包时也照样列出来")
  }

  func testMissingSelectionsGroupUsesAndSpotMelodies() {
    let preferences = PluginPreferences(document: [
      "plugins": ["key_sound": ["pack": "tune"], "phrase_tables": ["gone"], "helpcode_pack_quanpin": "lost",
                  "helpcode_pack_shuangpin": "lost"],
    ])
    let packages = [PluginPackage(kind: .sound, id: "tune", name: "曲子")]
    var tune = packages[0]
    tune.mode = "sequence"
    let missing = PluginCatalogPolicy.missingSelections(preferences, packages: [tune])
    XCTAssertEqual(missing.map(\.key), ["sound/tune", "phrase_table/gone", "helpcode/lost"])
    XCTAssertTrue(missing[0].mismatched)
    XCTAssertEqual(missing[0].title, "tune（不可用）")
    XCTAssertEqual(missing[2].uses, ["全拼辅助码", "双拼辅助码"])
    XCTAssertEqual(missing[1].title, "gone（未找到）")
  }

  /// 启用排在已启用的之后，停用只去掉它；删除包后清掉选中、启用它的设置；`plugins` 里的其他键原样保留。
  func testDocumentWritesKeepTheOtherKeys() {
    var document: [String: Any] = [
      "plugins": ["key_sound": ["pack": "clicky", "volume": 70], "command_tables": ["a"], "effect_pack": "spark"],
      "local_modes": ["quick_phrase": false],
    ]
    PluginCatalogPolicy.setTable(.commandTable, id: "b", enabled: true, document: &document)
    PluginCatalogPolicy.setTable(.commandTable, id: "a", enabled: true, document: &document)
    var plugins = document["plugins"] as? [String: Any]
    XCTAssertEqual(plugins?["command_tables"] as? [String], ["b", "a"])
    XCTAssertEqual(plugins?["effect_pack"] as? String, "spark")
    PluginCatalogPolicy.setLocalMode("command", on: true, document: &document)
    XCTAssertEqual(document["local_modes"] as? [String: Bool], ["quick_phrase": false, "command": true])

    PluginCatalogPolicy.withoutRemovedPack(.sound, id: "clicky", document: &document)
    PluginCatalogPolicy.withoutRemovedPack(.commandTable, id: "b", document: &document)
    PluginCatalogPolicy.setHelpcode("h", quanpin: true, on: true, document: &document)
    PluginCatalogPolicy.withoutRemovedPack(.helpcode, id: "h", document: &document)
    plugins = document["plugins"] as? [String: Any]
    XCTAssertEqual((plugins?["key_sound"] as? [String: Any])?["pack"] as? String, "default")
    XCTAssertEqual((plugins?["key_sound"] as? [String: Any])?["volume"] as? Int, 70)
    XCTAssertEqual(plugins?["command_tables"] as? [String], ["a"])
    XCTAssertEqual(plugins?["helpcode_pack_quanpin"] as? String, "")
  }

  /// 展示顺序先按 `plugin_order`，目录里没有的和重复的键不算，没提到的包按目录原来的先后接在后面；分组里的包照这个顺序排，类型的先后不变。
  func testPluginOrderIgnoresUnknownAndDuplicateKeys() {
    let packages = [
      PluginPackage(kind: .commandTable, id: "a", name: "A"), PluginPackage(kind: .phraseTable, id: "p", name: "P"),
      PluginPackage(kind: .commandTable, id: "b", name: "B"), PluginPackage(kind: .commandTable, id: "c", name: "C"),
      PluginPackage(kind: .music, id: "rain", name: "雨声"),
    ]
    let order = ["command_table/c", "phrase_table/gone", "command_table/c", "nonsense", "command_table/a"]
    XCTAssertEqual(PluginOrderPolicy.orderedKeys(packages, order: order),
                   ["command_table/c", "command_table/a", "phrase_table/p", "command_table/b", "music/rain"])
    XCTAssertEqual(PluginOrderPolicy.orderedKeys(packages, order: []), packages.map(\.key), "没有保存过顺序时就是目录的顺序")
    let preferences = PluginPreferences(document: ["plugins": ["plugin_order": order]])
    XCTAssertEqual(preferences.pluginOrder, order)
    XCTAssertEqual(PluginPreferences(document: [:]).pluginOrder, [])
    let groups = PluginCatalog(packages: packages).groups(missing: [], order: preferences.pluginOrder)
    XCTAssertEqual(groups.map(\.kind), [.commandTable, .phraseTable])
    XCTAssertEqual(groups[0].packs.map(\.id), ["c", "a", "b"])
    XCTAssertEqual(PluginCatalog(packages: packages).groups(missing: [])[0].packs.map(\.id), ["a", "b", "c"])
  }

  /// 在一组里拖动只换这一组占着的位置，写下的是目录里全部包的顺序（含这台设备不列的类型）；`plugins` 里的其他键原样保留。
  func testReorderingWithinAGroupWritesTheFullOrder() {
    let packages = [
      PluginPackage(kind: .commandTable, id: "a", name: "A"), PluginPackage(kind: .phraseTable, id: "p", name: "P"),
      PluginPackage(kind: .commandTable, id: "b", name: "B"), PluginPackage(kind: .music, id: "rain", name: "雨声"),
      PluginPackage(kind: .commandTable, id: "c", name: "C"),
    ]
    let group = ["command_table/a", "command_table/b", "command_table/c"]
    // 把第一个拖到末尾。
    let last = PluginOrderPolicy.reordered(packages, order: [], group: group, from: IndexSet(integer: 0), to: 3)
    XCTAssertEqual(last, ["command_table/b", "phrase_table/p", "command_table/c", "music/rain", "command_table/a"])
    // 在已保存的顺序上把最后一个拖到最前。
    let saved = ["phrase_table/p", "command_table/b", "command_table/a", "stale/key"]
    let first = PluginOrderPolicy.reordered(packages, order: saved, group: ["command_table/b", "command_table/a", "command_table/c"],
                                            from: IndexSet(integer: 2), to: 0)
    XCTAssertEqual(first, ["phrase_table/p", "command_table/c", "command_table/b", "music/rain", "command_table/a"])
    // 拖回原位不变；组里有目录中没有的键时也不动。
    XCTAssertEqual(PluginOrderPolicy.reordered(packages, order: [], group: group, from: IndexSet(integer: 1), to: 2), packages.map(\.key))
    XCTAssertEqual(PluginOrderPolicy.reordered(packages, order: [], group: ["command_table/a", "command_table/zz"],
                                               from: IndexSet(integer: 0), to: 2), packages.map(\.key))
    XCTAssertEqual(PluginOrderPolicy.moved(["a", "b", "c", "d"], from: IndexSet([0, 2]), to: 4), ["b", "d", "a", "c"])
    XCTAssertEqual(PluginOrderPolicy.moved(["a", "b", "c", "d"], from: IndexSet(integer: 3), to: 1), ["a", "d", "b", "c"])

    var document: [String: Any] = ["plugins": ["command_tables": ["a"], "effect_pack": "spark", "plugin_order": saved]]
    XCTAssertTrue(PluginOrderPolicy.isWritable(first))
    PluginOrderPolicy.setOrder(first, document: &document)
    let plugins = document["plugins"] as? [String: Any]
    XCTAssertEqual(plugins?["plugin_order"] as? [String], first)
    XCTAssertEqual(plugins?["command_tables"] as? [String], ["a"], "拖动不改启用顺序")
    XCTAssertEqual(plugins?["effect_pack"] as? String, "spark")
  }

  /// client-core 不收的顺序不写：超过 256 条、有重复、不是 `kind/id`、哪一段不是合规的 id。
  func testPluginOrderThatTheCoreRejectsIsNotWritable() {
    XCTAssertTrue(PluginOrderPolicy.isWritable([]))
    XCTAssertTrue(PluginOrderPolicy.isWritable(["symbol_set/my-arrows.v2", "sound/0day"]))
    XCTAssertFalse(PluginOrderPolicy.isWritable(["sound/a", "sound/a"]))
    XCTAssertFalse(PluginOrderPolicy.isWritable(["sound"]))
    XCTAssertFalse(PluginOrderPolicy.isWritable(["sound/"]))
    XCTAssertFalse(PluginOrderPolicy.isWritable(["sound/Upper"]))
    XCTAssertFalse(PluginOrderPolicy.isWritable(["sound/-dash"]))
    XCTAssertFalse(PluginOrderPolicy.isWritable(["sound/a/b"]))
    XCTAssertFalse(PluginOrderPolicy.isWritable(["sound/" + String(repeating: "a", count: 65)]))
    XCTAssertTrue(PluginOrderPolicy.isWritable((0..<256).map { "sound/p\($0)" }))
    XCTAssertFalse(PluginOrderPolicy.isWritable((0..<257).map { "sound/p\($0)" }))
  }

  /// 删掉一个包以后顺序里它的那一条也去掉，哪一种类型都一样；顺序里没有它时不凭空写出 `plugins`。
  func testRemovingAPackDropsItsOrderEntry() {
    var document: [String: Any] = ["plugins": [
      "plugin_order": ["wordbook/w", "command_table/a", "sound/clicky", "phrase_table/a"], "command_tables": ["a"],
    ]]
    PluginCatalogPolicy.withoutRemovedPack(.wordbook, id: "w", document: &document)
    PluginCatalogPolicy.withoutRemovedPack(.commandTable, id: "a", document: &document)
    PluginCatalogPolicy.withoutRemovedPack(.music, id: "absent", document: &document)
    let plugins = document["plugins"] as? [String: Any]
    XCTAssertEqual(plugins?["plugin_order"] as? [String], ["sound/clicky", "phrase_table/a"], "同 id 的短语表不受影响")
    XCTAssertEqual(plugins?["command_tables"] as? [String], [])
    var empty: [String: Any] = [:]
    PluginCatalogPolicy.withoutRemovedPack(.symbolSet, id: "arrows", document: &empty)
    XCTAssertNil(empty["plugins"])
  }

  /// 行上和详情里的「启用」开关同一条规则：启用满 16 个之后，没启用的不能再启用，已启用的仍能停用。
  func testTableSwitchStopsAtTheLimit() {
    let full = (0..<PluginPreferences.maximumTables).map { "t\($0)" }
    let preferences = PluginPreferences(document: ["plugins": ["command_tables": full, "phrase_tables": ["addr"]]])
    let state = { (kind: PluginKind, id: String) in
      PluginCatalogPolicy.tableSwitch(PluginPackage(kind: kind, id: id, name: id), preferences: preferences)
    }
    XCTAssertTrue(state(.commandTable, "t3").enabled)
    XCTAssertFalse(state(.commandTable, "t3").full)
    XCTAssertFalse(state(.commandTable, "new").enabled)
    XCTAssertTrue(state(.commandTable, "new").full)
    XCTAssertTrue(state(.phraseTable, "addr").enabled)
    XCTAssertFalse(state(.phraseTable, "new").full)
  }

  func testErrorMessagesMatchTheSharedPage() {
    XCTAssertEqual(PluginCatalogPolicy.errorMessage(.init(code: "plugin_invalid", detail: "短语编码 DH 必须是 1 到 32 个小写字母"), fallback: "x"),
                   "插件不符合要求（短语编码 DH 必须是 1 到 32 个小写字母）。")
    XCTAssertEqual(PluginCatalogPolicy.errorMessage(.init(code: "plugin_reserved"), fallback: "x"), "这个 id 属于内置插件，不能覆盖或删除。")
    XCTAssertEqual(PluginCatalogPolicy.errorMessage(.init(code: "nope"), fallback: "导入失败，请重试。"), "导入失败，请重试。")
    var music = PluginPackage(kind: .music, id: "rain", name: "雨声", version: "2.0")
    XCTAssertEqual(PluginCatalogPolicy.importedNotice(music), "已导入音乐包「雨声」2.0。这台设备用不上音乐包，它不会出现在「我的插件」里。")
    music = PluginPackage(kind: .phraseTable, id: "addr", name: "地址")
    XCTAssertEqual(PluginCatalogPolicy.importedNotice(music), "已导入短语表「地址」1.0.0。")
  }

  /// 全拼、双拼一系的方案打得开 K 和 / 模式；五笔只能从符号键盘的 / 进 / 模式，没有本地输入菜单，进不了 K 模式；日语等方案都打不开。
  func testSchemesThatOpenTableModes() {
    for scheme in [ChineseInputScheme.quanpin, .nineKey, .shuangpin, .microsoft, .fourteenKey, .eighteenKey, .nineKeyShuangpin] {
      XCTAssertTrue(PluginCatalogPolicy.schemeOpensTableModes(scheme, kind: .phraseTable), scheme.rawValue)
      XCTAssertTrue(PluginCatalogPolicy.schemeOpensTableModes(scheme, kind: .commandTable), scheme.rawValue)
    }
    XCTAssertTrue(PluginCatalogPolicy.schemeOpensTableModes(.wubi, kind: .commandTable))
    XCTAssertFalse(PluginCatalogPolicy.schemeOpensTableModes(.wubi, kind: .phraseTable))
    for scheme in [ChineseInputScheme.japanese, .korean, .cantonese, .zhuyin, .stroke] {
      XCTAssertFalse(PluginCatalogPolicy.schemeOpensTableModes(scheme, kind: .commandTable), scheme.rawValue)
      XCTAssertNotNil(PluginCatalogPolicy.schemeTableModeNote(scheme, kind: .commandTable))
    }
    XCTAssertEqual(PluginCatalogPolicy.schemeTableModeNote(.wubi, kind: .phraseTable)?.hasSuffix("切换到全拼或双拼后才能用。"), true)
  }

  /// 社区列表的「全部」里这台设备用不上的类型不显示，但下一页的偏移照样算上它们和 Rust 跳过的条目。
  func testCommunityPagingSkipsKindsThePhoneDoesNotUse() throws {
    func item(_ id: String, _ kind: String) -> [String: Any] {
      ["id": id, "kind": kind, "plugin_id": "p-\(id)", "name": id, "description": "", "author": "作者", "version": "1.0.0",
       "license": "CC0-1.0", "size": 1, "downloads": 0, "rating_count": 0, "rating_average": 0, "owned": false,
       "my_rating": 0, "created_at": ""]
    }
    func page(_ items: [[String: Any]], more: Bool, skipped: Int) throws -> CommunityPluginPage {
      try JSONDecoder().decode(CommunityPluginPage.self, from: JSONSerialization.data(withJSONObject: [
        "plugins": items, "has_more": more, "skipped": skipped,
      ]))
    }
    var paging = CommunityPluginPaging()
    paging.accept(try page([item("a", "music"), item("b", "phrase_table"), item("c", "effect")], more: true, skipped: 2), append: false)
    XCTAssertEqual(paging.items.map(\.id), ["b"])
    XCTAssertEqual(paging.offset, 5)
    paging.accept(try page([item("b", "phrase_table"), item("d", "sound")], more: false, skipped: 0), append: true)
    XCTAssertEqual(paging.items.map(\.id), ["b", "d"])
    XCTAssertEqual(paging.offset, 7)
    XCTAssertFalse(paging.hasMore)
  }

  /// 在设置 App 里新建的短语表装进键盘读的插件目录，目录列出它和它的内容；编辑器读回来是同一份；删掉以后不再列出。
  func testAuthoredPackRoundTripsThroughTheSharedStore() throws {
    var draft = PluginEditorModel.Draft(kind: .phraseTable)
    draft.name = "测试短语"
    draft.phrases = [.init(key: "dz", text: "合成地址"), .init(key: "dh", text: "合成电话")]
    let saved = MetasequoiaInputSessionBridge.plugins(
      ["operation": "authoring_save", "pack": PluginEditorModel.pack(from: draft), "replace": false], stateRoot: state)
    guard case .success(let value) = saved, let pack = PluginPackage(json: value) else {
      return XCTFail("authoring_save 失败：\(saved)")
    }
    XCTAssertEqual(pack.kind, .phraseTable)
    XCTAssertTrue(pack.id.hasPrefix("my-"))

    guard case .success(let listed) = MetasequoiaInputSessionBridge.plugins(["operation": "catalog"], stateRoot: state),
          let catalog = PluginCatalog(json: listed) else { return XCTFail("catalog 读不出来") }
    XCTAssertEqual(catalog.package(kind: .phraseTable, id: pack.id)?.phrases.map(\.key), ["dz", "dh"])

    guard case .success(let loaded) = MetasequoiaInputSessionBridge.plugins(
      ["operation": "authoring_load", "kind": "phrase_table", "id": pack.id], stateRoot: state) else {
      return XCTFail("authoring_load 失败")
    }
    let reloaded = try XCTUnwrap(PluginEditorModel.Draft(authored: loaded))
    XCTAssertEqual(reloaded.phrases, draft.phrases)
    XCTAssertEqual(reloaded.id, pack.id)

    guard case .success = MetasequoiaInputSessionBridge.plugins(
      ["operation": "remove", "kind": "phrase_table", "id": pack.id], stateRoot: state) else { return XCTFail("remove 失败") }
    guard case .success(let after) = MetasequoiaInputSessionBridge.plugins(["operation": "catalog"], stateRoot: state) else {
      return XCTFail("catalog 读不出来")
    }
    XCTAssertNil(PluginCatalog(json: after)?.package(kind: .phraseTable, id: pack.id))
  }
}

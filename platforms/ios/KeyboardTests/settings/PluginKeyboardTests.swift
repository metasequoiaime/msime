import XCTest

/// 设置 App 装进插件目录的包在键盘里起作用：启用的指令表经「本地输入 → 指令」（/ 模式）出候选，26 键和九键都打得出触发词；启用的短语表进 K 模式；符号集的组进符号面板和颜文字页。包都用 `authoring_save` 装进临时状态目录，与设置 App 走同一条路。
@MainActor
final class PluginKeyboardTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-plugin-keyboard-\(UUID().uuidString)", isDirectory: true)
    XCTAssertNoThrow(try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true))
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// 装一个包，返回它的 id。
  private func install(_ pack: [String: Any]) throws -> String {
    var pack = pack
    pack["id"] = NSNull()
    pack["version"] = "1.0.0"
    pack["license"] = "CC0-1.0"
    pack["author"] = NSNull()
    pack["description"] = NSNull()
    let result = MetasequoiaInputSessionBridge.plugins(["operation": "authoring_save", "pack": pack, "replace": false],
                                                       stateRoot: state)
    guard case .success(let value) = result, let id = (value as? [String: Any])?["id"] as? String else {
      throw XCTSkip("authoring_save 失败：\(result)")
    }
    return id
  }

  private func update(_ change: (inout [String: Any]) -> Void) {
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state, change))
  }

  /// / 指令和其他模式一样跟着 `local_modes`，但文档里没写时是关的；菜单里的「指令」也跟着它。
  func testCommandModeIsOffUntilTurnedOn() {
    XCTAssertFalse(KeyboardViewController.enabledLocalInputModes(in: nil).contains { $0.key == "command" })
    XCTAssertTrue(KeyboardViewController.enabledLocalInputModes(in: nil).contains { $0.key == "quick_phrase" })
    let on = KeyboardViewController.enabledLocalInputModes(in: ["local_modes": ["command": true]])
    XCTAssertEqual(on.last?.trigger, "/")
    XCTAssertEqual(on.last?.title, "指令")

    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed, bridge.cancel().diagnosticText ?? "运行时初始化失败")
    XCTAssertFalse(bridge.slashOpensCommandMode(), "/ 指令默认关")
  }

  func testEnabledCommandTableAnswersInTheSlashModeOnBothLayouts() throws {
    let id = try install(["kind": "command_table", "name": "合成指令",
                          "commands": [["trigger": "zqm", "title": "签名", "template": "合成签名"]]])
    update {
      PluginCatalogTestSupport.enable(id, key: "command_tables", in: &$0)
      var modes = $0["local_modes"] as? [String: Any] ?? [:]
      modes["command"] = true
      $0["local_modes"] = modes
    }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed, bridge.cancel().diagnosticText ?? "运行时初始化失败")
    XCTAssertTrue(bridge.slashOpensCommandMode())

    for nineKey in [false, true] {
      if nineKey { _ = bridge.switchToNineKey() }
      XCTAssertTrue(bridge.slashOpensCommandMode(), "九键：\(nineKey)")
      var snapshot = bridge.handlePunctuation("/")
      XCTAssertEqual(snapshot.localMode, "command", "九键：\(nineKey)")
      // 进了本地模式，九键的键盘换成 26 个字母，键盘照样把字母交给会话。
      for letter in "zqm" { snapshot = bridge.handleCharacter(String(letter)) }
      let index = try XCTUnwrap(snapshot.candidates.firstIndex(of: "合成签名"), "九键 \(nineKey)：\(snapshot.candidates)")
      XCTAssertEqual(bridge.selectCandidate(at: UInt(index)).commitText, "合成签名")
      XCTAssertFalse(bridge.isInLocalMode)
    }

    // 英文标点下 / 只是一个斜杠，菜单不打开模式。
    bridge.setChinesePunctuation(false)
    XCTAssertFalse(bridge.slashOpensCommandMode())
  }

  /// 设置 App 里打开「启用」以后，已经打开的键盘重读设置就用上这个短语表。
  func testEnabledPhraseTableAnswersInQuickPhraseMode() async throws {
    let id = try install(["kind": "phrase_table", "name": "合成短语",
                          "phrases": [["key": "zqdz", "text": "合成地址"]]])
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed, bridge.cancel().diagnosticText ?? "运行时初始化失败")
    XCTAssertFalse(quickPhrase(bridge, "zqdz").contains("合成地址"), "没启用的短语表不出候选")

    update { PluginCatalogTestSupport.enable(id, key: "phrase_tables", in: &$0) }
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")
    XCTAssertTrue(quickPhrase(bridge, "zqdz").contains("合成地址"))
  }

  /// 打开 K 模式输入 `code` 时的候选，之后放弃。
  private func quickPhrase(_ bridge: MetasequoiaInputSessionBridge, _ code: String) -> [String] {
    _ = bridge.cancel()
    var snapshot = bridge.openLocalMode("K")
    XCTAssertEqual(snapshot.localMode, "quick_phrase")
    for letter in code { snapshot = bridge.handleCharacter(String(letter)) }
    _ = bridge.cancel()
    return snapshot.candidates
  }

  /// 符号集的 `symbols` 组以插件名为分类排在符号面板的内置分类之后，`kaomoji` 组排在颜文字的 All 之后；搜索按组的关键词找。
  func testSymbolSetGroupsReachThePanels() throws {
    _ = try install(["kind": "symbol_set", "name": "合成符号", "groups": [
      ["tab": "symbols", "title": "箭头", "keywords": "jiantou", "items": ["⇐", "⇒"]],
      ["tab": "symbols", "title": "星", "keywords": NSNull(), "items": ["✩"]],
      ["tab": "kaomoji", "title": "开心", "keywords": "kaixin", "items": ["(＾▽＾)"]],
    ]])
    let plugins = state.appendingPathComponent("plugins").path
    let groups = KeyboardEmojiCatalog.pluginSymbolGroups(resources: state.path, plugins: plugins)
    XCTAssertEqual(groups.map(\.title), ["箭头", "星", "开心"])
    let categories = KeyboardEmojiCatalog.pluginSymbolCategories(groups)
    XCTAssertEqual(categories.map(\.title), ["合成符号"])
    XCTAssertEqual(categories.first?.symbols, ["⇐", "⇒", "✩"])
    XCTAssertEqual(KeyboardEmojiCatalog.pluginKaomojiGroups(groups).map(\.title), ["开心"])
    XCTAssertEqual(groups[0].matches("JianTou"), ["⇐", "⇒"])
    XCTAssertEqual(groups[1].matches("✩"), ["✩"])
    XCTAssertEqual(groups[1].matches("jiantou"), [])
    XCTAssertEqual(KeyboardEmojiCatalog.pluginSymbolGroups(resources: state.path, plugins: "plugins"), [], "相对路径不读")

    let builtIn = KeyboardSymbolPanelView(catalog: .init(parents: { [] }, symbols: { _ in [] }), onInsert: { _ in },
                                          onDelete: {}, onClose: {})
    let withPlugins = KeyboardSymbolPanelView(catalog: .init(parents: { [] }, symbols: { _ in [] }, pluginGroups: { groups }),
                                              onInsert: { _ in }, onDelete: {}, onClose: {})
    XCTAssertEqual(withPlugins.categoryCount, builtIn.categoryCount + 1)
    let button = try XCTUnwrap(Self.button(in: withPlugins, identifier: "symbolCategory_\(builtIn.categoryCount)"))
    XCTAssertEqual(button.title(for: .normal), "合成符号")
    button.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(withPlugins.shownSymbols, ["⇐", "⇒", "✩"])
  }

  /// 坏组跳过，不让整份作废。
  func testMalformedPluginGroupsAreSkipped() {
    let groups = KeyboardEmojiCatalog.decodePluginSymbolGroups(["plugin_symbol_groups": [
      ["pack": "a", "pack_name": "甲", "tab": "symbols", "title": "好", "keywords": "", "items": ["→", 3, ""]],
      ["pack": "a", "pack_name": "甲", "tab": "emoji", "title": "坏", "keywords": "", "items": ["x"]],
      ["pack": "", "pack_name": "甲", "tab": "symbols", "title": "坏", "keywords": "", "items": ["x"]],
      ["pack": "a", "pack_name": "甲", "tab": "kaomoji", "title": "空", "keywords": "", "items": []],
    ]])
    XCTAssertEqual(groups.map(\.title), ["好"])
    XCTAssertEqual(groups.first?.items, ["→"])
  }

  private static func button(in view: UIView, identifier: String) -> UIButton? {
    if let button = view as? UIButton, button.accessibilityIdentifier == identifier { return button }
    for subview in view.subviews {
      if let found = button(in: subview, identifier: identifier) { return found }
    }
    return nil
  }
}

enum PluginCatalogTestSupport {
  /// 把包排进 `plugins.<key>`（`command_tables` 或 `phrase_tables`），其他键保留。
  static func enable(_ id: String, key: String, in document: inout [String: Any]) {
    var plugins = document["plugins"] as? [String: Any] ?? [:]
    plugins[key] = (plugins[key] as? [String] ?? []) + [id]
    document["plugins"] = plugins
  }
}

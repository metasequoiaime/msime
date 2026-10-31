import XCTest

/// 九键左侧符号栏自定义（#10223）的规则和编辑页：上限与 client-core、Android 相同，规范化与 client-core `normalize_nine_key_sidebar_symbols` 逐项相同，编辑页的整表校验、加入、删除、排序、恢复默认和保存（与出厂表相同时写空数组）。三端的常量逐项相同由 scripts/test-nine-key-sidebar-parity.py 核对。
@MainActor
final class NineKeySidebarSettingsTests: XCTestCase {
  override func setUp() {
    super.setUp()
    // 保存会写 App Group 镜像，结束后放回原样。
    let mirror = TouchKeyLayoutStyle.mirrored
    addTeardownBlock { TouchKeyLayoutStyle.mirrored = mirror }
  }

  /// 上限、出厂表、标题和说明。
  func testConstants() {
    XCTAssertEqual(NineKeySidebar.maxSymbols, 30)
    XCTAssertEqual(NineKeySidebar.maxSymbolCodePoints, 4)
    XCTAssertEqual(NineKeySidebar.maxTextUTF16, 200)
    XCTAssertEqual(NineKeySidebarTable.letter.defaults, ["，", "。", "？"])
    XCTAssertEqual(NineKeySidebarTable.digit.defaults, ["+", "-", "*", "/"])
    XCTAssertEqual(NineKeySidebarTable.letter.title, "九键左侧符号")
    XCTAssertEqual(NineKeySidebarTable.digit.title, "数字键盘左侧符号")
    XCTAssertEqual(NineKeySidebarTable.letter.note, "拼音九键、九键双拼和笔画共用，多于一屏时上下滑动。")
    XCTAssertEqual(NineKeySidebarTable.digit.note, "所有九宫格数字键盘共用，#+= 固定在最后。")
    XCTAssertEqual(NineKeySidebarTable.letter.documentKey, "touch_nine_key_sidebar_symbols")
    XCTAssertEqual(NineKeySidebarTable.digit.documentKey, "touch_nine_key_digit_sidebar_symbols")
    XCTAssertEqual(NineKeySidebar.addableSymbols.count, 51)
    XCTAssertEqual(NineKeySidebar.addableSymbols.first, "，")
    XCTAssertEqual(NineKeySidebar.addableSymbols.last, ":-)")
    XCTAssertEqual(Set(NineKeySidebar.addableSymbols).count, NineKeySidebar.addableSymbols.count)
    XCTAssertTrue(NineKeySidebar.isValid(NineKeySidebar.addableSymbols.prefix(30).map { $0 }), "every addable symbol fits")
  }

  /// 逐项规范化，用例照抄 client-core 的 `nine_key_sidebar_symbols_normalize_item_by_item`。
  func testNormalizationMatchesClientCore() {
    XCTAssertEqual(NineKeySidebar.normalize([
      " ， ", "", "  ", "a b", "\u{3000}", "\u{7}", "。\u{0}", "12345", "……", "，", ":-)", "👍🏽", "👨‍👩‍👧",
    ]), ["，", "……", ":-)", "👍🏽"])
    XCTAssertEqual(NineKeySidebar.normalize(["abcd", "abcde"]), ["abcd"])
    XCTAssertEqual(NineKeySidebar.normalize(["", "toolong"]), [])
    let many = (0..<40).map(String.init)
    XCTAssertEqual(NineKeySidebar.normalize(many), Array(many.prefix(30)))
    // 25 个四码位的增补平面字符串，每项 8 个 UTF-16 单元，连上空格第 23 项会到 206。
    let wide = (0..<25).map { String(repeating: String(UnicodeScalar(0x1F600 + $0)!), count: 4) }
    let normalized = NineKeySidebar.normalize(wide)
    XCTAssertEqual(normalized, Array(wide.prefix(22)))
    XCTAssertEqual(normalized.joined(separator: " ").utf16.count, 22 * 8 + 21)
    // 不间断空格、全角空格夹在中间的项不收，首尾的去掉。
    XCTAssertEqual(NineKeySidebar.normalize(["a\u{00A0}b", "\u{00A0}、\u{3000}"]), ["、"])
    // 按码位比较：组合前后的 é 是两项，与 client-core 比较字节相同。
    XCTAssertEqual(NineKeySidebar.normalize(["\u{E9}", "e\u{301}"]).count, 2)
    // 文档里的值：不是字符串的元素丢掉，不是数组时是空表。
    XCTAssertEqual(NineKeySidebar.symbols(in: ["？", 1, NSNull(), "toolong", "？", ["！"], "！"] as [Any]), ["？", "！"])
    XCTAssertEqual(NineKeySidebar.symbols(in: "， 。"), [])
    XCTAssertEqual(NineKeySidebar.symbols(in: nil), [])
    // 文档里存着与 iOS 出厂表逐项相同的非空表时按出厂表读（空表），与 Android `NineKeySidebarPolicy.custom` 相同；换了顺序就是自定义表。
    let factory = TouchKeyLayoutStyle(in: [
      TouchKeyLayoutStyle.nineKeySidebarDocumentKey: ["，", "。", "？"],
      TouchKeyLayoutStyle.digitSidebarDocumentKey: ["+", "-", "*", "/"],
    ])
    XCTAssertEqual(factory.nineKeySidebar, [])
    XCTAssertEqual(factory.digitSidebar, [])
    XCTAssertEqual(TouchKeyLayoutStyle(in: [TouchKeyLayoutStyle.nineKeySidebarDocumentKey: ["。", "，", "？"]]).nineKeySidebar,
                   ["。", "，", "？"])
  }

  /// 编辑页的整表校验：空表、超过 30 项、有重复、任何一项不合规、空格连起来超过 200 个 UTF-16 单元时不能保存。合规的表规范化后不变，client-core 才收。
  func testWholeTableValidation() {
    XCTAssertTrue(NineKeySidebar.isValid(["，"]))
    XCTAssertTrue(NineKeySidebar.isValid(["，", "……", ":-)", "👍🏽", "#+="]))
    XCTAssertFalse(NineKeySidebar.isValid([]))
    XCTAssertFalse(NineKeySidebar.isValid(["，", "，"]))
    XCTAssertFalse(NineKeySidebar.isValid([" ，"]))
    XCTAssertFalse(NineKeySidebar.isValid(["a b"]))
    XCTAssertFalse(NineKeySidebar.isValid(["\u{7}"]))
    XCTAssertFalse(NineKeySidebar.isValid(["abcde"]))
    XCTAssertFalse(NineKeySidebar.isValid(["👨‍👩‍👧"]))
    XCTAssertFalse(NineKeySidebar.isValid((0..<31).map(String.init)))
    XCTAssertTrue(NineKeySidebar.isValid((0..<30).map(String.init)))
    let wide = (0..<23).map { String(repeating: String(UnicodeScalar(0x1F600 + $0)!), count: 4) }
    XCTAssertTrue(NineKeySidebar.isValid(Array(wide.prefix(22))))
    XCTAssertFalse(NineKeySidebar.isValid(wide), "206 UTF-16 units when joined")
    for symbols in [["，"], ["，", "……", ":-)", "👍🏽"], (0..<30).map(String.init), Array(wide.prefix(22))] {
      XCTAssertEqual(NineKeySidebar.normalize(symbols), symbols)
    }
  }

  /// 摘要照 Android `summary(symbols, 4)`；成对符号照 Android `literalPairClosing`；一屏放几项照 Android `rowHeight`。
  func testSummaryPairsAndRowsPerScreen() {
    XCTAssertEqual(NineKeySidebar.summary(["，", "。", "？"]), "， 。 ？")
    XCTAssertEqual(NineKeySidebar.summary(["+", "-", "*", "/"]), "+ - * /")
    XCTAssertEqual(NineKeySidebar.summary(["，", "。", "？", "！", "、", "；", "：", "…"]), "， 。 ？ ！ 等 8 个")
    XCTAssertEqual(NineKeySidebar.literalPairClosing("（）"), "）")
    XCTAssertEqual(NineKeySidebar.literalPairClosing("()"), ")")
    XCTAssertEqual(NineKeySidebar.literalPairClosing("【】"), "】")
    XCTAssertEqual(NineKeySidebar.literalPairClosing("“”"), "”")
    XCTAssertNil(NineKeySidebar.literalPairClosing("（"))
    XCTAssertNil(NineKeySidebar.literalPairClosing("(a)"))
    XCTAssertNil(NineKeySidebar.literalPairClosing(":-)"))
    XCTAssertNil(NineKeySidebar.literalPairClosing("\"\""), "ASCII quotes are not a pair, as on Android")
    XCTAssertNil(NineKeySidebar.literalPairClosing("<>"), "ASCII < is a less-than sign, as on Android")
    XCTAssertEqual(NineKeySidebar.visibleRows(count: 5, limit: 3, height: 150), 3)
    XCTAssertEqual(NineKeySidebar.visibleRows(count: 2, limit: 3, height: 150), 2)
    XCTAssertEqual(NineKeySidebar.visibleRows(count: 1, limit: 3, height: 150), 1)
    XCTAssertEqual(NineKeySidebar.visibleRows(count: 5, limit: 3, height: 70), 2, "rows would drop under 28pt")
    XCTAssertEqual(NineKeySidebar.visibleRows(count: 5, limit: 3, height: 10), 1)
    XCTAssertEqual(NineKeySidebar.visibleRows(count: 0, limit: 3, height: 150), 0)
  }

  /// 编辑页从当前表起步（空表时是出厂表）；加入时挡掉重复、不合规和放不下的项；可删除、可调整顺序、可恢复默认；与出厂表相同时写空数组。
  func testDraftEditsTheTable() {
    var draft = NineKeySidebarDraft(table: .letter, style: TouchKeyLayoutStyle())
    XCTAssertEqual(draft.symbols, ["，", "。", "？"])
    XCTAssertTrue(draft.isDefault)
    XCTAssertEqual(draft.stored, [])
    XCTAssertFalse(draft.addable.contains("，"))
    XCTAssertEqual(draft.addable.first, "！")
    XCTAssertFalse(draft.add("，"), "a duplicate is refused when it is added")
    XCTAssertFalse(draft.add("abcde"))
    XCTAssertFalse(draft.add("a b"))
    XCTAssertFalse(draft.add(""))
    XCTAssertTrue(draft.add(" 、 "), "surrounding spaces are trimmed")
    XCTAssertTrue(draft.add("（）"))
    XCTAssertEqual(draft.symbols, ["，", "。", "？", "、", "（）"])
    XCTAssertFalse(draft.isDefault)
    draft.move(from: IndexSet(integer: 4), to: 0)
    XCTAssertEqual(draft.symbols, ["（）", "，", "。", "？", "、"])
    draft.move(from: IndexSet([0, 1]), to: 5)
    XCTAssertEqual(draft.symbols, ["。", "？", "、", "（）", "，"])
    draft.remove(at: IndexSet([0, 4]))
    XCTAssertEqual(draft.symbols, ["？", "、", "（）"])
    XCTAssertEqual(draft.stored, ["？", "、", "（）"])
    draft.remove(at: IndexSet(0..<3))
    XCTAssertFalse(draft.canSave, "an empty table cannot be saved")
    draft.restoreDefaults()
    XCTAssertEqual(draft.symbols, ["，", "。", "？"])
    XCTAssertTrue(draft.canSave)
    XCTAssertEqual(draft.stored, [])
    // 放满 30 项后不能再加。
    var full = NineKeySidebarDraft(table: .digit, style: TouchKeyLayoutStyle(digitSidebar: (0..<30).map(String.init)))
    XCTAssertEqual(full.symbols.count, 30)
    XCTAssertFalse(full.canAdd("+"))
    XCTAssertFalse(full.add("+"))
    // 自定义的表从文档里存着的样子起步。
    let digits = NineKeySidebarDraft(table: .digit, style: TouchKeyLayoutStyle(digitSidebar: ["=", "%"]))
    XCTAssertEqual(digits.symbols, ["=", "%"])
    XCTAssertEqual(NineKeySidebarDraft(table: .digit, style: TouchKeyLayoutStyle()).symbols, ["+", "-", "*", "/"])
  }

  /// #10396：每行前的序号 1..N 跟着顺序变；行的标识按码位、随符号走，组合前后的 é 是两行。
  func testRowsNumberTheCurrentOrder() {
    XCTAssertEqual(NineKeySidebar.ordinal(0), "1.")
    XCTAssertEqual(NineKeySidebar.ordinal(29), "30.")
    var draft = NineKeySidebarDraft(table: .letter, style: TouchKeyLayoutStyle())
    XCTAssertEqual(draft.rows.map(\.ordinal), ["1.", "2.", "3."])
    XCTAssertEqual(draft.rows.map(\.accessibilityLabel), ["第 1 项，符号 ，", "第 2 项，符号 。", "第 3 项，符号 ？"])
    let before = Dictionary(uniqueKeysWithValues: draft.rows.map { ($0.symbol, $0.id) })
    draft.move(from: IndexSet(integer: 2), to: 0)
    XCTAssertEqual(draft.rows.map(\.symbol), ["？", "，", "。"])
    XCTAssertEqual(draft.rows.map(\.ordinal), ["1.", "2.", "3."])
    XCTAssertEqual(draft.rows.map(\.id), draft.rows.compactMap { before[$0.symbol] }, "a row keeps its id when it moves")
    XCTAssertTrue(draft.add("\u{E9}"))
    XCTAssertTrue(draft.add("e\u{301}"), "the two forms of é are two entries, as in client-core")
    XCTAssertEqual(Set(draft.rows.map(\.id)).count, draft.rows.count, "every row has its own id")
    draft.remove(at: IndexSet(integer: 0))
    XCTAssertEqual(draft.rows.map(\.ordinal), ["1.", "2.", "3.", "4."])
  }

  /// 编辑页保存的那一步：只写这一张表，写文档也写镜像；与出厂表相同时写空数组，键盘照出厂表画。
  func testDraftSavesThroughTheSharedDocument() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    var draft = NineKeySidebarDraft(table: .digit, style: TouchKeyLayoutStyle())
    draft.add("=")
    XCTAssertTrue(TouchKeyLayoutStyle.save(sidebar: draft.symbols, table: draft.table, stateRoot: state))
    var document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["touch_nine_key_digit_sidebar_symbols"] as? [String], ["+", "-", "*", "/", "="])
    XCTAssertNil(document["touch_nine_key_sidebar_symbols"])
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.digitSidebar, ["+", "-", "*", "/", "="])
    draft.restoreDefaults()
    XCTAssertTrue(TouchKeyLayoutStyle.save(sidebar: draft.symbols, table: draft.table, stateRoot: state))
    document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertTrue((document["touch_nine_key_digit_sidebar_symbols"] as? [Any])?.isEmpty ?? true)
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.digitSidebar, [])
    XCTAssertEqual(TouchKeyLayoutStyle.mirrored.sidebarSymbols(.digit), ["+", "-", "*", "/"])
  }
}

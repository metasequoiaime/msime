import XCTest

/// 设置 App 的「五笔查码」（#6789）：App 进程里经 `msime_client_prepare_host` 和词库请求 `wubi_codes` 读随包码表，排法与共享设置页一致。
final class WubiCodeLookupTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-wubi-lookup-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// 测试宿主带着和键盘扩展同一套 `EngineResources`，查到的是随包词库里的编码：段、舞两张表的全码不一样，所以两列各自的来源都验到了；「输入法」整段也有一行，排在逐字之前。
  func testThePackagedTablesAnswerBothColumns() throws {
    let lookup = try MetasequoiaInputSessionBridge.wubiCodes("段舞", stateRoot: state)
    XCTAssertNil(lookup.phrase, "段舞 is not a word in either table")
    XCTAssertEqual(lookup.characters.map(\.text), ["段", "舞"])
    XCTAssertEqual(lookup.characters[0].wubi86.last, "wdmc")
    XCTAssertEqual(lookup.characters[0].wubi98.last, "thdc")
    XCTAssertEqual(lookup.characters[1].wubi86.last, "rlgh")
    XCTAssertEqual(lookup.characters[1].wubi98.last, "tglg")
    for item in lookup.characters {
      XCTAssertEqual(item.wubi86, item.wubi86.sorted { $0.count < $1.count }, "short codes come first")
    }
    XCTAssertFalse(lookup.truncated)

    let phrase = try MetasequoiaInputSessionBridge.wubiCodes("输入法", stateRoot: state)
    XCTAssertEqual(phrase.phrase?.text, "输入法")
    XCTAssertEqual(phrase.phrase?.wubi86, ["ltif"])
    XCTAssertEqual(phrase.rows.map(\.text), ["输入法", "输", "入", "法"])

    let none = try MetasequoiaInputSessionBridge.wubiCodes("abc，123", stateRoot: state)
    XCTAssertTrue(none.rows.isEmpty)
  }

  func testTheAnswerShapeIsCheckedAtTheBoundary() throws {
    let lookup = try WubiCodeLookup.decode([
      "phrase": NSNull(),
      "characters": [["text": "你", "wubi86": ["wq", "wqiy"], "wubi98": []]],
      "truncated": true,
    ])
    XCTAssertNil(lookup.phrase)
    XCTAssertEqual(lookup.characters, [WubiCodeLookup.Codes(text: "你", wubi86: ["wq", "wqiy"], wubi98: [])])
    XCTAssertEqual(lookup.truncationNotice(), "字太多，只列出了前 1 个不同的字。")
    XCTAssertThrowsError(try WubiCodeLookup.decode(["characters": [], "truncated": "no"]))
    XCTAssertThrowsError(try WubiCodeLookup.decode(["characters": [["text": "你", "wubi86": [1], "wubi98": []]],
                                                    "truncated": false]))
    XCTAssertThrowsError(try WubiCodeLookup.decode(["phrase": ["text": "你好"], "characters": [], "truncated": false]))
  }

  /// 与共享设置页同样的列标题、占位和读屏文字。
  func testColumnsFollowTheSharedPage() {
    let duan = WubiCodeLookup.Codes(text: "段", wubi86: ["wdm", "wdmc"], wubi98: [])
    XCTAssertEqual(WubiCodeLookup.columnTitle("wubi86", profile: "wubi86"), "86 五笔（当前）")
    XCTAssertEqual(WubiCodeLookup.columnTitle("wubi98", profile: "wubi86"), "98 五笔")
    XCTAssertEqual(WubiCodeLookup.columnTitle("wubi98", profile: "wubi98"), "98 五笔（当前）")
    XCTAssertEqual(WubiCodeLookup.columnTitle("wubi86", profile: "wubi06"), "86 五笔（当前）")
    XCTAssertEqual(WubiCodeLookup.codes([]), "—")
    XCTAssertEqual(WubiCodeLookup.spoken(duan, profile: "wubi98"), "段，86 五笔 wdm wdmc，98 五笔（当前） —")
    XCTAssertFalse(WubiCodeLookup.searchable(" \n"))
    XCTAssertTrue(WubiCodeLookup.searchable(" 段 "))
  }
}

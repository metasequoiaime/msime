import XCTest

/// 插件编辑器的检查与整理，逐条对照共享设置页 `plugin-editor-model.ts` 的提示文字和行号。
final class PluginEditorModelTests: XCTestCase {
  private func phraseDraft(_ rows: [PluginEditorModel.Phrase]) -> PluginEditorModel.Draft {
    var draft = PluginEditorModel.Draft(kind: .phraseTable)
    draft.name = "常用"
    draft.phrases = rows
    return draft
  }

  func testNewDraftStartsWithDefaultsAndOneBlankRow() {
    let draft = PluginEditorModel.Draft(kind: .commandTable)
    XCTAssertNil(draft.id)
    XCTAssertEqual(draft.version, "1.0.0")
    XCTAssertEqual(draft.license, "CC0-1.0")
    XCTAssertEqual(draft.commands, [PluginEditorModel.Command()])
    XCTAssertEqual(draft.phrases, [])
    XCTAssertEqual(PluginEditorModel.issue(draft), "请给插件起个名字。")
  }

  func testBasicInformationIsCheckedBeforeTheContent() {
    var draft = phraseDraft([.init(key: "dz", text: "北京")])
    XCTAssertNil(PluginEditorModel.issue(draft))
    draft.name = String(repeating: "长", count: 27)
    XCTAssertEqual(PluginEditorModel.issue(draft), "名字太长了。")
    draft.name = "常用"
    draft.version = " "
    XCTAssertEqual(PluginEditorModel.issue(draft), "请填写版本，例如 1.0.0。")
    draft.version = "1.0.0"
    draft.license = "CC0 1.0 ✓"
    XCTAssertEqual(PluginEditorModel.issue(draft), "许可证只能写 SPDX 许可证代码，例如 CC0-1.0、CC-BY-4.0。")
    draft.license = ""
    XCTAssertEqual(PluginEditorModel.issue(draft), "请填写许可证，不确定时写 CC0-1.0。")
    draft.license = "MIT OR Apache-2.0"
    draft.description = "第一行\u{0007}"
    XCTAssertEqual(PluginEditorModel.issue(draft), "介绍太长了。")
  }

  /// 行号按表单里的行数算，整行都空着的行既不算问题也不保存。
  func testPhraseRowsNameTheirRow() {
    XCTAssertEqual(PluginEditorModel.issue(phraseDraft([.init(), .init(key: "DZ", text: "北京")])),
                   "第 2 条短语的编码只能是 1 到 32 个小写字母。")
    XCTAssertEqual(PluginEditorModel.issue(phraseDraft([.init(key: "dz", text: "")])), "第 1 条短语还没有填写文字。")
    XCTAssertEqual(PluginEditorModel.issue(phraseDraft([.init(key: "dz", text: "北京"), .init(key: " dz ", text: "北京 ")])),
                   "第 2 条短语和前面的一条重复了。")
    XCTAssertEqual(PluginEditorModel.issue(phraseDraft([.init(key: "dz", text: String(repeating: "字", count: 200))])),
                   "第 1 条短语的文字太长了。")
    XCTAssertEqual(PluginEditorModel.issue(phraseDraft([.init()])), "至少要有一条短语。")
  }

  func testCommandTemplatesOnlyKnowThreePlaceholders() {
    var draft = PluginEditorModel.Draft(kind: .commandTable)
    draft.name = "签名"
    draft.commands = [.init(trigger: "sig", title: "签名", template: "{date:%m月%d日} {weekday} 张三")]
    XCTAssertNil(PluginEditorModel.issue(draft))
    draft.commands[0].template = "{name}"
    XCTAssertEqual(PluginEditorModel.issue(draft), "第 1 条指令的花括号里只能写 {date}、{time} 或 {weekday}。")
    draft.commands[0].template = "{date"
    XCTAssertNotNil(PluginEditorModel.issue(draft))
    draft.commands = [.init(trigger: "sig", title: "签名", template: "张三"), .init(trigger: "sig", title: "又一个", template: "李四")]
    XCTAssertEqual(PluginEditorModel.issue(draft), "指令 /sig 重复了。")
    XCTAssertFalse(PluginEditorModel.templatePlaceholdersKnown("}{"))
    XCTAssertFalse(PluginEditorModel.templatePlaceholdersKnown("{{date}}"))
    XCTAssertTrue(PluginEditorModel.templatePlaceholdersKnown("{time}"))
  }

  func testSymbolGroupsAndWords() {
    var symbols = PluginEditorModel.Draft(kind: .symbolSet)
    symbols.name = "箭头"
    symbols.groups = [.init(tab: "symbols", title: "箭头", keywords: "", items: "→\n←\n\n"),
                      .init(tab: "kaomoji", title: "箭头", keywords: "", items: "(=^_^=)")]
    XCTAssertNil(PluginEditorModel.issue(symbols), "不同的页可以同名")
    symbols.groups[1].tab = "symbols"
    XCTAssertEqual(PluginEditorModel.issue(symbols), "第 2 组的标题和同一页的另一组重复了。")
    symbols.groups = [.init(tab: "symbols", title: "箭头", keywords: "", items: "→\n→")]
    XCTAssertEqual(PluginEditorModel.issue(symbols), "第 1 组里「→」重复了。")

    var words = PluginEditorModel.Draft(kind: .wordbook)
    words.name = "我的词书"
    words.words = [.init(word: "#cache", phonetic: "", gloss: "缓存")]
    XCTAssertEqual(PluginEditorModel.issue(words), "第 1 个单词不能以 # 开头。")
    words.words = [.init(word: "cache", phonetic: "", gloss: "缓存"), .init(word: "cache", phonetic: "", gloss: "又一个")]
    XCTAssertEqual(PluginEditorModel.issue(words), "「cache」出现了不止一次。")
    words.name = String(repeating: "词", count: 27)
    XCTAssertEqual(PluginEditorModel.issue(words), "名字太长了。")
  }

  func testBulkLinesParseLikeTheSharedPage() {
    let phrases = PluginEditorModel.parsePhraseLines("dz 北京市 海淀区\r\n\n  dh\t电话  \n")
    XCTAssertNil(phrases.issue)
    XCTAssertEqual(phrases.rows, [.init(key: "dz", text: "北京市 海淀区"), .init(key: "dh", text: "电话")])
    XCTAssertEqual(PluginEditorModel.parsePhraseLines("dz 北京\nonly").issue, "第 2 行要写成「编码 文字」，中间用空格隔开。")

    let words = PluginEditorModel.parseWordLines("# 单词\t音标\t释义\ncache\tn. 缓存\nalgorithm\t/x/\tn. 算法\n")
    XCTAssertNil(words.issue)
    XCTAssertEqual(words.rows, [.init(word: "cache", phonetic: "", gloss: "n. 缓存"),
                                .init(word: "algorithm", phonetic: "/x/", gloss: "n. 算法")])
    XCTAssertEqual(PluginEditorModel.parseWordLines("a\tb\tc\td").issue,
                   "第 1 行要写成「单词、释义」两列或「单词、音标、释义」三列，列之间用 Tab 隔开。")

    let replaced = PluginEditorModel.appendRows([PluginEditorModel.Phrase()], phrases.rows) { $0.isBlank }
    XCTAssertEqual(replaced, phrases.rows)
    let appended = PluginEditorModel.appendRows([PluginEditorModel.Phrase(key: "a", text: "甲")], phrases.rows) { $0.isBlank }
    XCTAssertEqual(appended.count, 3)
  }

  /// 交给 `authoring_save` 的包：字段去掉首尾空白，可选字段留空时是 null，空行不保存，只带与类型对应的内容。
  func testPackFromDraftTrimsAndDropsBlankRows() throws {
    var draft = PluginEditorModel.Draft(kind: .wordbook)
    draft.name = " 词书 "
    draft.author = "  "
    draft.words = [.init(word: " cache ", phonetic: " ", gloss: " 缓存 "), .init()]
    let pack = PluginEditorModel.pack(from: draft)
    XCTAssertEqual(pack["kind"] as? String, "wordbook")
    XCTAssertTrue(pack["id"] is NSNull)
    XCTAssertEqual(pack["name"] as? String, "词书")
    XCTAssertTrue(pack["author"] is NSNull)
    XCTAssertNil(pack["phrases"])
    let words = try XCTUnwrap(pack["words"] as? [[String: Any]])
    XCTAssertEqual(words.count, 1)
    XCTAssertEqual(words[0]["word"] as? String, "cache")
    XCTAssertTrue(words[0]["phonetic"] is NSNull)
    XCTAssertEqual(words[0]["gloss"] as? String, "缓存")
    XCTAssertTrue(JSONSerialization.isValidJSONObject(pack))
  }

  func testSaveNoteNamesTheMissingStep() {
    XCTAssertEqual(PluginEditorModel.saveNote(kind: .commandTable, enabled: true, modeOn: true), "已保存，修改已经生效。")
    XCTAssertEqual(PluginEditorModel.saveNote(kind: .commandTable, enabled: false, modeOn: false),
                   "已保存。打开下面的「启用」，再在「输入 → 快捷模式」打开 / 指令，这些指令才能用。")
    XCTAssertEqual(PluginEditorModel.saveNote(kind: .symbolSet, enabled: false, modeOn: false), "已保存，符号面板里会显示这些符号。")
  }
}

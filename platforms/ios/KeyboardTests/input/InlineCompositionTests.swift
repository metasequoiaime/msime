import XCTest

/// 行内预编辑 keeps the host's marked text equal to the composition, and context reads look past it.
final class InlineCompositionTests: XCTestCase {
  func testEditsOnlyWhenTheMarkedTextChanges() {
    XCTAssertNil(InlineCompositionPolicy.edit(showing: "", next: ""))
    XCTAssertNil(InlineCompositionPolicy.edit(showing: "ni", next: "ni"))
    XCTAssertEqual(InlineCompositionPolicy.edit(showing: "", next: "n"), .mark("n"))
    XCTAssertEqual(InlineCompositionPolicy.edit(showing: "ni", next: "ni'h"), .mark("ni'h"))
    XCTAssertEqual(InlineCompositionPolicy.edit(showing: "ni", next: ""), .clear)
  }

  func testContextBeforeCompositionDropsOnlyTheMarkedSuffix() {
    XCTAssertEqual(InlineCompositionPolicy.contextBefore("你好，ni'hao", marked: "ni'hao"), "你好，")
    XCTAssertEqual(InlineCompositionPolicy.contextBefore("你好，", marked: ""), "你好，")
    // A host that does not report the marked text before the caret is left as it is.
    XCTAssertEqual(InlineCompositionPolicy.contextBefore("你好，", marked: "ni"), "你好，")
    XCTAssertNil(InlineCompositionPolicy.contextBefore(nil, marked: "ni"))
  }

  func testTheStyleIsOffUntilChosenAndTheOldSwitchReadsAsPinyin() {
    let defaults = InlinePreeditPreference.defaults
    let keys = [InlinePreeditPreference.key, InlinePreeditPreference.styleKey]
    let previous = keys.map { defaults.object(forKey: $0) }
    defer { for (key, value) in zip(keys, previous) { defaults.set(value, forKey: key) } }
    keys.forEach(defaults.removeObject(forKey:))
    XCTAssertEqual(InlinePreeditPreference.style, .off)
    XCTAssertFalse(InlinePreeditPreference.isEnabled)
    defaults.set(true, forKey: InlinePreeditPreference.key)
    XCTAssertEqual(InlinePreeditPreference.style, .pinyin)
    InlinePreeditPreference.style = .raw
    XCTAssertEqual(defaults.string(forKey: InlinePreeditPreference.styleKey), "raw")
    XCTAssertEqual(InlinePreeditPreference.style, .raw)
    InlinePreeditPreference.style = .off
    XCTAssertFalse(InlinePreeditPreference.isEnabled)
  }

  func testEachStyleWritesItsOwnFormOfTheComposition() {
    typealias Style = InlinePreeditPreference.Style
    XCTAssertEqual(Style.off.text(phrasePrefix: "你", preedit: "hao", editingText: "hao", japaneseReading: nil), "")
    XCTAssertEqual(Style.pinyin.text(phrasePrefix: "", preedit: "ni'hao", editingText: "nihao", japaneseReading: nil), "ni'hao")
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "ni'hao", editingText: "nihao", japaneseReading: nil), "nihao")
    // Shuangpin keys stay keys in raw and expand in pinyin, as Windows draws them.
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "shi", editingText: "ui", japaneseReading: nil), "ui")
    XCTAssertEqual(Style.raw.text(phrasePrefix: "你好", preedit: "shi'jie", editingText: "shijie", japaneseReading: nil), "你好shijie")
    // No ASCII keys (a local mode) falls back to what the Engine shows.
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "〔笔画〕", editingText: "", japaneseReading: nil), "〔笔画〕")
    XCTAssertEqual(Style.raw.text(phrasePrefix: "", preedit: "かな", editingText: "kana", japaneseReading: "かな"), "かな")
  }

  /// Korean marks the Hangul syllable whatever the style says, never its Dubeolsik key letters.
  func testKoreanAlwaysMarksTheSyllable() {
    for style in InlinePreeditPreference.Style.allCases {
      XCTAssertEqual(InlineCompositionPolicy.markedText(inPlace: true, style: style, phrasePrefix: "", preedit: "녕",
                                                        editingText: "sud", japaneseReading: nil), "녕", "\(style)")
    }
    XCTAssertEqual(InlineCompositionPolicy.markedText(inPlace: false, style: .off, phrasePrefix: "", preedit: "ni'hao",
                                                      editingText: "nihao", japaneseReading: nil), "")
    XCTAssertEqual(InlineCompositionPolicy.markedText(inPlace: false, style: .raw, phrasePrefix: "", preedit: "ni'hao",
                                                      editingText: "nihao", japaneseReading: nil), "nihao")
  }
}

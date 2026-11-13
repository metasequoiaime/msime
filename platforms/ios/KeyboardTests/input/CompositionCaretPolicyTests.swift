import XCTest

/// 放大拼写编辑条的光标换算，用例照 Android 的 `CompositionCaretPolicySmoke` 逐条对应（#10591），两端换算出的光标位置必须一样。
final class CompositionCaretPolicyTests: XCTestCase {
  func testLetterSpellingMapsOneToOne() {
    // 全拼：读音就是按键。#5613 的例子：jintianciwufanlemei 里把 ci 改成 chi，点在 c 和 i 之间。
    let typed = "jintianciwufanlemei"
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: typed, editing: typed, tapped: 8), 8)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: typed, editing: typed, tapped: 99), typed.count, "past the end")
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: typed, editing: typed, tapped: 0), 0)
    let mark = CompositionCaretPolicy.markIndex(display: typed, editing: typed, caret: 8, keyGrid: false)
    XCTAssertEqual(mark, 8)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: typed, editing: typed, caretMark: mark, tapped: 10), 9, "a tap after the mark skips it")
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: typed, editing: typed, caretMark: mark, tapped: 8), 8)
    XCTAssertNil(CompositionCaretPolicy.markIndex(display: typed, editing: typed, caret: typed.count, keyGrid: false), "a caret at the end has no mark")
  }

  func testLetterCaretSitsBeforeAnEngineSeparator() {
    // #10133：全拼读音带引擎加的分隔，分隔不在按键里，退格删的是分隔前的字母，光标画在分隔前面。
    let segmented = "zuo'tian'jin'tian"
    let letters = "zuotianjintian"
    let beforeSplit = CompositionCaretPolicy.markIndex(display: segmented, editing: letters, caret: 3, keyGrid: false)
    XCTAssertEqual(beforeSplit, 3)
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: segmented, editing: letters, caret: 3, keyGrid: false), 3)
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: segmented, editing: letters, caret: 4, keyGrid: false), 5, "zuo't|ian")
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: segmented, editing: letters, caret: 2, keyGrid: false), 2, "zu|o'tian")
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: segmented, editing: letters, caret: 7, keyGrid: false), 8, "zuo'tian|'jin'tian")
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: segmented, editing: letters, caretMark: beforeSplit, tapped: 3), 3)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: segmented, editing: letters, caretMark: beforeSplit, tapped: 4), 3)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: segmented, editing: letters, caretMark: beforeSplit, tapped: 5), 3)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: segmented, editing: letters, caretMark: beforeSplit, tapped: 6), 4)
    // 编辑条不画光标符：点分隔前后都换算到分隔前那个按键位置。
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: segmented, editing: letters, tapped: 3), 3)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: segmented, editing: letters, tapped: 4), 3)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: segmented, editing: letters, tapped: 5), 4)
    // 用户自己打的 ' 和粤拼的空格在按键里，一一对齐。
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "zuo'tian", editing: "zuo'tian", caret: 3, keyGrid: false), 3)
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "zuo'tian", editing: "zuo'tian", caret: 4, keyGrid: false), 4)
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "nei hou", editing: "nei hou", caret: 4, keyGrid: false), 4)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "nei hou", editing: "nei hou", tapped: 4), 4)
  }

  func testChosenPrefixMovesTheCaretHome() {
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(prefixLength: 2, display: "fanlemei", editing: "fanlemei", tapped: 1), 0)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(prefixLength: 2, display: "fanlemei", editing: "fanlemei", tapped: 5), 3)
  }

  func testNineKeyReadingAlignsLettersWithDigits() {
    // 九键：读音是拼音（9426 画成 xi'an），按键是数字；字母按键位对上数字，显示用的分隔跳过。
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "xi'an", editing: "9426", tapped: 3), 2, "tap after the separator")
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "xi'an", editing: "9426", tapped: 2), 2, "tap before the separator")
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "xi'an", editing: "9426", tapped: 4), 3, "tap inside the second syllable")
    // 组码网格里同一个光标位置隔着分隔对应两处时画在分隔后面。
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "94'26", editing: "9426", caret: 2, keyGrid: true), 3)
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "ba'ba", editing: "baba", caret: 2, keyGrid: true), 3, "fourteen-key caret after a split")
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "ba'ba", editing: "baba", caret: 2, keyGrid: true), 3)
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "ba'ba", editing: "baba", caret: 2, keyGrid: false), 2, "the same keys on 26 keys sit before the separator")
    // 选过拼音之后：ni'426 对 64426；首选是简拼行时读音是首字母（m't 对 68）。
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "ni'426", editing: "64426", tapped: 4), 3)
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "m't", editing: "68", tapped: 2), 1)
    // 多按或按错一键切不成音节时，切不开的那一键在读音里是键上的一个字母、单独成段（ni'hao's's 对 6442677），照样一位对一键。
    XCTAssertEqual(CompositionCaretPolicy.alignment("ni'hao's's", "6442677"), [0, 1, 2, 2, 3, 4, 5, 5, 6, 6, 7])
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "ni'hao's's", editing: "6442677", tapped: 7), 5, "tap on the first stray key")
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "ni'hao's's", editing: "6442677", tapped: 9), 6, "tap on the second stray key")
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "ni'hao's's", editing: "6442677", caret: 5, keyGrid: true), 7)
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "ni'hao's's", editing: "6442677", caret: 6, keyGrid: true), 9)
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "ni'hao's's", editing: "6442677", caret: 7, keyGrid: true), 10)
    // 九键首行是英文词时读音是这个词的字母（good 对 4663）。
    XCTAssertEqual(CompositionCaretPolicy.alignment("good", "4663"), [0, 1, 2, 3, 4])
    // 编辑条在末尾也画光标，分隔后的光标跟着分隔。
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "ni'hao", editing: "64426", caret: 5, keyGrid: true), 6)
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "ni'hao", editing: "64426", caret: 2, keyGrid: true), 3)
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "NI'HAO", editing: "64426", caret: 0, keyGrid: true), 0, "letters match their key in either case")
  }

  func testFullPinyinGridReadingAlignsLettersWithGroupCodes() {
    // 全拼 14 键：nihao 打的是 BN UI GH AS OP，按键是每组的首字母 bugao，读音是 ni'hao。
    XCTAssertNil(CompositionCaretPolicy.alignment("ni'hao", "bugao"), "without the grid the letters are not the keys")
    XCTAssertEqual(CompositionCaretPolicy.alignment("ni'hao", "bugao", letterGroups: .fourteenKey), [0, 1, 2, 2, 3, 4, 5])
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "ni'hao", editing: "bugao", tapped: 3, letterGroups: .fourteenKey), 2,
                   "tap before h lands before its key GH")
    XCTAssertEqual(CompositionCaretPolicy.tapTarget(display: "ni'hao", editing: "bugao", tapped: 5, letterGroups: .fourteenKey), 4)
    XCTAssertEqual(CompositionCaretPolicy.markIndex(display: "ni'hao", editing: "bugao", caret: 2, keyGrid: true,
                                                    letterGroups: .fourteenKey), 3, "a grid caret sits after the split")
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "ni'hao", editing: "bugao", caret: 5, keyGrid: true,
                                                        letterGroups: .fourteenKey), 6)
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "NI'HAO", editing: "bugao", caret: 1, keyGrid: true,
                                                        letterGroups: .fourteenKey), 1, "letters match their group in either case")
    // 选过拼音后按键里有原字母，同一个字母照样对上。
    XCTAssertEqual(CompositionCaretPolicy.alignment("ni'hao", "ni'gao", letterGroups: .fourteenKey), [0, 1, 2, 3, 4, 5, 6])
    // 18 键的分组不同：O 和 I 同键（组码 i），E 归 WE（组码 w）。
    XCTAssertEqual(CompositionCaretPolicy.alignment("ni'hao", "bihai", letterGroups: .eighteenKey), [0, 1, 2, 2, 3, 4, 5])
    XCTAssertNil(CompositionCaretPolicy.alignment("ni'hao", "bugao", letterGroups: .eighteenKey), "UI is two keys on 18 keys")
    XCTAssertTrue(CompositionCaretPolicy.sameKey("e", "w", letterGroups: .eighteenKey))
    XCTAssertFalse(CompositionCaretPolicy.sameKey("e", "w", letterGroups: .fourteenKey))
    // 组码之外的键不认：14 键上 n 只对 b。
    XCTAssertNil(CompositionCaretPolicy.alignment("ni", "ne", letterGroups: .fourteenKey))
    XCTAssertNil(CompositionCaretPolicy.alignment("ni", "ba", letterGroups: .fourteenKey))
  }

  func testTwentySixKeysAndShuangpinGridsKeepExactLetters() {
    // 26 键不传网格：n 就是 n，不会因为 14 键上同组而对上 b。
    XCTAssertFalse(CompositionCaretPolicy.sameKey("n", "b"))
    XCTAssertTrue(CompositionCaretPolicy.sameKey("n", "b", letterGroups: .fourteenKey))
    XCTAssertNil(CompositionCaretPolicy.alignment("nihao", "bugao"))
    XCTAssertEqual(CompositionCaretPolicy.alignment("nihao", "nihao"), [0, 1, 2, 3, 4, 5])
    // 14 键小鹤双拼 ni'hao 是 ni hc，按键 bugc；双拼网格不传网格，读音对不上，编辑条退回原始按键。
    XCTAssertNil(CompositionCaretPolicy.alignment("ni'hao", "bugc"))
    XCTAssertNil(CompositionCaretPolicy.displayOffset(display: "ni'hao", editing: "bugc", caret: 4, keyGrid: true))
  }

  func testAReadingOfOtherKeysIsNotGuessed() {
    XCTAssertNil(CompositionCaretPolicy.tapTarget(display: "xi'an", editing: "9427", tapped: 3))
    XCTAssertNil(CompositionCaretPolicy.tapTarget(display: "xi", editing: "9426", tapped: 1), "a reading shorter than the keys")
    XCTAssertNil(CompositionCaretPolicy.markIndex(display: "xi'an", editing: "9427", caret: 1, keyGrid: true))
    XCTAssertNil(CompositionCaretPolicy.displayOffset(display: "xi'an", editing: "9427", caret: 4, keyGrid: true))
    XCTAssertNil(CompositionCaretPolicy.tapTarget(display: "ni", editing: "64", tapped: -1), "no layout offset")
    XCTAssertNil(CompositionCaretPolicy.alignment("你好", "nihao"), "a non-ASCII reading")
    XCTAssertEqual(CompositionCaretPolicy.alignment("ni'hao", "64426"), [0, 1, 2, 2, 3, 4, 5])
  }

  func testMovesUseHomeAndEndAtTheEdgesAndStepsInBetween() {
    XCTAssertEqual(CompositionCaretPolicy.moves(current: 19, target: 0, length: 19), .home)
    XCTAssertEqual(CompositionCaretPolicy.moves(current: 3, target: 19, length: 19), .end)
    XCTAssertEqual(CompositionCaretPolicy.moves(current: 19, target: 9, length: 19), .left(10))
    XCTAssertEqual(CompositionCaretPolicy.moves(current: 2, target: 5, length: 19), .right(3))
    XCTAssertNil(CompositionCaretPolicy.moves(current: 5, target: 5, length: 19))
    XCTAssertNil(CompositionCaretPolicy.moves(current: 5, target: -1, length: 19))
    XCTAssertNil(CompositionCaretPolicy.moves(current: 5, target: 20, length: 19))
  }
}

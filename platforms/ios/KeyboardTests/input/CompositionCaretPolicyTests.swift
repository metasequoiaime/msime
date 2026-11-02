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
    // 编辑条在末尾也画光标，分隔后的光标跟着分隔。
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "ni'hao", editing: "64426", caret: 5, keyGrid: true), 6)
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "ni'hao", editing: "64426", caret: 2, keyGrid: true), 3)
    XCTAssertEqual(CompositionCaretPolicy.displayOffset(display: "NI'HAO", editing: "64426", caret: 0, keyGrid: true), 0, "letters match their key in either case")
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

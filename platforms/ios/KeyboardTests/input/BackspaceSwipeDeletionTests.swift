import XCTest

/// 按住 ⌫ 横滑删除：往左滑多远删多少，不松手往右滑按原样恢复。
final class BackspaceSwipeDeletionTests: XCTestCase {
  private let step = KeyboardLayoutPreference.backspaceSwipeStep(speed: KeyboardLayoutPreference.backspaceSwipeSpeedDefault)

  func testSpeedLevelsMapToStepDistances() {
    XCTAssertEqual(KeyboardLayoutPreference.backspaceSwipeStep(speed: 1), 22)
    XCTAssertEqual(KeyboardLayoutPreference.backspaceSwipeStep(speed: 7), 10, "默认档")
    XCTAssertEqual(KeyboardLayoutPreference.backspaceSwipeStep(speed: 10), 4)
    XCTAssertEqual(KeyboardLayoutPreference.backspaceSwipeStep(speed: 0), 22, "低于范围按第 1 档")
    XCTAssertEqual(KeyboardLayoutPreference.backspaceSwipeStep(speed: 99), 4, "高于范围按第 10 档")
    XCTAssertEqual(BackspaceSwipeDeletion(unit: .text(context: "a")).step, 10, "不传时用默认档")
    // 档位越高，滑同样的距离删得越多。
    let slow = BackspaceSwipeDeletion(unit: .text(context: String(repeating: "字", count: 50)), step: 22)
    let fast = BackspaceSwipeDeletion(unit: .text(context: String(repeating: "字", count: 50)), step: 4)
    XCTAssertEqual(slow.target(translation: -88), 4)
    XCTAssertEqual(fast.target(translation: -88), 22)
  }

  func testLeftwardDistanceDecidesHowManyToDelete() {
    let swipe = BackspaceSwipeDeletion(unit: .text(context: "你好世界"))
    XCTAssertEqual(swipe.available, 4)
    XCTAssertEqual(swipe.target(translation: 0), 0)
    XCTAssertEqual(swipe.target(translation: 30), 0, "往右滑不删")
    XCTAssertEqual(swipe.target(translation: -(step - 0.1)), 0, "不满一格不删")
    XCTAssertEqual(swipe.target(translation: -step), 1)
    XCTAssertEqual(swipe.target(translation: -2.5 * step), 2)
    XCTAssertEqual(swipe.target(translation: -100 * step), 4, "最多删到读得到的字数")
    XCTAssertEqual(swipe.target(translation: -.infinity), 0)
  }

  func testMovingBackRestoresWhatWasDeleted() {
    var swipe = BackspaceSwipeDeletion(unit: .text(context: "ab😀c"))
    XCTAssertEqual(swipe.move(to: -3 * step), 3, "删 c、😀、b")
    XCTAssertEqual(swipe.applied, 3)
    XCTAssertEqual((0..<3).compactMap { swipe.deletedText(at: $0) }, ["c", "😀", "b"], "离光标近的先删")
    XCTAssertEqual(swipe.move(to: -1 * step), -2, "往右滑回两格，恢复两个")
    XCTAssertEqual(swipe.applied, 1)
    XCTAssertEqual(swipe.move(to: 20), -1, "滑过起点全部恢复")
    XCTAssertEqual(swipe.applied, 0)
    XCTAssertEqual(swipe.move(to: 25), 0)
  }

  func testCompositionDeletesBySyllableAndRestoresInOrder() {
    var swipe = BackspaceSwipeDeletion(unit: .composition(original: "nihaoshijie"))
    XCTAssertEqual(swipe.available, 11, "事先按组码字数估上限")
    XCTAssertEqual(swipe.move(to: -2 * step), 2)
    swipe.recordRemoved(before: "nihaoshijie", after: "nihaoshi")
    swipe.recordRemoved(before: "nihaoshi", after: "nihao")
    XCTAssertEqual(swipe.removedCodes, ["jie", "shi"], "一格一个音节")
    XCTAssertTrue(swipe.restorable)
    XCTAssertEqual(swipe.popRemovedCode(), "shi", "先恢复最后删的")
    XCTAssertEqual(swipe.popRemovedCode(), "jie")
    XCTAssertNil(swipe.popRemovedCode())
    XCTAssertNil(swipe.deletedText(at: 0), "组码不按文字插回")
  }

  func testCompositionStopsWhenEmptyAndRefusesMismatchedCodes() {
    var swipe = BackspaceSwipeDeletion(unit: .composition(original: "nihao"))
    XCTAssertEqual(swipe.move(to: -5 * step), 5)
    swipe.stop(at: 2)
    XCTAssertEqual(swipe.applied, 2)
    XCTAssertEqual(swipe.move(to: -9 * step), 0, "删空之后再往左滑不再删")
    XCTAssertEqual(swipe.move(to: -1 * step), -1, "往右滑从删空处恢复")
    swipe.recordRemoved(before: "nihao", after: "ni'hao")
    XCTAssertFalse(swipe.restorable, "删完的码不是原码的前缀时不再恢复")
  }

  func testOnlyRetypableCompositionsTakeTheSwipe() {
    XCTAssertTrue(BackspaceSwipeDeletion.canRetype(composition: "ni'hao", caret: 6))
    XCTAssertTrue(BackspaceSwipeDeletion.canRetype(composition: "64426", caret: 5), "九键数字")
    XCTAssertFalse(BackspaceSwipeDeletion.canRetype(composition: "nihao", caret: 2), "光标不在末尾")
    XCTAssertFalse(BackspaceSwipeDeletion.canRetype(composition: "", caret: 0))
    XCTAssertFalse(BackspaceSwipeDeletion.canRetype(composition: "u;", caret: 2), "微软双拼的 ;")
    XCTAssertFalse(BackspaceSwipeDeletion.canRetype(composition: "かな", caret: 2))
  }
}

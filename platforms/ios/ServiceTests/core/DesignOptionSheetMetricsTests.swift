import XCTest

final class DesignOptionSheetMetricsTests: XCTestCase {
  /// 四个选项（中文键盘）的列表只有四行加三条分隔线高，不随屏幕高度撑大。
  func testShortListHugsItsRows() {
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 4, limit: 537.6), 226)
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 1, limit: 537.6), 56)
  }

  /// 3x 屏上 0.5pt 的分隔线对齐像素后会多出零点几，高度向上取整，内容不会比框高。
  func testHeightRoundsUpToWholePoints() {
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 2, limit: 1000), 113)
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 3, limit: 1000), 169)
  }

  /// 选项多到超过上限时停在上限，由列表滚动。
  func testLongListStopsAtLimit() {
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 30, limit: 400), 400)
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 8, limit: 451), 451)
  }

  /// 带副标题的选项（粤语双拼的「方案设计：陳澍鋒」）读屏时接在选项名后面，没有副标题的只读选项名。
  func testSubtitleFollowsTheTitleForVoiceOver() {
    XCTAssertEqual(DesignOption(title: "粤语双拼", value: 1, subtitle: "方案设计：陳澍鋒").accessibilityTitle, "粤语双拼，方案设计：陳澍鋒")
    XCTAssertEqual(DesignOption(title: "粤拼", value: 0).accessibilityTitle, "粤拼")
  }

  func testEmptyListAndNegativeLimitTakeNoSpace() {
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 0, limit: 400), 0)
    XCTAssertEqual(DesignOptionSheetMetrics.listHeight(rows: 3, limit: -1), 0)
  }
}

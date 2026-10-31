import XCTest

/// 设置「输入」页的语言行和它的选项面板代表同一个方案：行上写着「双拼 · 小鹤」，打开面板就要在小鹤上打勾，键盘此刻用着全拼也一样。
final class LanguageRowSchemeTests: XCTestCase {
  private let shuangpin: [ChineseInputScheme] = [.shuangpin, .ziranma, .microsoft, .shoudao, .nineKeyShuangpin]

  func testOtherLanguageInUseStillMarksTheEnabledScheme() {
    XCTAssertEqual(ChineseInputScheme.represented(among: shuangpin, enabled: [.quanpin, .shuangpin], current: .quanpin), .shuangpin)
    XCTAssertEqual(ChineseInputScheme.represented(among: [.wubi], enabled: [.quanpin, .wubi], current: .quanpin), .wubi)
  }

  func testSchemeInUseWinsOverEarlierEnabledOnes() {
    XCTAssertEqual(ChineseInputScheme.represented(among: shuangpin, enabled: [.shuangpin, .ziranma], current: .ziranma), .ziranma)
  }

  /// 当前方案属于这个语言却没启用（偏好正在改写的一瞬间），退回该语言第一个已启用的方案。
  func testCurrentSchemeMustBeEnabled() {
    XCTAssertEqual(ChineseInputScheme.represented(among: shuangpin, enabled: [.quanpin, .microsoft], current: .ziranma), .microsoft)
  }

  func testNothingEnabledMarksNothing() {
    XCTAssertNil(ChineseInputScheme.represented(among: shuangpin, enabled: [.quanpin], current: .quanpin))
  }
}

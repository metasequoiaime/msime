import XCTest

/// 「输入」页的语言行（`InputLanguage`）：每门语言列出的触屏方案与 Android `TypingPage.Language` 相同。
final class InputLanguageTests: XCTestCase {
  /// 全拼一行列 26、14、18、9 键，与 Android 的 `Language.QUANPIN` 同序。#9653 按方案拆分输入设置时漏了 #9643 加的 18 键，键盘在 18 键时全拼一行既不标默认、面板里也没有它。
  func testQuanpinOffersEveryPinyinLayoutInAndroidOrder() {
    XCTAssertEqual(InputLanguage.quanpin.schemes, [.quanpin, .fourteenKey, .eighteenKey, .nineKey])
    XCTAssertEqual(InputLanguage.of(.eighteenKey), .quanpin)
    XCTAssertEqual(InputLanguage.of(.fourteenKey), .quanpin)
  }

  /// 每个触屏方案恰好属于一门语言：`InputLanguage.of` 的兜底不能替一个漏掉的方案找到行。
  func testEverySchemeBelongsToExactlyOneLanguage() {
    for scheme in ChineseInputScheme.allCases {
      let owners = InputLanguage.allCases.filter { $0.schemes.contains(scheme) }
      XCTAssertEqual(owners.count, 1, "\(scheme) belongs to \(owners)")
    }
  }

  /// 键盘在全拼 18 键时，全拼一行代表的就是 18 键，面板里打勾的是它，而不是第一个启用的 26 键。行上的「默认」和移除全拼由 `OnboardingUITests.testQuanpinRowOffersEighteenKeyAndRemovesIt` 在设置界面上核对。
  func testTheQuanpinRowRepresentsTheEighteenKeys() {
    XCTAssertEqual(ChineseInputScheme.represented(among: InputLanguage.quanpin.schemes,
                                                  enabled: [.quanpin, .eighteenKey, .shuangpin], current: .eighteenKey),
                   .eighteenKey)
  }
}

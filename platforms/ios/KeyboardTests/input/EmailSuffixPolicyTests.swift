import XCTest

/// 光标前打到 `xxx@` 时候选栏给出的邮箱后缀，与 Android 的 `EmailSuffixPolicySmoke` 逐条对应。
final class EmailSuffixPolicyTests: XCTestCase {
  func testTwentyLowercaseSuffixesWithDomesticOnesFirst() {
    let all = EmailSuffixPolicy.suffixes
    XCTAssertEqual(all.count, 20)
    XCTAssertEqual(Set(all).count, all.count)
    XCTAssertEqual(Array(all.prefix(7)), ["@qq.com", "@163.com", "@126.com", "@foxmail.com", "@139.com", "@189.cn", "@aliyun.com"])
    for suffix in all {
      XCTAssertTrue(suffix.hasPrefix("@") && !suffix.dropFirst().contains("@") && suffix == suffix.lowercased(), suffix)
    }
  }

  func testTypedDomainNarrowsTheSuffixes() throws {
    let bare = try XCTUnwrap(EmailSuffixPolicy.match(textBeforeCursor: "abc@"))
    XCTAssertEqual(bare.typedDomain, "")
    XCTAssertEqual(bare.suffixes, EmailSuffixPolicy.suffixes)
    XCTAssertEqual(EmailSuffixPolicy.match(textBeforeCursor: "abc@1")?.suffixes, ["@163.com", "@126.com", "@139.com", "@189.cn"])
    XCTAssertEqual(EmailSuffixPolicy.match(textBeforeCursor: "abc@yahoo.c")?.suffixes, ["@yahoo.com", "@yahoo.co.jp"])
    XCTAssertEqual(EmailSuffixPolicy.match(textBeforeCursor: "abc@gm")?.suffixes, ["@gmail.com", "@gmx.com"])
    let upper = try XCTUnwrap(EmailSuffixPolicy.match(textBeforeCursor: "Abc@GMa"), "已打的域名不分大小写")
    XCTAssertEqual(upper.typedDomain, "GMa")
    XCTAssertEqual(upper.suffixes, ["@gmail.com"])
  }

  func testFinishedOrUnknownDomainOffersNothing() {
    XCTAssertNil(EmailSuffixPolicy.match(textBeforeCursor: "abc@163.com"))
    XCTAssertNil(EmailSuffixPolicy.match(textBeforeCursor: "abc@example.org"))
  }

  func testOnlyTheCharacterBeforeTheLastAtIsChecked() {
    XCTAssertNotNil(EmailSuffixPolicy.match(textBeforeCursor: "a.b_c+d-e%f@"), "本地部分可以带 ._%+-")
    XCTAssertEqual(EmailSuffixPolicy.match(textBeforeCursor: "x@y@")?.typedDomain, "", "以最后一个 @ 为准")
    XCTAssertNotNil(EmailSuffixPolicy.match(textBeforeCursor: "我的邮箱 hi@"), "只有紧挨 @ 的字符要是 ASCII")
  }

  func testTextThatIsNotAnAddressOffersNothing() {
    for text in ["@", "微信@", "a b @", "a@@", "abc＠", "abc@ ", ""] {
      XCTAssertNil(EmailSuffixPolicy.match(textBeforeCursor: text), text)
    }
    XCTAssertNil(EmailSuffixPolicy.match(textBeforeCursor: nil))
  }

  /// 宿主给的上文可能是一整段，只看最后 `maxContext` 个字符，本地部分比这更长也照样匹配。
  func testLongContextBeforeTheAddressStillMatches() {
    let paragraph = String(repeating: "这是一段很长的上文，", count: 20) + String(repeating: "a", count: 200) + "@g"
    XCTAssertEqual(EmailSuffixPolicy.match(textBeforeCursor: paragraph)?.suffixes, ["@gmail.com", "@gmx.com"])
  }

  func testCompletionDeletesTheTypedDomainAndInsertsTheSuffixWithoutAt() {
    let bare = EmailSuffixPolicy.match(textBeforeCursor: "abc@")
    XCTAssertEqual(EmailSuffixPolicy.replacement(bare, suffix: "@163.com"), .init(deleteCount: 0, insert: "163.com"))
    let upper = EmailSuffixPolicy.match(textBeforeCursor: "Abc@GMa")
    XCTAssertEqual(EmailSuffixPolicy.replacement(upper, suffix: "@gmail.com"), .init(deleteCount: 3, insert: "gmail.com"))
    XCTAssertNil(EmailSuffixPolicy.replacement(EmailSuffixPolicy.match(textBeforeCursor: "abc@1"), suffix: "@gmail.com"))
    XCTAssertNil(EmailSuffixPolicy.replacement(nil, suffix: "@163.com"))
  }
}

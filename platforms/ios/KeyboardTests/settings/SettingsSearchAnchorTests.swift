import XCTest

/// 设置搜索结果带着命中的关键词打开页面：以前结果只给页面，搜「社区插件」打开「插件」停在「我的插件」。规则与 Android `SettingsSearchAnchorPolicySmoke` 钉住的相同。
final class SettingsSearchAnchorTests: XCTestCase {
  private let pluginKeywords = ["短语表", "指令表", "符号集", "单词本", "音效包", "辅助码表", "社区插件"]

  func testTheFirstKeywordContainingTheQueryIsTheAnchor() {
    XCTAssertEqual(SettingsSearchAnchor.anchor(keywords: pluginKeywords, query: "社区插件"), "社区插件")
    XCTAssertEqual(SettingsSearchAnchor.anchor(keywords: pluginKeywords, query: "社区"), "社区插件")
    XCTAssertEqual(SettingsSearchAnchor.anchor(keywords: pluginKeywords, query: " 社区 "), "社区插件")
    XCTAssertEqual(SettingsSearchAnchor.anchor(keywords: ["组字时保留 Shift", "Shift"], query: "SHIFT"), "组字时保留 Shift")
  }

  func testATitleOnlyOrEmptyQueryHasNoAnchor() {
    XCTAssertNil(SettingsSearchAnchor.anchor(keywords: pluginKeywords, query: "插件包"))
    XCTAssertNil(SettingsSearchAnchor.anchor(keywords: pluginKeywords, query: ""))
    XCTAssertNil(SettingsSearchAnchor.anchor(keywords: pluginKeywords, query: "   "))
  }

  func testAQueryNamingThePageHasNoTarget() {
    XCTAssertNil(SettingsSearchAnchor.target(title: "插件", keywords: pluginKeywords, query: "插件"))
    XCTAssertNil(SettingsSearchAnchor.target(title: "插件", keywords: pluginKeywords, query: " 插件 "))
    XCTAssertNil(SettingsSearchAnchor.target(title: "插件", keywords: pluginKeywords, query: ""))
    XCTAssertEqual(SettingsSearchAnchor.target(title: "插件", keywords: pluginKeywords, query: "社区插件"), "社区插件")
    XCTAssertEqual(SettingsSearchAnchor.target(title: "插件", keywords: pluginKeywords, query: "社区"), "社区插件")
  }

  func testAnAnchorNamingASegmentOpensThatSegment() {
    let segments = ["我的插件", "社区插件"]
    XCTAssertEqual(SettingsSearchAnchor.section(segments, anchor: "社区插件"), "社区插件")
    XCTAssertNil(SettingsSearchAnchor.section(segments, anchor: "短语表"))
    XCTAssertNil(SettingsSearchAnchor.section(segments, anchor: nil))
    XCTAssertNil(SettingsSearchAnchor.section([], anchor: "社区插件"))
  }
}

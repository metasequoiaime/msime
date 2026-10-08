import XCTest

/// 社区列表的「已添加」看的是本机排队导入过的词库，不是服务器上的收藏。
final class CommunityDictionaryImportsTests: XCTestCase {
  private var suite = ""
  private var defaults: UserDefaults!

  override func setUp() {
    super.setUp()
    suite = "CommunityDictionaryImportsTests-\(UUID().uuidString)"
    defaults = UserDefaults(suiteName: suite)
  }

  override func tearDown() {
    defaults.removePersistentDomain(forName: suite)
    super.tearDown()
  }

  func testRecordedImportsAreReported() {
    XCTAssertEqual(CommunityDictionaryImports.ids(in: defaults), [])
    CommunityDictionaryImports.record("a", in: defaults)
    CommunityDictionaryImports.record("b", in: defaults)
    CommunityDictionaryImports.record("a", in: defaults)
    XCTAssertEqual(CommunityDictionaryImports.ids(in: defaults), ["a", "b"])
    XCTAssertEqual(defaults.stringArray(forKey: CommunityDictionaryImports.key), ["b", "a"])
  }

  func testRecordKeepsOnlyMostRecentImports() {
    for index in 0...CommunityDictionaryImports.limit {
      CommunityDictionaryImports.record("id-\(index)", in: defaults)
    }
    let ids = CommunityDictionaryImports.ids(in: defaults)
    XCTAssertEqual(ids.count, CommunityDictionaryImports.limit)
    XCTAssertFalse(ids.contains("id-0"))
    XCTAssertTrue(ids.contains("id-\(CommunityDictionaryImports.limit)"))
  }
}

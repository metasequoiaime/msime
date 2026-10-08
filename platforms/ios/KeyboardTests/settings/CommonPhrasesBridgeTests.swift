import XCTest

/// 应用的「常用语」页经 `CommonPhrasesBridge` 写入，键盘面板读取同一个存储，所以这些测试在临时目录里运行真实的存储：页面添加了什么，面板随后就列出什么；存储的拒绝以其稳定错误码传到页面。
final class CommonPhrasesBridgeTests: XCTestCase {
  private var directory: URL!

  override func setUpWithError() throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent("common-phrases-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: directory)
  }

  func testAddedPhrasesAreWhatTheKeyboardLoads() throws {
    XCTAssertEqual(try CommonPhrasesBridge.load(directory: directory.path), [])
    _ = try CommonPhrasesBridge.add("我在开会，稍后回复", directory: directory.path)
    let stored = try CommonPhrasesBridge.add("第一行\n第二行", directory: directory.path)
    XCTAssertEqual(stored.map(\.text), ["我在开会，稍后回复", "第一行\n第二行"])
    XCTAssertTrue(stored.allSatisfy { $0.pack == nil })
    XCTAssertEqual(try CommonPhrasesBridge.load(directory: directory.path), stored)
  }

  func testRemoveDropsOnlyThatPhrase() throws {
    _ = try CommonPhrasesBridge.add("好的", directory: directory.path)
    let both = try CommonPhrasesBridge.add("收到", directory: directory.path)
    let remaining = try CommonPhrasesBridge.remove(id: both[0].id, directory: directory.path)
    XCTAssertEqual(remaining.map(\.text), ["收到"])
    XCTAssertEqual(try CommonPhrasesBridge.load(directory: directory.path), remaining)
  }

  func testRefusalsCarryTheStoreCodeAndAMessage() throws {
    _ = try CommonPhrasesBridge.add("好的", directory: directory.path)
    XCTAssertThrowsError(try CommonPhrasesBridge.add("好的", directory: directory.path)) { error in
      XCTAssertEqual(error as? CommonPhrasesBridge.Failure, .rejected("common_phrases_duplicate"))
      XCTAssertEqual(CommonPhrasesBridge.message(for: error), "已经有这条常用语了。")
    }
    XCTAssertThrowsError(try CommonPhrasesBridge.add("   ", directory: directory.path)) { error in
      XCTAssertEqual(error as? CommonPhrasesBridge.Failure, .rejected("common_phrases_invalid"))
    }
    XCTAssertThrowsError(try CommonPhrasesBridge.remove(id: UUID().uuidString.lowercased(), directory: directory.path)) { error in
      XCTAssertEqual(error as? CommonPhrasesBridge.Failure, .rejected("common_phrases_not_found"))
    }
    XCTAssertThrowsError(try CommonPhrasesBridge.load(directory: "relative")) { error in
      XCTAssertEqual(error as? CommonPhrasesBridge.Failure, .unavailable)
      XCTAssertEqual(CommonPhrasesBridge.message(for: error), "词库还没准备好，请先完成首次设置。")
    }
  }
}

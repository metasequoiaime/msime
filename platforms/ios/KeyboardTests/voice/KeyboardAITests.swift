import XCTest
import Security

final class KeyboardAITests: XCTestCase {
  @MainActor
  func testReplyClearDropsLateResultsAndNeverInsertsAutomatically() async throws {
    let model = ReplyKeyboardModel()
    var inserted = 0
    model.setText("你睡了吗")
    model.generate(style: "高情商", request: { _, _ in
      try? await Task.sleep(nanoseconds: 30_000_000)
      return "还没有"
    }, insert: { _ in inserted += 1; return true })
    model.setText("")
    try await Task.sleep(nanoseconds: 80_000_000)
    XCTAssertTrue(model.replies.isEmpty)
    XCTAssertFalse(model.busy)
    XCTAssertEqual(inserted, 0)
    model.setText("你好")
    model.generate(style: "高情商", request: { _, _ in "你好呀" }, insert: { _ in inserted += 1; return true })
    while model.busy { await Task.yield() }
    XCTAssertEqual(model.replies, ["你好呀"])
    XCTAssertEqual(inserted, 0)
    model.invalidateContext()
    model.use("你好呀")
    XCTAssertEqual(inserted, 0)
  }

  @MainActor
  func testDownloadedReplyTemplateReachesPromptAndRemovalPreventsReuse() async throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-community-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let id = UUID().uuidString
    let item = CommunityResource(id: id, kind: .reply, name: "测试风格", description: "测试", author: "测试作者",
      content: .init(prompt: "使用三句简短的话"), revision: 1, saves: 0, saved: true, owned: false,
      rating_count: 0, rating_average: 0, my_rating: 0)
    try CommunityLibrary.save(item, in: directory)
    let model = ReplyKeyboardModel(communityReplies: { CommunityLibrary.replies(in: directory) })
    model.setText("你好")
    var captured = ""
    var requests = 0
    model.generate(style: "community:\(id)", request: { source, prompt in
      XCTAssertEqual(source, "你好"); captured = prompt; requests += 1; return "你好呀"
    }, insert: { _ in XCTFail("must not insert automatically"); return false })
    while model.busy { await Task.yield() }
    XCTAssertTrue(captured.contains("使用三句简短的话"))
    XCTAssertEqual(model.replies, ["你好呀"])
    try CommunityLibrary.remove(id, in: directory)
    model.resetResults()
    model.generate(style: "community:\(id)", request: { _, _ in requests += 1; return "unexpected" }, insert: { _ in false })
    XCTAssertEqual(requests, 1)
    XCTAssertFalse(model.busy)
    XCTAssertTrue(model.status.contains("模板已移除"))
  }

  func testCommunityLibraryRejectsMalformedReplyTemplate() throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-community-invalid-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let item = CommunityResource(id: UUID().uuidString, kind: .reply, name: "测试风格",
      description: "测试", author: "测试作者",
      content: .init(prompt: String(repeating: "字", count: 2_001)), revision: 1, saves: 0,
      saved: true, owned: false, rating_count: 0, rating_average: 0, my_rating: 0)
    XCTAssertThrowsError(try CommunityLibrary.save(item, in: directory))
  }

  func testAISelectionRejectsDocumentCaretAndTextChanges() {
    let id = UUID()
    let selection = KeyboardDocumentContext(document: id, before: "before", selected: "fixture", after: "after")
    XCTAssertTrue(selection.matches(document: id, before: "before", selected: "fixture", after: "after"))
    XCTAssertFalse(selection.matches(document: UUID(), before: "before", selected: "fixture", after: "after"))
    XCTAssertFalse(selection.matches(document: nil, before: "before", selected: "fixture", after: "after"))
    XCTAssertFalse(selection.matches(document: id, before: "changed", selected: "fixture", after: "after"))
    XCTAssertFalse(selection.matches(document: id, before: "before", selected: nil, after: "after"))
    XCTAssertFalse(selection.matches(document: id, before: "before", selected: "fixture", after: "changed"))
  }

  func testSharedAIKeysStayOriginScopedAndConfigurationRoundTrips() throws {
    let first = try XCTUnwrap(URL(string: "https://fixture.invalid/v1/chat/completions"))
    let otherPath = try XCTUnwrap(URL(string: "https://fixture.invalid/v2/chat/completions"))
    let otherHost = try XCTUnwrap(URL(string: "https://other.invalid/v1/chat/completions"))
    let otherPort = try XCTUnwrap(URL(string: "https://fixture.invalid:444/v1/chat/completions"))
    let query = KeyboardAIService.query(url: first)
    let account = kSecAttrAccount as String
    XCTAssertEqual(query[kSecAttrAccessGroup as String] as? String, "group.app.msime.ios")
    XCTAssertEqual(query[account] as? String, KeyboardAIService.query(url: otherPath)[account] as? String)
    XCTAssertNotEqual(query[account] as? String, KeyboardAIService.query(url: otherHost)[account] as? String)
    XCTAssertNotEqual(query[account] as? String, KeyboardAIService.query(url: otherPort)[account] as? String)
    var config = CustomServiceConfiguration()
    config.endpoint = first.absoluteString
    config.model = "fixture"
    config.prompt = "保留换行\n保持原意"
    let encoded = try JSONEncoder().encode(config)
    XCTAssertEqual(try JSONDecoder().decode(CustomServiceConfiguration.self, from: encoded), config)
    let fields = try XCTUnwrap(JSONSerialization.jsonObject(with: encoded) as? [String: Any])
    XCTAssertEqual(Set(fields.keys), ["provider", "endpoint", "model", "prompt"])
  }
}

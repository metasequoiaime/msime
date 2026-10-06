import XCTest

final class CloudDictionaryTransferTests: XCTestCase {
  private func entry(_ kind: String, _ code: String, _ word: String) throws -> BackendAccountClient.DictionaryEntry {
    let data = try JSONSerialization.data(withJSONObject: ["id": String(repeating: "a", count: 64), "kind": kind,
      "code": code, "word": word, "weight": 100_000, "revision": 1])
    return try JSONDecoder().decode(BackendAccountClient.DictionaryEntry.self, from: data)
  }
  func testCloudKindsUseEngineValidationBeforeLocalQueue() throws {
    XCTAssertEqual(try entry("pinyin", "ni'hao", "你好").localWord().kind, .pinyin)
    XCTAssertEqual(try entry("wubi", "wq", "你").localWord().kind, .wubi)
    XCTAssertEqual(try entry("wubi98", "wq", "你").localWord().kind, .wubi98)
    XCTAssertEqual(try entry("english", "hello", "Hello").localWord().kind, .english)
    XCTAssertEqual(try entry("quick", "test", "合成短语").localWord().kind, .quickPhrase)
    XCTAssertThrowsError(try entry("pinyin", "nihao", "你好").localWord())
    XCTAssertEqual(try entry("english", "dont", "don't").localWord().kind, .english)
    XCTAssertThrowsError(try entry("english", "hello world", "Different").localWord())
  }
  @MainActor func testDownloadedCloudWordReachesActualKeyboardCandidate() throws {
    let word = try entry("quick", "cloudfixture", "合成云词库验收").localWord()
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = PersonalDictionaryStore(directory: root)
    let resources = try XCTUnwrap(
      Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true)
    )
    let session = MetasequoiaInputSessionBridge(
      resources: resources,
      stateRoot: root.appendingPathComponent("EngineState", isDirectory: true)
    )
    defer {
      _ = session.cancel()
      try? session.applyPersonalPrevious(word.bridgeValue, replacement: nil, requestID: UUID().uuidString)
    }
    try store.enqueue(previous: nil, replacement: word)
    try store.synchronize(apply: {
      try session.applyPersonalPrevious($0.previous?.bridgeValue, replacement: $0.replacement?.bridgeValue, requestID: $0.id)
    }, page: { _ in .init(entries: [], hasMore: false) })
    XCTAssertEqual(try store.read().requests.first?.status, .applied)
    _ = session.openLocalMode("K")
    var snapshot = session.handleCharacter("c")
    for letter in word.key.dropFirst() { snapshot = session.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains(word.value))
  }
}

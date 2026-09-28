import XCTest

final class VoiceTextHandoffTests: XCTestCase {
  func testOneTimeClaimReplacementAndExpiry() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let host = VoiceTextHandoffStore(directory: root), keyboard = VoiceTextHandoffStore(directory: root)
    let now = Date(timeIntervalSince1970: 1_000_000)
    XCTAssertNil(try keyboard.read(now: now))
    let first = try host.save("语音测试\n保留换行", now: now)
    XCTAssertEqual(try keyboard.read(now: now)?.text, first.text)
    let second = try host.save("新结果", now: now)
    XCTAssertThrowsError(try keyboard.consume(first.id, now: now))
    XCTAssertEqual(try keyboard.read(now: now)?.id, second.id)
    XCTAssertEqual(try keyboard.consume(second.id, now: now), "新结果")
    XCTAssertThrowsError(try keyboard.consume(second.id, now: now))
    XCTAssertNil(try host.read(now: now))
    let expiring = try host.save("expired fixture", now: now)
    XCTAssertThrowsError(try keyboard.consume(expiring.id, now: now.addingTimeInterval(600)))
    XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("VoiceHandoff/result.json").path))
    let removable = try host.save("discard fixture", now: now)
    try host.discard(removable.id, now: now)
    XCTAssertNil(try keyboard.read(now: now))
  }

  func testInvalidSavePreservesPendingTextAndMalformedStateIsNotConsumed() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = VoiceTextHandoffStore(directory: root)
    let entry = try store.save("fixture")
    XCTAssertThrowsError(try store.save(" \n"))
    XCTAssertThrowsError(try store.save(String(repeating: "字", count: 10_001)))
    XCTAssertEqual(try store.read(), entry)
    let file = root.appendingPathComponent("VoiceHandoff/result.json")
    let fields = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any])
    XCTAssertEqual(Set(fields.keys), ["version", "id", "text", "createdAt", "expiresAt"])
    let malformed = Data("invalid fixture".utf8)
    try malformed.write(to: file)
    XCTAssertThrowsError(try store.consume(entry.id))
    XCTAssertEqual(try Data(contentsOf: file), malformed)
    try Data(repeating: 32, count: 256 * 1024 + 1).write(to: file)
    XCTAssertThrowsError(try store.read())
    XCTAssertThrowsError(try VoiceTextHandoffStore.readBounded(file))
    // A deliberate new transfer recovers the transport without retaining corrupt content.
    let replacement = try store.save("recovery fixture")
    XCTAssertEqual(try store.read(), replacement)
    XCTAssertThrowsError(try VoiceTextHandoffStore(directory: nil).save("fixture"))
  }

  func testConcurrentKeyboardsCannotClaimTheSameResultTwice() throws {
    final class Claims: @unchecked Sendable {
      let lock = NSLock()
      var texts: [String] = []
      func append(_ text: String) { lock.lock(); defer { lock.unlock() }; texts.append(text) }
    }
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let entry = try VoiceTextHandoffStore(directory: root).save("one fixture")
    let claims = Claims()
    DispatchQueue.concurrentPerform(iterations: 2) { _ in
      if let text = try? VoiceTextHandoffStore(directory: root).consume(entry.id) { claims.append(text) }
    }
    XCTAssertEqual(claims.texts, ["one fixture"])
  }

  func testVoiceInsertionContextAcceptsEmptySelectionButRejectsCaretChanges() {
    let id = UUID()
    let context = KeyboardDocumentContext(document: id, before: "fixture", selected: nil, after: nil)
    XCTAssertTrue(context.matches(document: id, before: "fixture", selected: nil, after: nil))
    XCTAssertFalse(context.matches(document: id, before: "fixtur", selected: nil, after: "e"))
    XCTAssertFalse(context.matches(document: UUID(), before: "fixture", selected: nil, after: nil))
    XCTAssertFalse(context.matches(document: id, before: "fixture", selected: "selected", after: nil))
  }
}

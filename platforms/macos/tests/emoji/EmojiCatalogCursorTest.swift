import Foundation

@main enum EmojiCatalogCursorTest {
  static let item: [String: String] = ["text": "synthetic", "annotation": "fixture", "group": "fixture"]
  static func page(_ count: Int, _ next: Int, _ complete: Bool) -> NSDictionary {
    ["items": Array(repeating: item, count: count), "next_offset": next, "complete": complete]
  }
  @MainActor static func main() async throws {
    var offsets: [Int] = []
    let result = try MacEmojiCatalogCursor.collect(revision: { 0 }) { offset, limit in
      assert(limit == 255); offsets.append(offset)
      switch offset {
      case 0: return page(0, 255, false)
      case 255: return page(255, 510, false)
      case 510: return page(1, 511, true)
      default: fatalError("Unexpected cursor offset")
      }
    }
    assert(offsets == [0, 255, 510] && result.count == 256)
    assert(result.first == result.last) // Valid duplicates are not removed at batch boundaries.
    let empty = try MacEmojiCatalogCursor.collect(revision: { 0 }, page: { _, _ in page(0, 0, true) })
    assert(empty.isEmpty)
    var prefixOffsets: [Int] = []
    let prefix = try MacEmojiCatalogCursor.collect(maximumItems: 3, revision: { 0 }) { offset, limit in
      prefixOffsets.append(offset)
      switch offset {
      case 0: assert(limit == 3); return page(0, 3, false)
      case 3: assert(limit == 3); return page(1, 6, false)
      case 6: assert(limit == 2); return page(2, 8, false)
      default: fatalError("Preview read beyond enough valid items")
      }
    }
    assert(prefixOffsets == [0, 3, 6] && prefix.count == 3 && prefix.first == prefix.last)
    let short = try MacEmojiCatalogCursor.collect(maximumItems: 18, revision: { 0 }) { _, limit in
      assert(limit == 18); return page(1, 2, true)
    }
    assert(short.count == 1)
    for limit in [0, -1] {
      do {
        _ = try MacEmojiCatalogCursor.collect(maximumItems: limit, revision: { 0 }) { _, _ in
          fatalError("Invalid preview limit invoked page")
        }
        assertionFailure("Invalid preview limit accepted")
      } catch { }
    }
    var previewRevision = 0
    do {
      _ = try MacEmojiCatalogCursor.collect(maximumItems: 1, revision: { previewRevision }) { _, _ in
        previewRevision += 1; return page(1, 1, false)
      }
      assertionFailure("Preview quota bypassed revision guard")
    } catch { }
    let malformed: [NSDictionary] = [
      ["items": []], ["items": [], "next_offset": true, "complete": true],
      ["items": [], "next_offset": -1, "complete": true],
      ["items": [], "next_offset": 0.5, "complete": true],
      ["items": [], "next_offset": NSNumber(value: UInt64.max), "complete": true],
      ["items": [], "next_offset": 0, "complete": 1],
      page(0, 0, false), page(0, 256, false), page(0, 255, true), page(1, 0, true),
      ["items": [], "next_offset": 0, "complete": true, "error": "synthetic"]
    ]
    for response in malformed {
      do { _ = try MacEmojiCatalogCursor.decode(response, offset: 0, limit: 255); assertionFailure("Invalid cursor accepted") }
      catch { }
    }
    do { _ = try MacEmojiCatalogCursor.decode(page(0, 1, true), offset: 2, limit: 255); assertionFailure() }
    catch { }
    var published: [MacEmojiCatalogItem]?
    do {
      published = try MacEmojiCatalogCursor.collect(revision: { 0 }) { offset, _ in
        if offset == 0 { return page(255, 255, false) }
        throw NSError(domain: "SyntheticFailure", code: 1)
      }
      assertionFailure("Partial failure ignored")
    } catch { assert(published == nil) }
    var revision = 0
    do {
      _ = try MacEmojiCatalogCursor.collect(revision: { revision }) { _, _ in
        revision += 1; return page(1, 1, true)
      }
      assertionFailure("Changed resource accepted")
    } catch { }
    var reads = 0
    let cancelled = Task { @MainActor in
      do {
        _ = try MacEmojiCatalogCursor.collect(revision: { 0 }) { _, _ in reads += 1; return page(0, 0, true) }
        assertionFailure("Cancellation ignored")
      } catch { assert(error is CancellationError) }
    }
    cancelled.cancel(); await cancelled.value
    assert(reads == 0)
    let midRead = Task { @MainActor in
      do {
        _ = try MacEmojiCatalogCursor.collect(revision: { 0 }) { _, _ in
          reads += 1
          withUnsafeCurrentTask { $0?.cancel() }
          return page(255, 255, false)
        }
        assertionFailure("Mid-read cancellation ignored")
      } catch { assert(error is CancellationError) }
    }
    await midRead.value
    assert(reads == 1)
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cursor-fixture-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    let database = directory.appendingPathComponent("msime-others.db")
    try Data("synthetic".utf8).write(to: database)
    let original = try MacEmojiCatalogRevision.capture(resources: directory.path)
    let unchanged = try MacEmojiCatalogRevision.capture(resources: directory.path)
    assert(original == unchanged)
    try Data([0]).write(to: directory.appendingPathComponent("msime-others.db-wal"))
    let withWal = try MacEmojiCatalogRevision.capture(resources: directory.path)
    assert(original != withWal)
    try Data("synthetic replacement".utf8).write(to: database, options: .atomic)
    let replaced = try MacEmojiCatalogRevision.capture(resources: directory.path)
    assert(replaced != withWal)
    print("Catalog cursor decoding, full collection, failure, cancellation and file-change guards passed")
  }
}

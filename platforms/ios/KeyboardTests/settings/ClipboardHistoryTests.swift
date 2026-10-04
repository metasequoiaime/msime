import XCTest
import UIKit

final class ClipboardHistoryTests: XCTestCase {
  func testTimestampRejectsMalformedNativeNumbers() {
    XCTAssertEqual(ClipboardHistoryStore.strictTimestampMilliseconds(NSNumber(value: 3)), 3)
    XCTAssertNil(ClipboardHistoryStore.strictTimestampMilliseconds(NSNumber(value: true)))
    XCTAssertNil(ClipboardHistoryStore.strictTimestampMilliseconds(NSNumber(value: 3.5)))
    XCTAssertNil(ClipboardHistoryStore.strictTimestampMilliseconds(NSNumber(value: -1)))
  }

  private func temporaryStore() throws -> ClipboardHistoryStore {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    return ClipboardHistoryStore(directory: directory)
  }
  func testDeduplicationPersistenceAndPinnedEviction() throws {
    let store = try temporaryStore()
    try store.add("  保留空格\n")
    try store.setPinned(true, text: "  保留空格\n")
    for number in 0..<60 { try store.add("记录 \(number)") }
    XCTAssertEqual(try store.load().count, 50)
    XCTAssertEqual(try store.load().first?.text, "  保留空格\n")
    XCTAssertEqual(try store.load().first?.pinned, true)
    XCTAssertFalse(try store.load().contains { $0.text == "记录 0" })
    try store.add("  保留空格\n")
    XCTAssertEqual(try store.load().first?.text, "  保留空格\n")
    XCTAssertEqual(try store.load().filter { $0.text == "  保留空格\n" }.count, 1)
    for item in try store.load() { try store.setPinned(true, text: item.text) }
    let items = try store.load()
    XCTAssertThrowsError(try store.add("满了"))
    XCTAssertEqual(try store.load(), items)
    try store.clear()
    XCTAssertTrue(try store.load().isEmpty)
  }
  func testInvalidInputAndCorruptHistoryDoNotOverwriteData() throws {
    let store = try temporaryStore()
    XCTAssertThrowsError(try store.add(" \n"))
    do {
      try store.add("前\0后")
      XCTFail("NUL clipboard text was accepted")
    } catch ClipboardHistoryStore.Failure.invalidText {
      // Expected: the native bridge and shared store cannot persist NUL.
    } catch {
      XCTFail("unexpected NUL rejection: \(error)")
    }
    XCTAssertThrowsError(try store.add(String(repeating: "字", count: 10_001)))
    try store.add("原文")
    try Data("invalid".utf8).write(to: store.file)
    XCTAssertThrowsError(try store.add("新的"))
    XCTAssertEqual(try Data(contentsOf: store.file), Data("invalid".utf8))
    try FileManager.default.removeItem(at: store.file)
    try store.clear()
    XCTAssertTrue(try store.load().isEmpty)
  }

  @MainActor func testPanelSelectionAccessGateAndNarrowLayout() throws {
    let store = try temporaryStore()
    try store.add("测试粘贴\n第二行")
    var inserted = ""
    let panel = KeyboardClipboardView(hasFullAccess: true, store: store, onInsert: { inserted = $0 }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 320, height: 260)
    panel.layoutIfNeeded()
    let table = try XCTUnwrap(panel.subviews.compactMap { $0 as? UITableView }.first)
    XCTAssertEqual(table.numberOfRows(inSection: 0), 1)
    panel.tableView(table, didSelectRowAt: IndexPath(row: 0, section: 0))
    XCTAssertEqual(inserted, "测试粘贴\n第二行")
    XCTAssertGreaterThan(table.bounds.height, 60)
    let header = try XCTUnwrap(panel.subviews.compactMap { $0 as? UIStackView }.first)
    let clear = try XCTUnwrap(header.arrangedSubviews.first { $0.accessibilityIdentifier == "clearClipboardHistory" } as? UIButton)
    clear.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try store.load().count, 1)
    clear.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try store.load().isEmpty)
    let gated = KeyboardClipboardView(hasFullAccess: false, store: store, onInsert: { _ in XCTFail() }, onClose: {})
    let capture = try XCTUnwrap(gated.subviews.compactMap { $0 as? UIButton }.first)
    XCTAssertFalse(capture.isEnabled)
    let hidden = try XCTUnwrap(gated.subviews.compactMap { $0 as? UITableView }.first)
    XCTAssertEqual(hidden.numberOfRows(inSection: 0), 0)
  }

  /// 键盘读取剪贴板内容会触发系统粘贴提示，所以只凭 changeCount 提示「有新复制的内容」，保存仍由用户点按。
  func testNewCopyPromptFollowsTheChangeCountSinceTheLastSave() throws {
    let suite = "msime-clipboard-prompt-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    XCTAssertTrue(ClipboardCapturePrompt.hasNewCopy(changeCount: 3, hasStrings: true, defaults: defaults))
    XCTAssertFalse(ClipboardCapturePrompt.hasNewCopy(changeCount: 3, hasStrings: false, defaults: defaults), "an image copy has no text to save")
    ClipboardCapturePrompt.markCaptured(changeCount: 3, defaults: defaults)
    XCTAssertFalse(ClipboardCapturePrompt.hasNewCopy(changeCount: 3, hasStrings: true, defaults: defaults))
    XCTAssertTrue(ClipboardCapturePrompt.hasNewCopy(changeCount: 4, hasStrings: true, defaults: defaults))
  }

  func testSearchMatchesSubstringsIgnoringCaseAndKeepsOrder() {
    let items = ["Hello World", "今天的会议记录", "hello again", "会议"].map { ClipboardHistoryItem(text: $0) }
    XCTAssertEqual(ClipboardHistoryItem.matching(items, query: ""), items)
    XCTAssertEqual(ClipboardHistoryItem.matching(items, query: "HELLO").map(\.text), ["Hello World", "hello again"])
    XCTAssertEqual(ClipboardHistoryItem.matching(items, query: "会议").map(\.text), ["今天的会议记录", "会议"])
    XCTAssertEqual(ClipboardHistoryItem.matching(items, query: "o w").map(\.text), ["Hello World"])
    XCTAssertTrue(ClipboardHistoryItem.matching(items, query: "不存在").isEmpty)
  }

  /// Windows' 搜索剪贴板 on a keyboard: a letter and digit pad filters the rows, a tap inserts the match, and 返回 leaves the search before the panel.
  @MainActor func testPanelSearchFiltersWithItsOwnPad() throws {
    let store = try temporaryStore()
    for text in ["会议链接 https://example.com/a1", "验证码 804512", "晚饭吃什么", "Example Draft"] { try store.add(text) }
    var inserted = ""
    var closed = false
    let panel = KeyboardClipboardView(hasFullAccess: true, store: store, onInsert: { inserted = $0 }, onClose: { closed = true })
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 300)
    panel.layoutIfNeeded()
    let table = try XCTUnwrap(panel.subviews.compactMap { $0 as? UITableView }.first)
    XCTAssertEqual(table.numberOfRows(inSection: 0), 4)

    try view("clipboardSearch", in: panel).sendActions(for: .primaryActionTriggered)
    panel.layoutIfNeeded()
    let pad = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "clipboardSearchPad" })
    XCTAssertFalse(pad.isHidden)
    XCTAssertTrue(try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "captureClipboard" }).isHidden)
    XCTAssertLessThanOrEqual(table.frame.maxY, pad.frame.minY)
    XCTAssertGreaterThanOrEqual(table.bounds.height, table.rowHeight, "the pad left no room for a single row on a phone-height panel")
    XCTAssertEqual(table.numberOfRows(inSection: 0), 4, "an empty query shows everything")

    for key in ["e", "x", "a", "m"] { try view("clipboardSearchKey-" + key, in: panel).sendActions(for: .primaryActionTriggered) }
    XCTAssertEqual(panel.searchQuery, "exam")
    XCTAssertEqual(table.numberOfRows(inSection: 0), 2, "the match ignores case")
    XCTAssertEqual((descendants(panel).first { $0.accessibilityIdentifier == "clipboardTitle" } as? UILabel)?.text, "搜索：exam")

    for _ in 0..<4 { try view("clipboardSearchDelete", in: panel).sendActions(for: .primaryActionTriggered) }
    for key in ["8", "0", "4"] { try view("clipboardSearchKey-" + key, in: panel).sendActions(for: .primaryActionTriggered) }
    XCTAssertEqual(table.numberOfRows(inSection: 0), 1)
    panel.tableView(table, didSelectRowAt: IndexPath(row: 0, section: 0))
    XCTAssertEqual(inserted, "验证码 804512")

    try view("clipboardSearchKey-q", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(table.numberOfRows(inSection: 0), 0)
    XCTAssertEqual((table.backgroundView as? UILabel)?.text, "没有匹配的记录")

    let back = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIButton }.first { $0.title(for: .normal) == "返回" })
    back.sendActions(for: .primaryActionTriggered)
    XCTAssertNil(panel.searchQuery)
    XCTAssertFalse(closed)
    XCTAssertTrue(pad.isHidden)
    XCTAssertEqual(table.numberOfRows(inSection: 0), 4)
    back.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(closed)
  }

  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }

  private func view(_ identifier: String, in root: UIView) throws -> UIControl {
    try XCTUnwrap(descendants(root).first { $0.accessibilityIdentifier == identifier } as? UIControl, identifier)
  }
}

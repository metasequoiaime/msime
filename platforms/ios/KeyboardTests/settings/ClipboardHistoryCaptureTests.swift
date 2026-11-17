import XCTest
import UIKit

@MainActor final class ClipboardHistoryCaptureTests: XCTestCase {
  private enum Failure: Error { case synthetic }

  func testContinuousCopiesKeepOriginalTextAcrossSaveRetry() {
    let buffer = ClipboardHistoryCaptureBuffer()
    buffer.start(changeCount: 0)
    var unavailable = true
    var saved: [String] = []
    var reads = 0
    let save: (Int, String) throws -> Void = { _, text in
      if unavailable { throw Failure.synthetic }
      saved.append(text)
    }
    for count in 1...3 {
      buffer.capture(changeCount: count, allowed: true, read: {
        reads += 1
        return "Synthetic copy \(count)"
      }, save: save)
    }
    XCTAssertTrue(buffer.hasPending)
    XCTAssertTrue(saved.isEmpty)
    unavailable = false
    buffer.capture(changeCount: 3, allowed: true, read: {
      XCTFail("重试不能改读当前正文")
      return "Synthetic replacement"
    }, save: save)
    XCTAssertEqual(saved, ["Synthetic copy 1", "Synthetic copy 2", "Synthetic copy 3"])
    XCTAssertEqual(reads, 3)
    XCTAssertFalse(buffer.hasPending)
  }

  func testDisabledAndStoppedCaptureNeverReadAndDiscardPending() {
    let buffer = ClipboardHistoryCaptureBuffer()
    buffer.start(changeCount: 4)
    var reads = 0
    var saved: [String] = []
    let read: () -> String? = { reads += 1; return "Synthetic private copy" }
    buffer.capture(changeCount: 4, allowed: true, read: read, save: { _, text in saved.append(text) })
    XCTAssertEqual(reads, 0, "启动不导入旧值")
    buffer.capture(changeCount: 5, allowed: false, read: read, save: { _, _ in XCTFail("关闭不保存") })
    XCTAssertEqual(reads, 0)
    buffer.capture(changeCount: 6, allowed: true, read: read, save: { _, _ in throw Failure.synthetic })
    XCTAssertTrue(buffer.hasPending)
    buffer.capture(changeCount: 6, allowed: false, read: read, save: { _, _ in XCTFail("隐私模式不重试") })
    XCTAssertFalse(buffer.hasPending)
    buffer.stop()
    buffer.capture(changeCount: 7, allowed: true, read: read, save: { _, _ in XCTFail("停止后不保存") })
    XCTAssertEqual(reads, 1)
    XCTAssertTrue(saved.isEmpty)
  }

  func testQueueIsBoundedAndRetainsNewestCopies() {
    let buffer = ClipboardHistoryCaptureBuffer()
    buffer.start(changeCount: 0)
    for count in 1...130 {
      buffer.capture(changeCount: count, allowed: true, read: { "Synthetic bounded \(count)" },
                     save: { _, _ in throw Failure.synthetic })
    }
    var saved: [String] = []
    buffer.capture(changeCount: 130, allowed: true, read: { XCTFail("没有变化不读取"); return nil },
                   save: { _, text in saved.append(text) })
    XCTAssertEqual(saved.count, ClipboardHistoryCaptureBuffer.capacity)
    XCTAssertEqual(saved.first, "Synthetic bounded 3")
    XCTAssertEqual(saved.last, "Synthetic bounded 130")
  }

  func testOversizedTextIsNotRetainedForRetry() {
    let buffer = ClipboardHistoryCaptureBuffer()
    buffer.start(changeCount: 0)
    buffer.capture(changeCount: 1, allowed: true,
      read: { String(repeating: "x", count: ClipboardHistoryCaptureBuffer.maximumTextBytes + 1) },
      save: { _, _ in XCTFail("超出共享上限的正文不保存") })
    XCTAssertFalse(buffer.hasPending)
  }

  func testIsolatedUIKitPasteboardNotificationsCaptureThreeCopiesAndStop() async throws {
    let board = UIPasteboard.withUniqueName()
    board.string = "Synthetic existing clipboard"
    var enabled = true
    var saved: [String] = []
    let monitor = KeyboardClipboardHistoryCapture(pasteboard: board, allowed: { enabled },
      save: { _, text in saved.append(text) })
    monitor.start()
    defer { monitor.stop(); UIPasteboard.remove(withName: board.name) }
    for count in 1...3 {
      board.string = "Synthetic UIKit copy \(count)"
      for _ in 0..<100 {
        if saved.count == count { break }
        try await Task.sleep(nanoseconds: 10_000_000)
      }
      XCTAssertEqual(saved.count, count)
    }
    XCTAssertEqual(saved, ["Synthetic UIKit copy 1", "Synthetic UIKit copy 2", "Synthetic UIKit copy 3"])
    enabled = false
    board.string = "Synthetic while disabled"
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertEqual(saved.count, 3)
    monitor.stop()
    enabled = true
    board.string = "Synthetic after stop"
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertEqual(saved.count, 3)
  }

  func testPrivacyTransitionDiscardsRetryBeforeReturningToNormalField() async throws {
    let board = UIPasteboard.withUniqueName()
    var allowed = true
    var unavailable = true
    var saved: [String] = []
    let monitor = KeyboardClipboardHistoryCapture(pasteboard: board, allowed: { allowed },
      save: { _, text in
        if unavailable { throw Failure.synthetic }
        saved.append(text)
      })
    monitor.start()
    defer { monitor.stop(); UIPasteboard.remove(withName: board.name) }
    board.string = "Synthetic before privacy"
    try await Task.sleep(nanoseconds: 50_000_000)
    allowed = false
    monitor.discardIfDisallowed()
    allowed = true
    unavailable = false
    try await Task.sleep(nanoseconds: 150_000_000)
    XCTAssertTrue(saved.isEmpty, "退出隐私状态不能恢复旧的待保存内容")
    board.string = "Synthetic after privacy"
    for _ in 0..<100 {
      if !saved.isEmpty { break }
      try await Task.sleep(nanoseconds: 10_000_000)
    }
    XCTAssertEqual(saved, ["Synthetic after privacy"])
  }

  func testSensitivePasteboardTypesAreExcluded() async throws {
    let board = UIPasteboard.withUniqueName()
    var saved: [String] = []
    let monitor = KeyboardClipboardHistoryCapture(pasteboard: board, allowed: { true },
      save: { _, text in saved.append(text) })
    monitor.start()
    defer { monitor.stop(); UIPasteboard.remove(withName: board.name) }
    board.setItems([["public.utf8-plain-text": "Synthetic sensitive text",
                     "org.nspasteboard.ConcealedType": Data()]])
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertTrue(saved.isEmpty)
    board.string = "Synthetic ordinary text"
    for _ in 0..<100 {
      if !saved.isEmpty { break }
      try await Task.sleep(nanoseconds: 10_000_000)
    }
    XCTAssertEqual(saved, ["Synthetic ordinary text"])
  }
}

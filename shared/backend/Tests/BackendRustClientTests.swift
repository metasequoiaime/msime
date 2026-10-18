import CMSIMEClient
import Foundation
import XCTest
@testable import MSIMEBackend

/// 替身 Rust：C 回调不能捕获变量，状态都放在这里。测试类内的用例串行运行。
private enum FakeRust {
  static let lock = NSLock()
  static var frees = 0
  static var lastRequest = Data()
  static var callThread: String?
  static var callOnMain = true
  static var chatCancels: [String] = []
  static var skinCancels: [String] = []
  static let cancelled = DispatchSemaphore(value: 0)

  static func record(_ request: UnsafePointer<UInt8>?, _ length: Int) {
    lock.lock(); defer { lock.unlock() }
    lastRequest = request.map { Data(bytes: $0, count: length) } ?? Data()
    callThread = Thread.current.name
    callOnMain = Thread.isMainThread
  }

  static func reply(_ text: String) -> UnsafeMutablePointer<CChar>? { strdup(text) }

  static func table() -> msime_rust_client {
    var table = msime_rust_client()
    table.version = MSIME_RUST_CLIENT_VERSION
    table.size = UInt32(MemoryLayout<msime_rust_client>.stride)
    table.string_free = { value in
      FakeRust.lock.lock(); FakeRust.frees += 1; FakeRust.lock.unlock()
      free(value)
    }
    table.session_store_register = { _ in FakeRust.reply(#"{"ok":true,"value":{"session_store":7}}"#) }
    table.session_store_unregister = { _ in FakeRust.reply(#"{"ok":true,"value":true}"#) }
    table.account = { request, length in
      FakeRust.record(request, length)
      return FakeRust.reply(#"{"ok":true,"value":{"user":{"id":"synthetic-user"}}}"#)
    }
    table.community = { request, length in
      FakeRust.record(request, length)
      return FakeRust.reply(#"{"ok":false,"error":"account_rate_limited","retry_after_seconds":42}"#)
    }
    table.account_chat_cancel = { request, length in
      let text = String(decoding: UnsafeBufferPointer(start: request, count: length), as: UTF8.self)
      FakeRust.lock.lock(); FakeRust.chatCancels.append(text); FakeRust.lock.unlock()
      FakeRust.cancelled.signal()
      return FakeRust.reply(#"{"ok":true,"value":true}"#)
    }
    table.ai_skin_cancel = { request, length in
      let text = String(decoding: UnsafeBufferPointer(start: request, count: length), as: UTF8.self)
      FakeRust.lock.lock(); FakeRust.skinCancels.append(text); FakeRust.lock.unlock()
      FakeRust.cancelled.signal()
      return FakeRust.reply(#"{"ok":true,"value":true}"#)
    }
    return table
  }

  /// 在同步上下文里读状态，异步测试里不直接碰 `NSLock`。
  static func snapshot() -> (frees: Int, request: Data, thread: String?, onMain: Bool, chatCancels: [String], skinCancels: [String]) {
    lock.lock(); defer { lock.unlock() }
    return (frees, lastRequest, callThread, callOnMain, chatCancels, skinCancels)
  }

  static func reset() {
    lock.lock(); defer { lock.unlock() }
    frees = 0; lastRequest = Data(); callThread = nil; callOnMain = true; chatCancels = []; skinCancels = []
    while cancelled.wait(timeout: .now()) == .success {}
  }
}

private func deliver(_ text: String, _ callback: msime_client_account_chat_delta_callback?, _ context: UnsafeMutableRawPointer?) {
  let bytes = Array(text.utf8)
  bytes.withUnsafeBufferPointer { callback?(context, $0.baseAddress, $0.count) }
}

final class BackendRustClientTests: XCTestCase {
  override func setUp() { FakeRust.reset() }

  func testTableIsRejectedWithAnotherVersionSizeOrWithoutTheRequiredFunctions() {
    XCTAssertNotNil(BackendRustClient(table: FakeRust.table()))
    var table = FakeRust.table(); table.version = MSIME_RUST_CLIENT_VERSION + 1
    XCTAssertNil(BackendRustClient(table: table))
    table = FakeRust.table(); table.size -= 8
    XCTAssertNil(BackendRustClient(table: table))
    table = FakeRust.table(); table.session_store_register = nil
    XCTAssertNil(BackendRustClient(table: table))
    table = FakeRust.table(); table.string_free = nil
    XCTAssertNil(BackendRustClient(table: table))
    XCTAssertFalse(BackendRustClient.install(table))
  }

  func testEnvelopeSplitsValueAndFailure() throws {
    XCTAssertEqual(try BackendRustClient.envelope(Data(#"{"ok":true,"value":{"a":1}}"#.utf8)).get(), Data(#"{"a":1}"#.utf8))
    XCTAssertEqual(try BackendRustClient.envelope(Data(#"{"ok":true,"value":null}"#.utf8)).get(), Data("null".utf8))
    XCTAssertEqual(try BackendRustClient.envelope(Data(#"{"ok":true}"#.utf8)).get(), Data("null".utf8))
    let failure = Data(#"{"ok":false,"error":"account_network","status":0}"#.utf8)
    XCTAssertEqual(BackendRustClient.envelope(failure), .failure(.init(code: "account_network", response: failure)))
    // `ok` 必须是 JSON 的布尔值，数字 1 不算成功；不是信封的回答都是 rust_client_invalid_response。
    XCTAssertEqual(BackendRustClient.envelope(Data(#"{"ok":1,"value":2}"#.utf8)), .failure(.invalidResponse))
    XCTAssertEqual(BackendRustClient.envelope(Data(#"{"ok":false}"#.utf8)), .failure(.invalidResponse))
    XCTAssertEqual(BackendRustClient.envelope(Data("not json".utf8)), .failure(.invalidResponse))
  }

  func testCallRunsOnADedicatedThreadAndFreesTheReplyOnce() async throws {
    let client = try XCTUnwrap(BackendRustClient(table: FakeRust.table()))
    let value = try await client.call(.account, ["action": ["operation": "status"], "session_store": 7])
    XCTAssertEqual((try JSONSerialization.jsonObject(with: value) as? [String: [String: String]])?["user"]?["id"], "synthetic-user")
    let seen = FakeRust.snapshot()
    XCTAssertEqual(seen.thread, "app.msime.rust-client")
    XCTAssertFalse(seen.onMain)
    XCTAssertEqual(seen.frees, 1)
    let sent = try JSONSerialization.jsonObject(with: seen.request) as? [String: Any]
    XCTAssertEqual(sent?["session_store"] as? Int, 7)
  }

  func testFailuresCarryTheCodeAndRetryAfter() async throws {
    let client = try XCTUnwrap(BackendRustClient(table: FakeRust.table()))
    do {
      _ = try await client.call(.community, ["action": ["operation": "candidate_skin_community_list"]])
      XCTFail("rate limited call succeeded")
    } catch let failure as BackendRustClient.Failure {
      XCTAssertEqual(failure.code, "account_rate_limited")
      XCTAssertEqual(failure.retryAfterSeconds, 42)
    }
    XCTAssertEqual(FakeRust.snapshot().frees, 1)
  }

  func testMissingEntryAndUnencodableRequestFailLocally() async throws {
    let client = try XCTUnwrap(BackendRustClient(table: FakeRust.table()))
    do { _ = try await client.call(.cloudSync, ["action": ["operation": "status"]]); XCTFail("called a missing entry") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure, .unavailable) }
    do { _ = try await client.call(.account, ["bad": Date()]); XCTFail("encoded a Date") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure, .invalidRequest) }
    XCTAssertEqual(FakeRust.snapshot().frees, 0)
  }

  func testChatStreamYieldsEveryDeltaThenFinishes() async throws {
    var table = FakeRust.table()
    table.account_chat_stream = { _, _, onDelta, context in
      for part in ["水杉", "输入", "法"] { deliver(part, onDelta, context) }
      return FakeRust.reply(#"{"ok":true,"value":{"content":"水杉输入法"}}"#)
    }
    let client = try XCTUnwrap(BackendRustClient(table: table))
    var parts: [String] = []
    for try await part in client.chatStream(requestID: "chat-1", request: Data(#"{"request_id":"chat-1"}"#.utf8)) { parts.append(part) }
    XCTAssertEqual(parts, ["水杉", "输入", "法"])
    XCTAssertEqual(FakeRust.snapshot().frees, 1)
    XCTAssertTrue(FakeRust.snapshot().chatCancels.isEmpty)
  }

  func testChatStreamFailureEndsTheStreamWithTheCode() async throws {
    var table = FakeRust.table()
    table.account_chat_stream = { _, _, onDelta, context in
      deliver("半", onDelta, context)
      return FakeRust.reply(#"{"ok":false,"error":"account_network"}"#)
    }
    let client = try XCTUnwrap(BackendRustClient(table: table))
    var parts: [String] = []
    do {
      for try await part in client.chatStream(requestID: "chat-2", request: Data()) { parts.append(part) }
      XCTFail("stream finished without the failure")
    } catch let failure as BackendRustClient.Failure {
      XCTAssertEqual(failure.code, "account_network")
    }
    XCTAssertEqual(parts, ["半"])
  }

  func testCancellingTheConsumerCancelsTheRequestByID() async throws {
    var table = FakeRust.table()
    // 等到宿主调了取消才返回，与 Rust 在取消后立即以 account_cancelled 返回相同。
    table.account_chat_stream = { _, _, onDelta, context in
      deliver("第一段", onDelta, context)
      _ = FakeRust.cancelled.wait(timeout: .now() + 10)
      return FakeRust.reply(#"{"ok":false,"error":"account_cancelled"}"#)
    }
    let client = try XCTUnwrap(BackendRustClient(table: table))
    let consumer = Task {
      for try await _ in client.chatStream(requestID: "chat-3", request: Data()) { break }
    }
    _ = try? await consumer.value
    // 跳出循环就是消费方不再读，流随之以 cancelled 结束。
    for _ in 0..<100 where FakeRust.snapshot().chatCancels.isEmpty { try await Task.sleep(nanoseconds: 20_000_000) }
    XCTAssertEqual(FakeRust.snapshot().chatCancels, [#"{"request_id":"chat-3"}"#])
  }

  func testAISkinGenerateReportsProgressAndCancelsByID() async throws {
    var table = FakeRust.table()
    table.ai_skin_generate = { _, _, progress, context in
      for completed in UInt32(1)...3 { progress?(context, completed) }
      return FakeRust.reply(#"{"ok":true,"value":[]}"#)
    }
    let client = try XCTUnwrap(BackendRustClient(table: table))
    final class Seen: @unchecked Sendable {
      private let lock = NSLock(); private var stored: [UInt32] = []
      func append(_ value: UInt32) { lock.lock(); stored.append(value); lock.unlock() }
      var values: [UInt32] { lock.lock(); defer { lock.unlock() }; return stored }
    }
    let seen = Seen()
    let value = try await client.aiSkinGenerate(requestID: "skin-1", request: Data()) { seen.append($0) }
    XCTAssertEqual(value, Data("[]".utf8))
    XCTAssertEqual(seen.values, [1, 2, 3])

    table.ai_skin_generate = { _, _, _, _ in
      _ = FakeRust.cancelled.wait(timeout: .now() + 10)
      return FakeRust.reply(#"{"ok":false,"error":"ai_skin_cancelled"}"#)
    }
    let waiting = try XCTUnwrap(BackendRustClient(table: table))
    let task = Task { try await waiting.aiSkinGenerate(requestID: "skin-2", request: Data()) }
    try await Task.sleep(nanoseconds: 100_000_000)
    task.cancel()
    do { _ = try await task.value; XCTFail("cancelled generation succeeded") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure.code, "ai_skin_cancelled") }
    XCTAssertEqual(FakeRust.snapshot().skinCancels, [#"{"request_id":"skin-2"}"#])
  }
}

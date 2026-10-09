import Foundation
import CryptoKit
import XCTest

@MainActor
final class DictionarySnapshotWorkerTests: XCTestCase {
  func testQueuedSnapshotForAnotherAccountIsCancelledBeforePreparation() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let session = MetasequoiaInputSessionBridge(resources: resources,
      stateRoot: root.appendingPathComponent("State", isDirectory: true))
    let version = try session.localDictionaryStateVersion()
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let queue = DictionarySnapshotQueue(directory: root)
    try queue.publishLocalVersion(version)
    let source = root.appendingPathComponent("snapshot.ndjson")
    let contents = Data("synthetic snapshot".utf8)
    try contents.write(to: source)
    let digest = SHA256.hash(data: contents).map { String(format: "%02x", $0) }.joined()
    _ = try queue.enqueue(file: source, accountID: "previous-account", cloudRevision: 1,
      expectedLocalVersion: version, fileSHA256: digest)
    let worker = DictionarySnapshotWorker(session: session, queue: queue,
      currentAccountID: { "replacement-account" })

    worker.tick(idle: true, fullAccess: true)

    XCTAssertEqual(try queue.read().request?.status, .cancelled)
    XCTAssertEqual(try session.localDictionaryStateVersion(), version)
    XCTAssertFalse(worker.isPreparing)
  }

  func testQueuedSnapshotRunsThroughBackgroundPreparationAndActualSessionActivation() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let stateRoot = root.appendingPathComponent("State", isDirectory: true)
    var session: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(resources: resources, stateRoot: stateRoot)
    let originalVersion = try session!.localDictionaryStateVersion()
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let queue = DictionarySnapshotQueue(directory: root)
    try queue.publishLocalVersion(originalVersion)
    let entry = #"{"id":"worker-fixture","kind":"quick","code":"workerfixture","word":"后台应用合成词条","weight":100000,"revision":1,"updated_at":"2026-09-08T00:00:00Z"}"#
    let lines = [#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":1}"#,
      "{\"type\":\"entry\",\"data\":\(entry)}", "{\"type\":\"overlay\",\"deleted\":false,\"data\":\(entry)}"]
    let body = Data((lines.joined(separator: "\n") + "\n").utf8)
    let digest = SHA256.hash(data: body).map { String(format: "%02x", $0) }.joined()
    let footer = try JSONSerialization.data(withJSONObject: ["type": "footer", "records": 3, "sha256": digest])
    let source = root.appendingPathComponent("snapshot.ndjson")
    try (body + footer + Data([10])).write(to: source)
    let preview = try BackendPreparedSnapshot(copying: source)
    let id = try queue.enqueue(file: preview.url, accountID: "synthetic-worker", cloudRevision: 1,
      expectedLocalVersion: originalVersion, fileSHA256: preview.fileSHA256)
    var worker: DictionarySnapshotWorker? = DictionarySnapshotWorker(session: session!, queue: queue,
      currentAccountID: { "synthetic-worker" })
    var cleaned = false
    defer {
      if !cleaned {
        worker?.stop(); worker = nil; session = nil
        try? FileManager.default.removeItem(at: root)
      }
    }
    var messages: [String] = []
    worker?.report = { messages.append($0) }
    worker?.tick(idle: false, fullAccess: true)
    XCTAssertEqual(try queue.read().request?.status, .queued)
    worker?.tick(idle: true, fullAccess: false)
    XCTAssertEqual(try queue.read().request?.status, .queued)
    worker?.tick(idle: true, fullAccess: true)
    worker?.stop()
    let firstPreparation = Date()
    while worker?.isPreparing == true { try await Task.sleep(nanoseconds: 30_000_000) }
    let firstPreparationSeconds = Date().timeIntervalSince(firstPreparation)
    XCTAssertEqual(try session!.localDictionaryStateVersion(), originalVersion)
    XCTAssertEqual(try queue.read().request?.id, id)
    XCTAssertEqual(try queue.read().request?.status, .preparing)
    // A bound for a stuck worker, not a budget for a slow one. The whole case takes 10 to 20 s on an idle runner, but one preparation copies and opens the dictionaries and took 23 s on a loaded one, and this wait holds a second preparation plus the activation; 30 s timed out with the worker still preparing and nothing wrong.
    let retry = Date()
    let deadline = retry.addingTimeInterval(120)
    while Date() < deadline {
      worker?.tick(idle: true, fullAccess: true)
      if try queue.read().request?.status == .applied { break }
      try await Task.sleep(nanoseconds: 30_000_000)
    }
    // The worker leaves a job where it is, without a message, when the job is still preparing, when the worker lease is held elsewhere, or when activation reports the snapshot busy; name which one, so a timeout here says why.
    var stalled = messages
    if try queue.read().request?.status != .applied {
      stalled.append("preparing: \(worker?.isPreparing == true)")
      let prepared = worker.flatMap { Mirror(reflecting: $0).descendant("prepared") }.map { String(describing: $0) } ?? "unknown"
      stalled.append("prepared: \(prepared != "nil")")
      stalled.append("worker lease free: \((try? queue.acquireWorkerLease()) != nil)")
      stalled.append(String(format: "first preparation %.1f s, waited %.1f s", firstPreparationSeconds, Date().timeIntervalSince(retry)))
    }
    XCTAssertEqual(try queue.read().request?.status, .applied, stalled.joined(separator: "; "))
    if try queue.read().request?.status == .applied {
      let page = try session!.personalEntries(atOffset: 0)
      let rows = try XCTUnwrap(page["entries"] as? [[String: Any]])
      XCTAssertEqual(rows.count, 1)
      XCTAssertEqual(rows.first?["value"] as? String, "后台应用合成词条")
      XCTAssertTrue(try session!.localDictionaryStateVersion().hasPrefix("local-v1:" + id.uuidString + ":"))
    }
    worker?.stop()
    while worker?.isPreparing == true { try await Task.sleep(nanoseconds: 30_000_000) }
    let appliedVersion = try session!.localDictionaryStateVersion()
    worker = nil; session = nil
    var restored: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(resources: resources, stateRoot: stateRoot)
    XCTAssertEqual(try restored!.localDictionaryStateVersion(), appliedVersion)
    restored = nil
    try FileManager.default.removeItem(at: root)
    cleaned = true
  }
}

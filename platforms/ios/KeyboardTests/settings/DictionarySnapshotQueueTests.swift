import Foundation
import CryptoKit
import XCTest
import Darwin

final class DictionarySnapshotQueueTests: XCTestCase {
  func testSnapshotDirectorySymlinkFailsClosedBeforeCreatingExternalState() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-directory-link-test-\(UUID().uuidString)")
    let outsideDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-directory-link-target-\(UUID().uuidString)")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outsideDirectory)
    }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outsideDirectory, withIntermediateDirectories: true)
    try FileManager.default.createSymbolicLink(
      at: root.appendingPathComponent("DictionarySnapshots", isDirectory: true),
      withDestinationURL: outsideDirectory)

    do {
      try DictionarySnapshotQueue(directory: root).publishLocalVersion(first)
      XCTFail("a symlinked snapshot directory must be rejected")
    } catch DictionarySnapshotQueue.Failure.unavailable {
      // Expected: snapshot state must stay inside the App Group directory.
    } catch {
      XCTFail("unexpected error: \(error)")
    }
    XCTAssertFalse(FileManager.default.fileExists(atPath: outsideDirectory.appendingPathComponent("state.lock").path))
  }

  func testStateLockSymlinkFailsClosedBeforeLockingExternalTarget() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-state-lock-test-\(UUID().uuidString)")
    let outsideDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-state-target-\(UUID().uuidString)")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outsideDirectory)
    }
    let queueDirectory = root.appendingPathComponent("DictionarySnapshots", isDirectory: true)
    try FileManager.default.createDirectory(at: queueDirectory, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outsideDirectory, withIntermediateDirectories: true)
    let outsideLock = outsideDirectory.appendingPathComponent("outside.lock")
    try Data("synthetic-lock-target".utf8).write(to: outsideLock)
    let descriptor = open(outsideLock.path, O_RDWR)
    guard descriptor >= 0 else { throw CocoaError(.fileNoSuchFile) }
    defer { flock(descriptor, LOCK_UN); close(descriptor) }
    XCTAssertEqual(flock(descriptor, LOCK_EX | LOCK_NB), 0)
    try FileManager.default.createSymbolicLink(
      at: queueDirectory.appendingPathComponent("state.lock"), withDestinationURL: outsideLock)

    do {
      try DictionarySnapshotQueue(directory: root).publishLocalVersion(first)
      XCTFail("a symlinked state lock must be rejected")
    } catch DictionarySnapshotQueue.Failure.unavailable {
      // Expected: the lock entry itself is not followed.
    } catch {
      XCTFail("unexpected error: \(error)")
    }
    XCTAssertEqual(try Data(contentsOf: outsideLock), Data("synthetic-lock-target".utf8))
  }

  func testWorkerLockSymlinkFailsClosedBeforeLockingExternalTarget() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-worker-lock-test-\(UUID().uuidString)")
    let outsideDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-worker-target-\(UUID().uuidString)")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outsideDirectory)
    }
    let queueDirectory = root.appendingPathComponent("DictionarySnapshots", isDirectory: true)
    try FileManager.default.createDirectory(at: queueDirectory, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outsideDirectory, withIntermediateDirectories: true)
    let outsideLock = outsideDirectory.appendingPathComponent("outside.lock")
    try Data("synthetic-lock-target".utf8).write(to: outsideLock)
    let descriptor = open(outsideLock.path, O_RDWR)
    guard descriptor >= 0 else { throw CocoaError(.fileNoSuchFile) }
    defer { flock(descriptor, LOCK_UN); close(descriptor) }
    XCTAssertEqual(flock(descriptor, LOCK_EX | LOCK_NB), 0)
    try FileManager.default.createSymbolicLink(
      at: queueDirectory.appendingPathComponent("worker.lock"), withDestinationURL: outsideLock)

    do {
      _ = try DictionarySnapshotQueue(directory: root).acquireWorkerLease()
      XCTFail("a symlinked worker lock must be rejected")
    } catch DictionarySnapshotQueue.Failure.unavailable {
      // Expected: the lock entry itself is not followed.
    } catch {
      XCTFail("unexpected error: \(error)")
    }
    XCTAssertEqual(try Data(contentsOf: outsideLock), Data("synthetic-lock-target".utf8))
  }

  func testStateFileSymlinkFailsClosedBeforeReadingExternalState() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-state-file-link-test-\(UUID().uuidString)")
    let outsideDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-state-file-target-\(UUID().uuidString)")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outsideDirectory)
    }
    let queueDirectory = root.appendingPathComponent("DictionarySnapshots", isDirectory: true)
    try FileManager.default.createDirectory(at: queueDirectory, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outsideDirectory, withIntermediateDirectories: true)
    let externalState = outsideDirectory.appendingPathComponent("state.json")
    try Data(#"{"version":1,"localVersion":null,"request":null}"#.utf8).write(to: externalState)
    try FileManager.default.createSymbolicLink(
      at: queueDirectory.appendingPathComponent("state.json"), withDestinationURL: externalState)

    do {
      _ = try DictionarySnapshotQueue(directory: root).read()
      XCTFail("a symlinked state file must be rejected")
    } catch DictionarySnapshotQueue.Failure.unavailable {
      // Expected: queue metadata must stay inside the App Group directory.
    } catch {
      XCTFail("unexpected error: \(error)")
    }
    XCTAssertEqual(try Data(contentsOf: externalState), Data(#"{"version":1,"localVersion":null,"request":null}"#.utf8))
  }

  private let first = "local-v1:legacy:" + String(repeating: "a", count: 64)
  private let second = "local-v1:legacy:" + String(repeating: "b", count: 64)
  private func fixture(_ action: (DictionarySnapshotQueue, URL, String, URL) throws -> Void) throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let file = root.appendingPathComponent("selected.ndjson")
    let data = Data("synthetic opaque handoff file".utf8)
    try data.write(to: file)
    let hash = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    let queue = DictionarySnapshotQueue(directory: root)
    try queue.publishLocalVersion(first)
    try action(queue, file, hash, root)
  }
  private func enqueue(_ queue: DictionarySnapshotQueue, _ file: URL, _ hash: String) throws -> UUID {
    try queue.enqueue(file: file, accountID: "synthetic-account", cloudRevision: 7, expectedLocalVersion: first, fileSHA256: hash)
  }
  func testSingleWorkerAndResumeKeepsRequestIdentity() throws {
    try fixture { queue, file, hash, root in
      let id = try enqueue(queue, file, hash)
      var lease: DictionarySnapshotQueue.WorkerLease? = try queue.acquireWorkerLease()
      XCTAssertThrowsError(try DictionarySnapshotQueue(directory: root).acquireWorkerLease())
      XCTAssertEqual(try queue.claim(using: XCTUnwrap(lease))?.id, id)
      lease = nil
      let restarted = DictionarySnapshotQueue(directory: root)
      let newLease = try restarted.acquireWorkerLease()
      let request = try XCTUnwrap(restarted.claim(using: newLease))
      XCTAssertEqual(request.id, id)
      XCTAssertEqual(request.status, .preparing)
      XCTAssertTrue(try restarted.complete(id: id, using: newLease, currentVersion: first, alreadyApplied: false) { second })
      XCTAssertEqual(try restarted.read().request?.status, .applied)
      XCTAssertEqual(try restarted.read().localVersion, second)
      XCTAssertFalse(FileManager.default.fileExists(atPath: try restarted.fileURL(for: request).path))
    }
  }
  func testLocalChangesRejectApplicationAndRemoveTransferFile() throws {
    try fixture { queue, file, hash, _ in
      let id = try enqueue(queue, file, hash)
      let lease = try queue.acquireWorkerLease()
      let request = try XCTUnwrap(queue.claim(using: lease))
      XCTAssertFalse(try queue.complete(id: id, using: lease, currentVersion: second, alreadyApplied: false) {
        XCTFail("stale snapshot applied"); return self.second
      })
      XCTAssertEqual(try queue.read().request?.status, .conflict)
      XCTAssertFalse(FileManager.default.fileExists(atPath: try queue.fileURL(for: request).path))
    }
  }
  func testCancellationRejectsLateCompletionAndIsAccountScoped() throws {
    try fixture { queue, file, hash, _ in
      let id = try enqueue(queue, file, hash)
      let lease = try queue.acquireWorkerLease()
      _ = try queue.claim(using: lease)
      try queue.cancel(accountID: "different-account")
      XCTAssertEqual(try queue.read().request?.status, .preparing)
      try queue.cancel(accountID: "synthetic-account")
      XCTAssertThrowsError(try queue.complete(id: id, using: lease, currentVersion: first, alreadyApplied: false) {
        XCTFail("cancelled snapshot applied"); return self.second
      })
      XCTAssertEqual(try queue.read().request?.status, .cancelled)
    }
  }
  func testPublishedButUnacknowledgedRequestIsNotReapplied() throws {
    try fixture { queue, file, hash, _ in
      let id = try enqueue(queue, file, hash)
      let lease = try queue.acquireWorkerLease()
      _ = try queue.claim(using: lease)
      var applications = 0
      XCTAssertThrowsError(try queue.complete(id: id, using: lease, currentVersion: first, alreadyApplied: false) {
        applications += 1
        throw CocoaError(.fileWriteUnknown) // Simulate failure after durable Engine publication.
      })
      XCTAssertEqual(try queue.read().request?.status, .preparing)
      XCTAssertTrue(try queue.complete(id: id, using: lease, currentVersion: second, alreadyApplied: true) {
        applications += 1; return self.second
      })
      XCTAssertEqual(applications, 1)
      XCTAssertEqual(try queue.read().request?.status, .applied)
    }
  }
  func testLateActivationReceiptDoesNotResurrectCancelledRequest() throws {
    try fixture { queue, file, hash, _ in
      let id = try enqueue(queue, file, hash)
      let lease = try queue.acquireWorkerLease()
      _ = try XCTUnwrap(queue.claim(using: lease))
      XCTAssertThrowsError(try queue.complete(id: id, using: lease, currentVersion: first, alreadyApplied: false) {
        throw CocoaError(.fileWriteUnknown)
      })
      try queue.cancel(accountID: "synthetic-account")
      let published = "local-v1:" + id.uuidString + ":" + String(repeating: "b", count: 64)
      try queue.publishLocalVersion(published)
      XCTAssertEqual(try queue.read().request?.status, .cancelled)
      XCTAssertEqual(try queue.read().localVersion, published)
    }
  }
  func testChangedFileAndBusyQueueDoNotReplaceExistingRequest() throws {
    try fixture { queue, file, hash, _ in
      try Data("changed source".utf8).write(to: file)
      XCTAssertThrowsError(try enqueue(queue, file, hash))
      XCTAssertNil(try queue.read().request)
      let data = Data("synthetic opaque handoff file".utf8)
      try data.write(to: file)
      let id = try enqueue(queue, file, hash)
      XCTAssertThrowsError(try enqueue(queue, file, hash))
      XCTAssertEqual(try queue.read().request?.id, id)
      try queue.fail(id: id)
      XCTAssertEqual(try queue.read().request?.status, .failed)
    }
  }

  func testEnqueueRejectsASymlinkedSource() throws {
    try fixture { queue, file, hash, _ in
      let target = file.deletingLastPathComponent().appendingPathComponent("target.ndjson")
      try Data("synthetic opaque handoff file".utf8).write(to: target)
      try FileManager.default.removeItem(at: file)
      try FileManager.default.createSymbolicLink(at: file, withDestinationURL: target)

      XCTAssertThrowsError(try enqueue(queue, file, hash))
      XCTAssertNil(try queue.read().request)
    }
  }
}

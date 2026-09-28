import Foundation
import XCTest
#if canImport(Darwin)
import Darwin
#endif
@testable import MSIMEBackend

final class BackendLocalStoreTests: XCTestCase {
  func testStoreSerializesAccessThroughAStableLockFile() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-store-test-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = BackendLocalStore(fileName: "session.json", directory: directory)
    let user = BackendAccountClient.User(id: "synthetic-user", display_name: "", created_at: "2026-09-26")
    let tokens = BackendAccountClient.Tokens(access_token: String(repeating: "a", count: 64),
      refresh_token: String(repeating: "b", count: 64), token_type: "Bearer", expires_in: 900, user: user)
    try store.save(BackendSavedSession(tokens: tokens, expiresAt: Date()))
    XCTAssertNotNil(try store.load())
    XCTAssertTrue(FileManager.default.fileExists(atPath: directory.appendingPathComponent("backend-local-store.lock").path))
    #if canImport(Darwin)
    let lockPath = directory.appendingPathComponent("backend-local-store.lock").path
    let permissions = try FileManager.default.attributesOfItem(atPath: lockPath)[.posixPermissions] as? NSNumber
    XCTAssertEqual(permissions?.intValue, 0o600)

    // A second descriptor must wait on the same lock, which is the cross-process
    // guarantee app and keyboard extension sessions rely on.
    let descriptor = open(lockPath, O_RDWR)
    XCTAssertGreaterThanOrEqual(descriptor, 0)
    XCTAssertEqual(flock(descriptor, LOCK_EX | LOCK_NB), 0)
    let started = DispatchSemaphore(value: 0)
    let finished = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
      started.signal()
      try? store.save(BackendSavedSession(tokens: tokens, expiresAt: Date()))
      finished.signal()
    }
    XCTAssertEqual(started.wait(timeout: .now() + 1), .success)
    XCTAssertEqual(finished.wait(timeout: .now() + 0.05), .timedOut)
    XCTAssertEqual(flock(descriptor, LOCK_UN), 0)
    XCTAssertEqual(finished.wait(timeout: .now() + 1), .success)
    close(descriptor)
    #endif
  }

  func testWriteIfAbsentAllowsOnlyOneCreator() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-store-create-test-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = BackendLocalStore(fileName: "anonymous-account.json", directory: directory)
    let first = Data("synthetic-first".utf8)
    let second = Data("synthetic-second".utf8)
    let outcomesLock = NSLock()
    var outcomes = [Bool]()
    let firstDone = expectation(description: "first creator")
    let secondDone = expectation(description: "second creator")
    DispatchQueue.global().async {
      let won = store.writeIfAbsent(first)
      outcomesLock.lock(); outcomes.append(won); outcomesLock.unlock()
      firstDone.fulfill()
    }
    DispatchQueue.global().async {
      let won = store.writeIfAbsent(second)
      outcomesLock.lock(); outcomes.append(won); outcomesLock.unlock()
      secondDone.fulfill()
    }
    wait(for: [firstDone, secondDone], timeout: 2)
    XCTAssertEqual(outcomes.filter { $0 }.count, 1)
    let value = try Data(contentsOf: directory.appendingPathComponent("anonymous-account.json"))
    XCTAssertTrue(value == first || value == second)
    XCTAssertFalse(store.writeIfAbsent(Data("synthetic-third".utf8)))
  }

  func testLoadRejectsAnOversizedSessionDocument() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-store-limit-test-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try Data(repeating: 0x41, count: BackendLocalStore.maximumSessionBytes + 1)
      .write(to: directory.appendingPathComponent("session.json"))
    XCTAssertThrowsError(try BackendLocalStore(fileName: "session.json", directory: directory).load())
  }
}

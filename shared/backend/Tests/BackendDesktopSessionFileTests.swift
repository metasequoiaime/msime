#if os(macOS)
import Foundation
import XCTest
@testable import MSIMEBackend

final class BackendDesktopSessionFileTests: XCTestCase {
  private var directory: URL!

  override func setUpWithError() throws {
    directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("desktop-session-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: directory)
  }

  private func session(_ access: String, expiresIn seconds: TimeInterval = 900) -> BackendSavedSession {
    .init(tokens: .init(access_token: String(repeating: access, count: 64), refresh_token: String(repeating: "f", count: 64),
                        token_type: "Bearer", expires_in: 900,
                        user: .init(id: "synthetic-user", display_name: "测试", created_at: "2026-09-08")),
          expiresAt: Date().addingTimeInterval(seconds))
  }

  func testSavesOwnerOnlyAndReadsBack() throws {
    let store = BackendDesktopSessionFile(directory: directory)
    XCTAssertNil(try store.load())
    try store.save(session("a"))
    let path = directory.appendingPathComponent(BackendDesktopSessionFile.fileName).path
    let mode = try XCTUnwrap(FileManager.default.attributesOfItem(atPath: path)[.posixPermissions] as? Int)
    XCTAssertEqual(mode & 0o777, 0o600)
    XCTAssertEqual(try store.load()?.tokens.access_token, String(repeating: "a", count: 64))
    // Only the session file is left behind; the temporary it was published through is gone.
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path), [BackendDesktopSessionFile.fileName])
    try store.clear()
    XCTAssertNil(try store.load())
    try store.clear()
  }

  /// The document exactly as the settings app writes it (`FileAccountSessionStorage` with the Apple layout), including the user fields this side does not model.
  func testReadsTheSessionTheSettingsAppWrites() throws {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    let expiresAt = Date().addingTimeInterval(600).timeIntervalSinceReferenceDate
    let document = """
    {"tokens":{"access_token":"\(String(repeating: "c", count: 64))","refresh_token":"\(String(repeating: "d", count: 64))",\
    "token_type":"Bearer","expires_in":900,"user":{"id":"synthetic-user","display_name":"测试",\
    "created_at":"2026-09-08","email":"synthetic@example.test"}},"expiresAt":\(expiresAt)}
    """
    let path = directory.appendingPathComponent(BackendDesktopSessionFile.fileName).path
    XCTAssertTrue(FileManager.default.createFile(atPath: path, contents: Data(document.utf8), attributes: [.posixPermissions: 0o600]))
    let loaded = try XCTUnwrap(BackendDesktopSessionFile(directory: directory).load())
    XCTAssertEqual(loaded.tokens.refresh_token, String(repeating: "d", count: 64))
    XCTAssertEqual(loaded.expiresAt.timeIntervalSinceReferenceDate, expiresAt, accuracy: 0.001)
  }

  func testRefusesAFileOthersCanRead() throws {
    let store = BackendDesktopSessionFile(directory: directory)
    try store.save(session("a"))
    let path = directory.appendingPathComponent(BackendDesktopSessionFile.fileName).path
    try FileManager.default.setAttributes([.posixPermissions: 0o644], ofItemAtPath: path)
    XCTAssertThrowsError(try store.load())
  }

  func testRefusesASymlinkedSessionFile() throws {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    let elsewhere = directory.appendingPathComponent("elsewhere.json")
    try BackendDesktopSessionFile(directory: directory).save(session("a"))
    try FileManager.default.moveItem(at: directory.appendingPathComponent(BackendDesktopSessionFile.fileName), to: elsewhere)
    try FileManager.default.createSymbolicLink(at: directory.appendingPathComponent(BackendDesktopSessionFile.fileName),
                                               withDestinationURL: elsewhere)
    XCTAssertThrowsError(try BackendDesktopSessionFile(directory: directory).load())
  }

  /// Without a storage the macOS account session is the shared file, refreshed under the lock beside it; a session given its own storage keeps a lock of its own.
  func testTheDefaultSessionSharesTheFileAndItsLock() async throws {
    let lock = BackendDesktopSessionFile.refreshLock
    XCTAssertTrue(lock.sharedAcrossProcesses)
    XCTAssertEqual(lock.url?.deletingLastPathComponent(), BackendDesktopSessionFile.standardDirectory)
  }
}
#endif

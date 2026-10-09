#if os(macOS)
import Foundation
import XCTest
@testable import MSIMEBackend

private struct FirstLoginAPI: BackendSessionAPI {
  let tokens: BackendAccountClient.Tokens

  func login(challenge: String, credential: String, linkToken: String?) async throws -> BackendAccountClient.Tokens {
    tokens
  }
  func refresh(_ token: String) async throws -> BackendAccountClient.Tokens {
    throw BackendAccountClient.Failure(status: 0)
  }
  func logout(token: String, all: Bool) async throws { }
}

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

  func testFirstSignInCreatesTheSharedLockDirectory() async throws {
    let storage = BackendDesktopSessionFile(directory: directory)
    let lock = BackendFileRefreshLock(url: directory.appendingPathComponent("account-refresh.lock"))
    let account = BackendAccountSession(api: FirstLoginAPI(tokens: session("a").tokens),
                                        storage: storage, refreshLock: lock)
    XCTAssertFalse(FileManager.default.fileExists(atPath: directory.path))

    try await account.signIn(challenge: "synthetic-challenge", credential: "synthetic-credential")

    XCTAssertEqual(try storage.load()?.tokens.access_token, String(repeating: "a", count: 64))
    let mode = try XCTUnwrap(FileManager.default.attributesOfItem(atPath: directory.path)[.posixPermissions] as? Int)
    XCTAssertEqual(mode & 0o777, 0o700)
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

  func testRefusesASymlinkedSessionDirectory() throws {
    let outside = FileManager.default.temporaryDirectory
      .appendingPathComponent("desktop-session-outside-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true,
                                             attributes: [.posixPermissions: 0o700])
    try FileManager.default.createSymbolicLink(at: directory, withDestinationURL: outside)
    XCTAssertThrowsError(try BackendDesktopSessionFile(directory: directory).save(session("a")))
    XCTAssertFalse(FileManager.default.fileExists(
      atPath: outside.appendingPathComponent(BackendDesktopSessionFile.fileName).path))
    try? FileManager.default.removeItem(at: outside)
  }

  func testClearRefusesASymlinkedSessionDirectory() throws {
    let outside = FileManager.default.temporaryDirectory
      .appendingPathComponent("desktop-session-clear-outside-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true,
                                             attributes: [.posixPermissions: 0o700])
    let outsideSession = outside.appendingPathComponent(BackendDesktopSessionFile.fileName)
    try Data("synthetic-session".utf8).write(to: outsideSession)
    try FileManager.default.createSymbolicLink(at: directory, withDestinationURL: outside)
    defer { try? FileManager.default.removeItem(at: outside) }

    XCTAssertThrowsError(try BackendDesktopSessionFile(directory: directory).clear())
    XCTAssertTrue(FileManager.default.fileExists(atPath: outsideSession.path))
  }

  func testBoundedReaderRejectsAnOversizedSessionDocument() throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("desktop-session-limit-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let file = directory.appendingPathComponent("session.json")
    try Data(repeating: 0x41, count: BackendDesktopSessionFile.maximumBytes + 1).write(to: file)
    XCTAssertThrowsError(try BackendDesktopSessionFile.readBounded(file, maximumBytes: BackendDesktopSessionFile.maximumBytes))
  }

  func testBoundedReaderRejectsASymlink() throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("desktop-session-link-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let target = directory.appendingPathComponent("target.json")
    let link = directory.appendingPathComponent("session.json")
    try Data("synthetic".utf8).write(to: target)
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: target)
    XCTAssertThrowsError(try BackendDesktopSessionFile.readBounded(link, maximumBytes: 64))
  }

  /// Without a storage the macOS account session is the shared file, refreshed under the lock beside it; a session given its own storage keeps a lock of its own.
  func testTheDefaultSessionSharesTheFileAndItsLock() async throws {
    let lock = BackendDesktopSessionFile.refreshLock
    XCTAssertTrue(lock.sharedAcrossProcesses)
    XCTAssertEqual(lock.url?.deletingLastPathComponent(), BackendDesktopSessionFile.standardDirectory)
  }
}
#endif

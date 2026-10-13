import XCTest

final class CloudSyncStateTests: XCTestCase {
  private var root: URL!
  private var state: CloudSyncState!

  override func setUpWithError() throws {
    root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-sync-\(UUID().uuidString)")
    state = CloudSyncState(directory: root)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: root)
  }

  func testStatePersistsAcrossInstances() throws {
    let session = UUID()
    let bound = try state.bind(accountID: "user-1", sessionID: session)
    try state.setEnabled(true)
    try state.updateIfCurrent(bound.bindingGeneration) { record in
      record.cursors[CloudSyncSection.settings.rawValue] = "12"
      record.unheldPhrases = ["很长的一条"]
      record.lastSynced = Date(timeIntervalSince1970: 1_700_000_000)
      record.error = "网络不可用，稍后自动重试"
    }

    let reopened = try CloudSyncState(directory: root).read()
    XCTAssertTrue(reopened.enabled)
    XCTAssertEqual(reopened.accountID, "user-1")
    XCTAssertEqual(reopened.sessionID, session)
    XCTAssertEqual(reopened.cursor(.settings), "12")
    XCTAssertEqual(reopened.cursor(.phrases), "")
    XCTAssertEqual(reopened.unheldPhrases, ["很长的一条"])
    XCTAssertEqual(reopened.lastSynced, Date(timeIntervalSince1970: 1_700_000_000))
    XCTAssertEqual(reopened.error, "网络不可用，稍后自动重试")
  }

  func testEnablingNeedsAnAccountAndMarksEverySectionDirty() throws {
    XCTAssertThrowsError(try state.setEnabled(true))
    XCTAssertFalse(try state.read().enabled)

    try state.bind(accountID: "user-1", sessionID: UUID())
    try state.setEnabled(true)
    let record = try state.read()
    XCTAssertTrue(record.enabled)
    XCTAssertTrue(CloudSyncSection.allCases.allSatisfy(record.dirty))
  }

  func testMarksAreRecordedOnlyWhileEnabledAndNotSuppressed() throws {
    try state.bind(accountID: "user-1", sessionID: UUID())
    state.markDirty(.settings)
    XCTAssertEqual(try state.read().generation(.settings), 0)

    try state.setEnabled(true)
    let enabled = try state.read().generation(.settings)
    state.markDirty(.settings)
    XCTAssertEqual(try state.read().generation(.settings), enabled + 1)

    CloudSyncState.suppressingMarks { state.markDirty(.settings) }
    XCTAssertEqual(try state.read().generation(.settings), enabled + 1)
    state.markDirty(.settings)
    XCTAssertEqual(try state.read().generation(.settings), enabled + 2)
  }

  func testDirtyMarkIsClearedOnlyAtTheGenerationThatWasUploaded() throws {
    let bound = try state.bind(accountID: "user-1", sessionID: UUID())
    try state.setEnabled(true)
    let uploaded = try state.read().generation(.phrases)
    // The user edits again while the upload is in flight.
    state.markDirty(.phrases)
    XCTAssertFalse(try state.clearDirtyIfCurrent(.phrases, expected: uploaded, bindingGeneration: bound.bindingGeneration))
    XCTAssertTrue(try state.read().dirty(.phrases))

    let latest = try state.read().generation(.phrases)
    XCTAssertTrue(try state.clearDirtyIfCurrent(.phrases, expected: latest, bindingGeneration: bound.bindingGeneration))
    XCTAssertFalse(try state.read().dirty(.phrases))
    XCTAssertTrue(try state.read().dirty(.settings))
  }

  func testANewLoginClearsProgressAndFencesTheOldRun() throws {
    let first = try state.bind(accountID: "user-1", sessionID: UUID())
    try state.setEnabled(true)
    try state.updateIfCurrent(first.bindingGeneration) { $0.cursors[CloudSyncSection.phrases.rawValue] = "3" }

    // The same login reconciled again changes nothing.
    let same = try state.bind(accountID: "user-1", sessionID: try state.read().sessionID)
    XCTAssertEqual(same.bindingGeneration, first.bindingGeneration)
    XCTAssertTrue(same.enabled)

    // Signing in again, even to the same account, starts over with sync off.
    let again = try state.bind(accountID: "user-1", sessionID: UUID())
    XCTAssertEqual(again.bindingGeneration, first.bindingGeneration + 1)
    XCTAssertFalse(again.enabled)
    XCTAssertEqual(again.cursor(.phrases), "")
    XCTAssertFalse(try state.updateIfCurrent(first.bindingGeneration) { $0.error = "旧账号的结果" })
    XCTAssertNil(try state.read().error)
    XCTAssertFalse(try state.clearDirtyIfCurrent(.phrases, expected: 0, bindingGeneration: first.bindingGeneration))

    // Signing out forgets the account.
    let signedOut = try state.bind(accountID: nil, sessionID: nil)
    XCTAssertEqual(signedOut.accountID, "")
    XCTAssertNil(signedOut.sessionID)
    XCTAssertFalse(signedOut.bound)
    XCTAssertEqual(signedOut.bindingGeneration, first.bindingGeneration + 2)
  }

  func testAnUnreadableFileReadsAsAFreshState() throws {
    let directory = root.appendingPathComponent("CloudSync", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try Data("{not json".utf8).write(to: directory.appendingPathComponent("state.json"))
    XCTAssertEqual(try state.read(), CloudSyncRecord())
  }

  func testAnOversizedFileReadsAsAFreshState() throws {
    let directory = root.appendingPathComponent("CloudSync", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let oversized = Data(repeating: 0x20, count: 256 * 1024 + 1)
    try oversized.write(to: directory.appendingPathComponent("state.json"))

    XCTAssertEqual(try state.read(), CloudSyncRecord())
    XCTAssertEqual(try Data(contentsOf: directory.appendingPathComponent("state.json")), oversized)
  }
}

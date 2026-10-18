import XCTest

/// 云同步状态交给 Rust（#8201 迁移计划 4d）：键盘扩展和 App 都只经 `CloudSyncState` 记改动、看开关，对着真的 `msime_client_cloud_sync` 在临时目录里跑。
final class CloudSyncStateTests: XCTestCase {
  private var root: URL!
  private var state: CloudSyncState!

  override func setUpWithError() throws {
    root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-sync-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    state = CloudSyncState(directory: root)
  }

  override func tearDownWithError() throws { try? FileManager.default.removeItem(at: root) }

  /// iOS 第 1 版的状态文件：开着同步、绑着一个账号，改动都已经上传。
  private func writeLegacy(enabled: Bool = true, generations: [String: Int] = ["settings": 2, "phrases": 1],
                           cleans: [String: Int] = ["settings": 2, "phrases": 1]) throws {
    let directory = root.appendingPathComponent("CloudSync", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let record: [String: Any] = ["version": 1, "enabled": enabled, "accountID": "legacy-user",
                                 "sessionID": UUID().uuidString, "bindingGeneration": 3,
                                 "cursors": ["settings": "12", "phrases": "4"], "generations": generations,
                                 "cleans": cleans, "unheldPhrases": [], "lastSynced": "2026-10-01T00:00:00Z"]
    try JSONSerialization.data(withJSONObject: record).write(to: directory.appendingPathComponent("state.json"))
  }

  func testAFreshStateIsOffAndMarksNothing() throws {
    let fresh = try state.peek()
    XCTAssertFalse(fresh.enabled)
    XCTAssertFalse(fresh.statusLine.isEmpty)
    state.markDirty(.settings)
    XCTAssertEqual(try state.peek().dirty, [])
  }

  func testTheFirstVersionsStateIsImportedOnceAndKeepsTheSwitchAndAccount() throws {
    try writeLegacy()
    XCTAssertTrue(state.importLegacy())
    let imported = try state.peek()
    XCTAssertTrue(imported.enabled)
    XCTAssertEqual(imported.accountID, "legacy-user")
    XCTAssertEqual(imported.dirty, [], "旧状态里已经上传完的改动不再算待上传")
    // 每次启动都会再调一次：导入过就不再导入。
    XCTAssertFalse(state.importLegacy())
    XCTAssertTrue(FileManager.default.fileExists(atPath: root.appendingPathComponent("cloud-sync/state.json").path))
  }

  func testAPendingLegacyChangeStaysPending() throws {
    try writeLegacy(generations: ["settings": 3, "phrases": 1])
    XCTAssertTrue(state.importLegacy())
    XCTAssertEqual(try state.peek().dirty, [.settings])
  }

  func testWithoutALegacyFileNothingIsImported() throws {
    XCTAssertFalse(state.importLegacy())
    try FileManager.default.createDirectory(at: root.appendingPathComponent("CloudSync"), withIntermediateDirectories: true)
    try Data("不是 JSON".utf8).write(to: root.appendingPathComponent("CloudSync/state.json"))
    XCTAssertFalse(state.importLegacy())
  }

  func testMarksAreRecordedOnlyWhileEnabledAndNotSuppressed() throws {
    try writeLegacy()
    XCTAssertTrue(state.importLegacy())
    CloudSyncState.suppressingMarks { state.markDirty(.phrases) }
    XCTAssertEqual(try state.peek().dirty, [])
    state.markDirty(.phrases)
    XCTAssertEqual(try state.peek().dirty, [.phrases])

    let off = CloudSyncState(directory: root.appendingPathComponent("off", isDirectory: true))
    off.markDirty(.settings)
    XCTAssertEqual(try off.peek().dirty, [])
  }

  func testWithoutAContainerEverythingIsANoOp() {
    let missing = CloudSyncState(directory: nil)
    missing.markDirty(.settings)
    XCTAssertFalse(missing.importLegacy())
    XCTAssertThrowsError(try missing.peek())
  }
}

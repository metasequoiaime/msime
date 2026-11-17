import XCTest

/// 设置 App 先查账号绑定；明确未登录时仍对键盘的真实词库执行本机重置，保留维护忙与自造词清除回归。
final class ClearLearningTests: XCTestCase {
  @MainActor
  func testTheResetWaitsForTheKeyboardAndThenClearsTheUsersWords() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let stateRoot = root.appendingPathComponent("EngineState")
    FakeRust.replies(["account.learning_binding": FakeRust.ok(["login": NSNull()])], signedIn: nil)
    let session = MetasequoiaInputSessionBridge(resources: resources, stateRoot: stateRoot)
    let word = try PersonalWord(kind: .quickPhrase, key: "msimecleared", value: "cleared fixture").validated()
    try session.applyPersonalPrevious(nil, replacement: word.bridgeValue, requestID: UUID().uuidString)

    XCTAssertThrowsError(try IOSAccountLearningReset.reset(resources: resources, stateRoot: stateRoot, account: FakeRust.account)) {
      XCTAssertEqual($0 as? MetasequoiaInputSessionBridge.LearnedDataResetFailure, .busy)
    }
    let options = try XCTUnwrap(JSONSerialization.jsonObject(
      with: MetasequoiaInputSessionBridge.preparedHostOptions(resources: resources, stateRoot: stateRoot)) as? [String: Any])
    let before = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertTrue(before.entries.contains(word))

    // 键盘收起时销毁会话、放掉词库。
    XCTAssertTrue(session.suspendDictionarySession())
    XCTAssertThrowsError(try IOSAccountLearningReset.reset(resources: resources, stateRoot: stateRoot, account: nil)) {
      XCTAssertEqual($0 as? MetasequoiaInputSessionBridge.LearnedDataResetFailure, .failed)
    }
    let unchanged = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertTrue(unchanged.entries.contains(word), "读不了账号时不能执行无屏障清除")
    XCTAssertNoThrow(try IOSAccountLearningReset.reset(resources: resources, stateRoot: stateRoot, account: FakeRust.account))
    let after = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertFalse(after.entries.contains(word))
  }

  func testSignedInResetPreservesTheBusyResultFromTheAccountBarrier() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    FakeRust.replies([
      "account.learning_binding": FakeRust.ok(["login": ["user_id": "synthetic-account", "session_id": "00000000-0000-4000-8000-000000000001"]]),
      "account.learning_reset_local": FakeRust.failure("learning_busy")
    ])
    XCTAssertThrowsError(try IOSAccountLearningReset.reset(resources: resources, stateRoot: root, account: FakeRust.account)) {
      XCTAssertEqual($0 as? MetasequoiaInputSessionBridge.LearnedDataResetFailure, .busy)
    }
    XCTAssertEqual(FakeRust.calls.map(\.operation), ["learning_binding", "learning_reset_local"])
  }

  func testSignedInResetKeepsTheCollectionRequeueResult() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    for collectionsRequeued in [true, false] {
      var result: [String: Any] = ["reset": true]
      if !collectionsRequeued { result["collections_error"] = "synthetic-requeue-failure" }
      FakeRust.replies([
        "account.learning_binding": FakeRust.ok(["login": ["user_id": "synthetic-account", "session_id": "00000000-0000-4000-8000-000000000001"]]),
        "account.learning_reset_local": FakeRust.ok(result)
      ])
      XCTAssertEqual(try IOSAccountLearningReset.reset(resources: resources, stateRoot: root, account: FakeRust.account), collectionsRequeued)
      XCTAssertEqual(FakeRust.calls.map(\.operation), ["learning_binding", "learning_reset_local"])
    }
  }

  @MainActor
  func testResetKeepsEnabledGroupsAndRequeuesTheirWords() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let stateRoot = root.appendingPathComponent("EngineState")
    FakeRust.replies(["account.learning_binding": FakeRust.ok(["login": NSNull()])], signedIn: nil)
    let session = MetasequoiaInputSessionBridge(resources: resources, stateRoot: stateRoot)
    XCTAssertTrue(session.suspendDictionarySession())
    let optionsData = try MetasequoiaInputSessionBridge.preparedHostOptions(resources: resources, stateRoot: stateRoot)
    let options = try XCTUnwrap(JSONSerialization.jsonObject(with: optionsData) as? [String: Any])
    _ = try DictionaryCollectionsBridge.request(["operation": "import", "name": "合成分组",
      "kind": "quickPhrase", "format": "standard", "text": "合成分组词\tmsimegroup\t100\n"], options: optionsData)
    _ = try DictionaryCollectionsBridge.synchronize(options: optionsData, budget: 2000)
    let before = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertTrue(before.entries.contains { $0.key == "msimegroup" && $0.value == "合成分组词" })
    XCTAssertTrue(try IOSAccountLearningReset.reset(resources: resources, stateRoot: stateRoot, account: FakeRust.account))
    let groups = try DictionaryCollectionsBridge.request(["operation": "load"], options: optionsData)
    let items = try XCTUnwrap(groups["collections"] as? [[String: Any]])
    XCTAssertEqual(items.first?["name"] as? String, "合成分组")
    XCTAssertEqual(items.first?["enabled"] as? Bool, true)
    let directory = try XCTUnwrap(options["preferences_directory"] as? String)
    let queued = try DictionaryCollectionsBridge.personalQueue(preferencesDirectory: directory).read().requests
    XCTAssertTrue(try DictionaryCollectionsBridge.needsSynchronization(options: optionsData))
    XCTAssertTrue(queued.contains { $0.status == .pending && $0.replacement?.key == "msimegroup" })
    let cleared = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertFalse(cleared.entries.contains { $0.key == "msimegroup" && $0.value == "合成分组词" })
    _ = try DictionaryCollectionsBridge.synchronize(options: optionsData, budget: 2000)
    let page = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertTrue(page.entries.contains { $0.key == "msimegroup" && $0.value == "合成分组词" })
  }

  /// 生产环境的 preferences_directory 是 `<AppGroup>/MSIME`，键盘判断要不要同步分组时读的队列必须就在它下面，而不是 `PersonalDictionaryStore()` 默认的 App Group 根目录。
  func testCollectionQueueLivesUnderThePreferencesDirectory() throws {
    let directory = MetasequoiaInputSessionBridge.sharedStateDirectory
    try XCTSkipIf(directory.isEmpty, "App Group unavailable")
    let file = try XCTUnwrap(DictionaryCollectionsBridge.personalQueue(preferencesDirectory: directory).exportFile)
    XCTAssertEqual(file.deletingLastPathComponent().standardizedFileURL.path,
                   URL(fileURLWithPath: directory, isDirectory: true).appendingPathComponent("PersonalDictionary").standardizedFileURL.path)
  }

  func testCopyNamesWhatGoesAndWhatStays() {
    for removed in ["候选调频", "置顶和固定位置", "选词记录", "整句联想", "拼写纠错记录", "自造词", "含快捷短语", "无法撤销", "未分组的导入和社区词条"] {
      XCTAssertTrue(ClearLearningCopy.confirmMessage.contains(removed), removed)
    }
    for kept in ["设置", "常用语", "剪贴板", "打字统计", "词库分组保留"] {
      XCTAssertTrue(ClearLearningCopy.confirmMessage.contains(kept), kept)
    }
    // iOS 的账号同步不含个人词库，确认框不提账号；「我的词条」网盘同步会把删除带到别的设备，要说。
    XCTAssertFalse(ClearLearningCopy.confirmMessage.contains("账号"))
    XCTAssertTrue(ClearLearningCopy.confirmMessage.hasSuffix(ClearLearningCopy.webdav))
    XCTAssertTrue(ClearLearningCopy.confirmMessage.hasPrefix(ClearLearningCopy.removed))
  }
}

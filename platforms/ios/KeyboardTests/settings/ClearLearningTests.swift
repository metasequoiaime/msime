import XCTest

/// 设置 App「词库 › 学习」的「清除学习记录」：App 经 `msime_client_prepare_host` 拿到键盘那套词库路径，发 `reset`。键盘会话开着时是忙，收起（销毁会话）以后清掉自造词。
final class ClearLearningTests: XCTestCase {
  @MainActor
  func testTheResetWaitsForTheKeyboardAndThenClearsTheUsersWords() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let stateRoot = root.appendingPathComponent("EngineState")
    let session = MetasequoiaInputSessionBridge(resources: resources, stateRoot: stateRoot)
    let word = try PersonalWord(kind: .quickPhrase, key: "msimecleared", value: "cleared fixture").validated()
    try session.applyPersonalPrevious(nil, replacement: word.bridgeValue, requestID: UUID().uuidString)

    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.resetLearnedData(resources: resources, stateRoot: stateRoot)) {
      XCTAssertEqual($0 as? MetasequoiaInputSessionBridge.LearnedDataResetFailure, .busy)
    }
    let options = try XCTUnwrap(JSONSerialization.jsonObject(
      with: MetasequoiaInputSessionBridge.preparedHostOptions(resources: resources, stateRoot: stateRoot)) as? [String: Any])
    let before = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertTrue(before.entries.contains(word))

    // 键盘收起时销毁会话、放掉词库。
    XCTAssertTrue(session.suspendDictionarySession())
    XCTAssertNoThrow(try MetasequoiaInputSessionBridge.resetLearnedData(resources: resources, stateRoot: stateRoot))
    let after = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertFalse(after.entries.contains(word))
  }

  @MainActor
  func testResetKeepsEnabledGroupsAndRequeuesTheirWords() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let stateRoot = root.appendingPathComponent("EngineState")
    let session = MetasequoiaInputSessionBridge(resources: resources, stateRoot: stateRoot)
    XCTAssertTrue(session.suspendDictionarySession())
    let optionsData = try MetasequoiaInputSessionBridge.preparedHostOptions(resources: resources, stateRoot: stateRoot)
    let options = try XCTUnwrap(JSONSerialization.jsonObject(with: optionsData) as? [String: Any])
    _ = try DictionaryCollectionsBridge.request(["operation": "import", "name": "合成分组",
      "kind": "quickPhrase", "format": "standard", "text": "合成分组词\tmsimegroup\t100\n"], options: optionsData)
    _ = try DictionaryCollectionsBridge.synchronize(options: optionsData, budget: 2000)
    let before = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertTrue(before.entries.contains { $0.key == "msimegroup" && $0.value == "合成分组词" })
    XCTAssertTrue(try MetasequoiaInputSessionBridge.resetLearnedData(resources: resources, stateRoot: stateRoot))
    let groups = try DictionaryCollectionsBridge.request(["operation": "load"], options: optionsData)
    let items = try XCTUnwrap(groups["collections"] as? [[String: Any]])
    XCTAssertEqual(items.first?["name"] as? String, "合成分组")
    XCTAssertEqual(items.first?["enabled"] as? Bool, true)
    let queued = try PersonalDictionaryStore(directory: stateRoot).read().requests
    XCTAssertTrue(queued.contains { $0.status == .pending && $0.replacement?.key == "msimegroup" })
    let cleared = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertFalse(cleared.entries.contains { $0.key == "msimegroup" && $0.value == "合成分组词" })
    _ = try DictionaryCollectionsBridge.synchronize(options: optionsData, budget: 2000)
    let page = try PersonalWordPage(bridgeResult: MetasequoiaInputSessionBridge.personalEntries(options: options, offset: 0))
    XCTAssertTrue(page.entries.contains { $0.key == "msimegroup" && $0.value == "合成分组词" })
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

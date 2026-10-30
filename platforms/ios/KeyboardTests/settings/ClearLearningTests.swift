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

  func testCopyNamesWhatGoesAndWhatStays() {
    for removed in ["候选调频", "置顶和固定位置", "选词记录", "整句联想", "拼写纠错记录", "自造词", "含快捷短语", "无法撤销", "导入和从社区添加的词条"] {
      XCTAssertTrue(ClearLearningCopy.confirmMessage.contains(removed), removed)
    }
    for kept in ["设置", "常用语", "剪贴板", "打字统计"] {
      XCTAssertTrue(ClearLearningCopy.confirmMessage.contains(kept), kept)
    }
    // iOS 的账号同步不含个人词库，确认框不提账号；「我的词条」网盘同步会把删除带到别的设备，要说。
    XCTAssertFalse(ClearLearningCopy.confirmMessage.contains("账号"))
    XCTAssertTrue(ClearLearningCopy.confirmMessage.hasSuffix(ClearLearningCopy.webdav))
    XCTAssertTrue(ClearLearningCopy.confirmMessage.hasPrefix(ClearLearningCopy.removed))
  }
}

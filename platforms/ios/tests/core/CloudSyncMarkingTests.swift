import XCTest

/// 记下一次本机改动、下一轮云同步上传的那些写入方。`CloudSyncState.shared` 在这里换成临时目录上的一份，先导入一份开着同步的旧状态（Rust 只在开着时记改动），用例结束换回去，不碰真正的同步进度。
final class CloudSyncMarkingTests: XCTestCase {
  private var root: URL!
  private var original: CloudSyncState!

  override func setUpWithError() throws {
    original = CloudSyncState.shared
    root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-sync-marking-\(UUID().uuidString)", isDirectory: true)
    try freshState()
  }

  override func tearDownWithError() throws {
    if let original { CloudSyncState.shared = original }
    if let root { try? FileManager.default.removeItem(at: root) }
  }

  /// 一份新的、开着同步、没有待上传改动的状态。
  private func freshState() throws {
    try? FileManager.default.removeItem(at: root)
    let legacy = root.appendingPathComponent("CloudSync", isDirectory: true)
    try FileManager.default.createDirectory(at: legacy, withIntermediateDirectories: true)
    let record: [String: Any] = ["version": 1, "enabled": true, "accountID": "cloud-sync-marking-test",
                                 "cursors": ["settings": "1", "phrases": "1"], "generations": [:], "cleans": [:]]
    try JSONSerialization.data(withJSONObject: record).write(to: legacy.appendingPathComponent("state.json"))
    CloudSyncState.shared = CloudSyncState(directory: root)
    XCTAssertTrue(CloudSyncState.shared.importLegacy())
    XCTAssertEqual(try CloudSyncState.shared.peek().dirty, [])
  }

  private func dirty(_ section: CloudSyncSection) throws -> Bool {
    try CloudSyncState.shared.peek().dirty.contains(section)
  }

  func testSharedDocumentWritesMarkSettingsUnlessNothingChangedOrSyncWrote() throws {
    let previous = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    defer { CloudSyncState.suppressingMarks { _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = previous } } }
    let traditional = previous[ChineseOutputPreference.documentKey] as? Bool ?? false

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[ChineseOutputPreference.documentKey] = !traditional })
    XCTAssertTrue(try dirty(.settings))

    // Writing the value it already has saves nothing and marks nothing.
    try freshState()
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[ChineseOutputPreference.documentKey] = !traditional })
    XCTAssertFalse(try dirty(.settings))

    // Cloud sync applying the cloud's settings is not a local change.
    CloudSyncState.suppressingMarks {
      XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[ChineseOutputPreference.documentKey] = traditional })
    }
    XCTAssertFalse(try dirty(.settings))

    // A test's own directory is not the user's settings.
    let other = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-sync-marking-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: other) }
    _ = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: other) { $0[ChineseOutputPreference.documentKey] = !traditional }
    XCTAssertFalse(try dirty(.settings))
  }

  func testKeyboardFeedbackChangesMarkSettingsOnlyWhenTheValueChanges() throws {
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.object(forKey: KeyboardFeedbackPreference.soundKey)
    defer { defaults.set(previous, forKey: KeyboardFeedbackPreference.soundKey) }
    let sound = KeyboardFeedbackPreference.soundEnabled

    KeyboardFeedbackPreference.set(!sound, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertEqual(KeyboardFeedbackPreference.soundEnabled, !sound)
    XCTAssertTrue(try dirty(.settings))
    try freshState()
    KeyboardFeedbackPreference.set(!sound, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertFalse(try dirty(.settings))
  }

  func testSavingAKeyboardDesignMarksSettingsButCopyingTheDocumentDoesNot() throws {
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.data(forKey: CustomKeyboardSkinStore.key)
    let previousTheme = defaults.object(forKey: GlobalThemePreference.key)
    defer {
      defaults.set(previous, forKey: CustomKeyboardSkinStore.key)
      defaults.set(previousTheme, forKey: GlobalThemePreference.key)
    }
    var design = CustomKeyboardSkinStore.current
    design.background = design.background == 0x123456 ? 0x654321 : 0x123456

    CustomKeyboardSkinStore.save(design)
    XCTAssertTrue(try dirty(.settings))
    try freshState()
    CustomKeyboardSkinStore.save(design)
    XCTAssertFalse(try dirty(.settings))

    design.accent = design.accent == 0x112233 ? 0x332211 : 0x112233
    let document = try XCTUnwrap(GlobalThemePreference.applyingDesign(design).map { mapping -> [String: Any] in
      var document: [String: Any] = [:]
      mapping(&document)
      return document
    })
    XCTAssertTrue(GlobalThemePreference.mirror(document))
    XCTAssertEqual(CustomKeyboardSkinStore.current, design.normalized)
    XCTAssertFalse(try dirty(.settings))
  }

  func testCommonPhraseEditsMarkPhrasesInTheSharedStoreOnly() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-sync-phrases-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let added = try CommonPhrasesBridge.add("测试目录里的常用语", directory: directory.path)
    _ = try CommonPhrasesBridge.remove(id: try XCTUnwrap(added.first).id, directory: directory.path)
    XCTAssertFalse(try dirty(.phrases))

    let text = "云同步标记测试 \(UUID().uuidString)"
    let stored = try CommonPhrasesBridge.add(text)
    let id = try XCTUnwrap(stored.first { $0.text == text }).id
    XCTAssertTrue(try dirty(.phrases))
    _ = try CommonPhrasesBridge.remove(id: id)
    XCTAssertTrue(try dirty(.phrases))
  }
}

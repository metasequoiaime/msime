import XCTest

/// The writers that mark a section for the next cloud sync. They mark the App Group state the keyboard and the app share, so each test saves that file and puts it back, along with every setting it touches.
final class CloudSyncMarkingTests: XCTestCase {
  private var savedState: Data?
  private var stateFile: URL!

  override func setUpWithError() throws {
    stateFile = CloudSyncState.defaultRoot.appendingPathComponent("CloudSync/state.json")
    savedState = try? Data(contentsOf: stateFile)
    let state = CloudSyncState.shared
    try state.bind(accountID: "cloud-sync-marking-test", sessionID: UUID())
    try state.setEnabled(true)
  }

  override func tearDownWithError() throws {
    // A setUp that failed before assigning stateFile must not crash the whole test bundle here.
    guard let stateFile else { return }
    if let savedState { try savedState.write(to: stateFile) } else { try? FileManager.default.removeItem(at: stateFile) }
  }

  private func generation(_ section: CloudSyncSection) throws -> Int64 {
    try CloudSyncState.shared.read().generation(section)
  }

  func testSharedDocumentWritesMarkSettingsUnlessNothingChangedOrSyncWrote() throws {
    let previous = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    defer { CloudSyncState.suppressingMarks { _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = previous } } }
    let traditional = previous[ChineseOutputPreference.documentKey] as? Bool ?? false
    let before = try generation(.settings)

    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[ChineseOutputPreference.documentKey] = !traditional })
    XCTAssertEqual(try generation(.settings), before + 1)

    // Writing the value it already has saves nothing and marks nothing.
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[ChineseOutputPreference.documentKey] = !traditional })
    XCTAssertEqual(try generation(.settings), before + 1)

    // Cloud sync applying the cloud's settings is not a local change.
    CloudSyncState.suppressingMarks {
      XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[ChineseOutputPreference.documentKey] = traditional })
    }
    XCTAssertEqual(try generation(.settings), before + 1)

    // A test's own directory is not the user's settings.
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-sync-marking-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: root) }
    _ = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: root) { $0[ChineseOutputPreference.documentKey] = !traditional }
    XCTAssertEqual(try generation(.settings), before + 1)
  }

  func testKeyboardFeedbackChangesMarkSettingsOnlyWhenTheValueChanges() throws {
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.object(forKey: KeyboardFeedbackPreference.soundKey)
    defer { defaults.set(previous, forKey: KeyboardFeedbackPreference.soundKey) }
    let sound = KeyboardFeedbackPreference.soundEnabled
    let before = try generation(.settings)

    KeyboardFeedbackPreference.set(!sound, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertEqual(KeyboardFeedbackPreference.soundEnabled, !sound)
    XCTAssertEqual(try generation(.settings), before + 1)
    KeyboardFeedbackPreference.set(!sound, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertEqual(try generation(.settings), before + 1)
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
    let before = try generation(.settings)

    CustomKeyboardSkinStore.save(design)
    XCTAssertEqual(try generation(.settings), before + 1)
    CustomKeyboardSkinStore.save(design)
    XCTAssertEqual(try generation(.settings), before + 1)

    design.accent = design.accent == 0x112233 ? 0x332211 : 0x112233
    let document = try XCTUnwrap(GlobalThemePreference.applyingDesign(design).map { mapping -> [String: Any] in
      var document: [String: Any] = [:]
      mapping(&document)
      return document
    })
    XCTAssertTrue(GlobalThemePreference.mirror(document))
    XCTAssertEqual(CustomKeyboardSkinStore.current, design.normalized)
    XCTAssertEqual(try generation(.settings), before + 1)
  }

  func testCommonPhraseEditsMarkPhrasesInTheSharedStoreOnly() throws {
    let before = try generation(.phrases)
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-cloud-sync-phrases-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let added = try CommonPhrasesBridge.add("测试目录里的常用语", directory: directory.path)
    _ = try CommonPhrasesBridge.remove(id: try XCTUnwrap(added.first).id, directory: directory.path)
    XCTAssertEqual(try generation(.phrases), before)

    let text = "云同步标记测试 \(UUID().uuidString)"
    let stored = try CommonPhrasesBridge.add(text)
    let id = try XCTUnwrap(stored.first { $0.text == text }).id
    XCTAssertEqual(try generation(.phrases), before + 1)
    _ = try CommonPhrasesBridge.remove(id: id)
    XCTAssertEqual(try generation(.phrases), before + 2)
  }
}

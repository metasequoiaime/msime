import XCTest

final class KeyboardSkinTrialTests: XCTestCase {
  func testTrialRestoresExactCustomDesignAndMissingSelection() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let suite = "trial-tests-\(UUID())"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite); try? FileManager.default.removeItem(at: directory) }
    let state = directory.appendingPathComponent("state", isDirectory: true)
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let original = try JSONEncoder().encode(CustomKeyboardSkin(background: 0x123456))
    defaults.set(original, forKey: CustomKeyboardSkinStore.key)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { document in
      document["global_theme"] = "paper"
      document["custom_theme"] = ["keyboard": CustomKeyboardSkin.documentValue(CustomKeyboardSkin(background: 0x123456))!]
    })
    let store = try KeyboardSkinTrialStore(directory: directory, defaults: defaults, stateRoot: state)
    let trial = try store.begin(name: "新皮肤", design: CustomKeyboardSkin(background: 0xABCDEF))
    XCTAssertEqual(defaults.string(forKey: GlobalThemePreference.key), "custom")
    // The keyboard shows what the document holds, so the trial and its undo go there too.
    let trying = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(trying["global_theme"] as? String, "custom")
    XCTAssertEqual(GlobalThemePreference.base(in: trying), "paper", "the theme on screen stays underneath the trial")
    XCTAssertEqual(GlobalThemePreference.design(in: trying)?.background, 0xABCDEF)
    try store.finish(trial.id, keep: false)
    let restored = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(restored["global_theme"] as? String, "paper")
    XCTAssertEqual(GlobalThemePreference.design(in: restored)?.background, 0x123456)
    XCTAssertNil(GlobalThemePreference.customTheme(in: restored)["base"])
    XCTAssertNil(defaults.string(forKey: GlobalThemePreference.key))
    XCTAssertEqual(defaults.data(forKey: CustomKeyboardSkinStore.key), original)
    let second = try store.begin(name: "保留", design: trial.design)
    try store.finish(second.id, keep: true)
    try store.restorePending()
    XCTAssertEqual(defaults.string(forKey: GlobalThemePreference.key), "custom")
    XCTAssertEqual(try JSONDecoder().decode(CustomKeyboardSkin.self, from: XCTUnwrap(defaults.data(forKey: CustomKeyboardSkinStore.key))), trial.design)
  }
  func testRestartRecoveryAndLaterExplicitSelection() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let suite = "trial-tests-\(UUID())"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite); try? FileManager.default.removeItem(at: directory) }
    let state = directory.appendingPathComponent("state", isDirectory: true)
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    defaults.set("night", forKey: GlobalThemePreference.key)
    let store = try KeyboardSkinTrialStore(directory: directory, defaults: defaults, stateRoot: state)
    _ = try store.begin(name: "试用", design: CustomKeyboardSkin())
    try KeyboardSkinTrialStore(directory: directory, defaults: defaults, stateRoot: state).restorePending()
    XCTAssertEqual(defaults.string(forKey: GlobalThemePreference.key), "night")
    let next = try store.begin(name: "试用", design: CustomKeyboardSkin())
    defaults.set("ink", forKey: GlobalThemePreference.key)
    try store.finish(next.id, keep: false)
    XCTAssertEqual(defaults.string(forKey: GlobalThemePreference.key), "ink")
  }
}

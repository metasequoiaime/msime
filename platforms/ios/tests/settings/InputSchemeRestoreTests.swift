import XCTest

/// 「重置所有设置」恢复出默认文档后，App Group 里的方案镜像要跟着回到默认入口。
final class InputSchemeRestoreTests: XCTestCase {
  private let keys = [InputSchemePreference.enabledSchemesKey, "chineseInputScheme"]
  private var defaults: UserDefaults { UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard }

  override func setUp() {
    super.setUp()
    // 镜像在模拟器上的各个测试包之间共用，用例结束后放回原样。
    let saved = keys.map { defaults.object(forKey: $0) }
    addTeardownBlock { [keys, defaults] in
      for (key, value) in zip(keys, saved) {
        if let value { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
      }
    }
  }

  func testRestoredDocumentWithoutSelectionResetsMirroredSchemeToFirstEnabled() {
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey, .wubi]
    InputSchemePreference.scheme = .wubi
    XCTAssertEqual(InputSchemePreference.scheme, .wubi)

    InputSchemePreference.mirrorRestoredDefaults(["touch_keyboard_schemes": ["enabled": ["quanpin", "nine_key", "wubi"]]])

    XCTAssertEqual(InputSchemePreference.enabledSchemes, [.quanpin, .nineKey, .wubi])
    XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
  }

  func testRestoredDocumentWithSelectionKeepsIt() {
    InputSchemePreference.enabledSchemes = [.quanpin, .wubi]
    InputSchemePreference.scheme = .wubi

    InputSchemePreference.mirrorRestoredDefaults([
      "touch_keyboard_schemes": ["enabled": ["quanpin", "nine_key", "handwriting"], "selected": "nine_key"],
    ])

    XCTAssertEqual(InputSchemePreference.scheme, .nineKey)
  }

  func testPlainMirrorStillKeepsSelectionTheDocumentDoesNotRecord() {
    InputSchemePreference.enabledSchemes = [.quanpin, .wubi]
    InputSchemePreference.scheme = .wubi

    InputSchemePreference.mirror(["touch_keyboard_schemes": ["enabled": ["quanpin", "wubi"]]])

    XCTAssertEqual(InputSchemePreference.scheme, .wubi)
  }
}

import XCTest

/// Fuzzy pinyin lives in the shared document, which the native page, the shared settings page and the keyboard all read.
@MainActor
final class FuzzyPinyinDocumentTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-fuzzy-document-\(UUID().uuidString)", isDirectory: true)
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private func stored() throws -> FuzzyPinyinPreference.Settings {
    try XCTUnwrap(FuzzyPinyinPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)))
  }

  func testFirstEnableKeepsTheChosenRulesInsteadOfReseedingEveryRule() throws {
    XCTAssertEqual(try stored(), .pristine)

    XCTAssertTrue(FuzzyPinyinPreference.save(.init(enabled: true, rules: ["n-l"], seeded: true), stateRoot: state))

    XCTAssertEqual(try stored(), .init(enabled: true, rules: ["n-l"], seeded: true))
  }

  func testTheDocumentSuppliesTheKeyboardRules() throws {
    XCTAssertTrue(FuzzyPinyinPreference.save(.init(enabled: true, rules: ["s-sh", "an-ang"], seeded: true), stateRoot: state))
    XCTAssertEqual(try stored().bits, 1 << 2 | 1 << 6)
    XCTAssertTrue(FuzzyPinyinPreference.save(.init(enabled: false, rules: ["s-sh"], seeded: true), stateRoot: state))
    XCTAssertEqual(try stored().bits, 0)
  }
}

import XCTest

/// 经真实 C ABI 校验键面、用户表、冷会话和恢复网格，不复制引擎的解码实现。
final class NineKeyShuangpinTests: XCTestCase {
  func testExportImportKeepsEditableErrorsAndRejectsMalformedOrOversizedFiles() throws {
    var table = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: "migala2").table
    XCTAssertEqual(try NineKeyShuangpinLayout.importTable(NineKeyShuangpinLayout.exportTable(table)), table)
    table["finals"]?["ang"] = "x"
    XCTAssertEqual(try NineKeyShuangpinLayout.importTable(NineKeyShuangpinLayout.exportTable(table)), table,
                   "无效单位保留给编辑器修正，不能静默换成米旮旯 3")
    XCTAssertThrowsError(try NineKeyShuangpinLayout.importTable(Data("[]".utf8)))
    XCTAssertThrowsError(try NineKeyShuangpinLayout.importTable(Data(repeating: 32, count: 16_385)))
    table["unknown_part"] = ["synthetic": "1"]
    XCTAssertThrowsError(try NineKeyShuangpinLayout.importTable(NineKeyShuangpinLayout.exportTable(table)))
  }

  func testBuiltinTablesAndInvalidCustomReportTheEffectiveMapping() throws {
    for profile in NineKeyShuangpinLayout.profiles where profile != "custom" {
      let layout = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: profile)
      XCTAssertNil(layout.problem)
      XCTAssertEqual(layout.effective, profile)
      XCTAssertTrue(layout.keys.contains { $0.key == "1" })
      let custom = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: "custom", table: layout.table)
      XCTAssertNil(custom.problem)
      XCTAssertEqual(custom.table, layout.table)
    }
    let source = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: "migala3")
    XCTAssertEqual(source.keys.first { $0.key == "0" }?.finals, ["u", "ü", "ei"])
    var table = source.table
    table["finals"]?["ang"] = "x"
    let invalid = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: "custom", table: table)
    XCTAssertEqual(invalid.effective, "migala3")
    XCTAssertEqual(invalid.problem?.part, "finals")
    XCTAssertEqual(invalid.problem?.unit, "ang")
    XCTAssertEqual(invalid.problem?.key, "x")
    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: "unknown"))
  }

  func testSavingAndRecreatingSessionKeepsCustomGridAndRejectsBrokenTable() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent("msime-nine-sp-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: state) }
    let mirror = (InputSchemePreference.enabledSchemes, InputSchemePreference.scheme)
    defer {
      InputSchemePreference.enabledSchemes = mirror.0
      InputSchemePreference.scheme = mirror.1
    }
    var table = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: "migala3").table
    table["initials"]?["n"] = "4"
    table["initials"]?["h"] = "6"
    XCTAssertTrue(try NineKeyShuangpinLayout.save(profile: "custom", table: table, stateRoot: state))
    var first: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(try XCTUnwrap(first).setTouchKeyboardScheme(.nineKeyShuangpin, enabledSchemes: [.quanpin, .nineKeyShuangpin]))
    var typed = MetasequoiaInputSnapshot()
    for digit in "4467" { typed = try XCTUnwrap(first).handleCharacter(String(digit)) }
    XCTAssertTrue(typed.candidates.contains("你好"), "\(typed.diagnosticText ?? "") / \(typed.candidates)")
    XCTAssertEqual(typed.editingText, "4467")
    _ = first?.cancel()
    first = nil
    let restored = MetasequoiaInputSessionBridge(stateRoot: state)
    for digit in "10" { typed = restored.handleCharacter(String(digit)) }
    XCTAssertEqual(typed.editingText, "10", "0 和 1 都参与组码")
    _ = restored.cancel()
    XCTAssertTrue(restored.suspendDictionarySession())
    try restored.resumeDictionarySession()
    XCTAssertEqual(restored.handleCharacter("1").editingText, "1")

    table["finals"]?["ang"] = "x"
    XCTAssertThrowsError(try NineKeyShuangpinLayout.save(profile: "custom", table: table, stateRoot: state))
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    XCTAssertEqual((document?[NineKeyShuangpinLayout.tableKey] as? NineKeyShuangpinLayout.Table)?["finals"]?["ang"], "5")
    XCTAssertEqual(InputSchemePreference.scheme, .nineKeyShuangpin)
  }
}

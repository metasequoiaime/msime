import XCTest

/// 经真实 C ABI 核对普通 26 键双拼的表、错误位置和键位提示，设置页不复制 Engine 规则。
final class ShuangpinCustomTests: XCTestCase {
  func testBuiltinAndCustomLayoutsShareTheSameTableAndKeyFace() throws {
    let builtin = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "xiaohe")
    XCTAssertNil(builtin.problem)
    XCTAssertEqual(builtin.effective, "xiaohe")
    XCTAssertEqual(builtin.table["finals"]?["ing"], "k")
    XCTAssertEqual(builtin.keys.first?.key, "Q")
    XCTAssertEqual(builtin.keys.first?.hint, "iu")

    let custom = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(
      profile: "custom", table: builtin.table)
    XCTAssertNil(custom.problem)
    XCTAssertEqual(custom.effective, "custom")
    XCTAssertEqual(custom.table, builtin.table)
    XCTAssertEqual(custom.keys.first?.hint, builtin.keys.first?.hint)
  }

  func testInvalidCustomTableReportsTheUnitAndUsesXiaoheForTheFace() throws {
    var table = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "xiaohe").table
    table["finals"]?.removeValue(forKey: "iong")
    let invalid = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(
      profile: "custom", table: table)
    XCTAssertEqual(invalid.effective, "xiaohe")
    XCTAssertEqual(invalid.problem?.kind, "missing_unit")
    XCTAssertEqual(invalid.problem?.part, "finals")
    XCTAssertEqual(invalid.problem?.unit, "iong")
    XCTAssertEqual(invalid.keys.first?.hint, "iu")
    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "unknown"))
  }

  func testTableExportAndImportRoundTrip() throws {
    let table = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "xiaohe").table
    XCTAssertEqual(try ShuangpinCustomLayout.importTable(ShuangpinCustomLayout.exportTable(table)), table)
    XCTAssertThrowsError(try ShuangpinCustomLayout.importTable(Data("[]".utf8)))
    XCTAssertThrowsError(try ShuangpinCustomLayout.importTable(Data(repeating: 32, count: 16_385)))
    var malformed = table
    malformed["unknown"] = ["synthetic": "q"]
    XCTAssertThrowsError(try ShuangpinCustomLayout.importTable(ShuangpinCustomLayout.exportTable(malformed)))
  }
}

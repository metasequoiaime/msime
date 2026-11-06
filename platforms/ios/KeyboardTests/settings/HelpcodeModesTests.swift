import XCTest

/// 设置 App「辅助码」页的双拼触发方式（#10624）：各行与共享页、Windows、Android 一致，缺字段时按 client-core 的出厂值读，写入只改一个字段，互斥由 client-core 在写入端归一后读回。
@MainActor
final class HelpcodeModesTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-helpcode-modes-\(UUID().uuidString)", isDirectory: true)
    XCTAssertNoThrow(try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true))
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private func mode(_ field: String) -> HelpcodeSettingsView.Mode {
    HelpcodeSettingsView.modes.first { $0.field == field }!
  }

  /// 顺序照共享页 modesRows，不列只在 macOS 生效的 manual_tab；标题与共享页逐字相同。
  func testRowsFollowTheSharedPage() {
    XCTAssertEqual(HelpcodeSettingsView.modes.map(\.field), [
      "direct", "trigger_semicolon", "trigger_backtick", "trigger_uppercase",
      "mid_sentence", "mid_sentence_backtick", "mid_sentence_semicolon", "mid_sentence_uppercase",
      "direct_helpcode", "direct_helpcode_slash", "direct_helpcode_uppercase",
    ])
    XCTAssertEqual(mode("direct").title, "双拼末尾直接辅助码")
    XCTAssertEqual(mode("trigger_semicolon").title, "双拼末尾间接触发（分号）")
    XCTAssertEqual(mode("mid_sentence").title, "双拼句中间接辅助码")
    XCTAssertEqual(mode("direct_helpcode_uppercase").title, "双拼逐音节直接（第二码大写结束）")
    XCTAssertEqual(HelpcodeSettingsView.strictFilterInfo, "关闭时一码只把匹配项前移；打开后排除辅助码不匹配的候选。两码本来就是严格筛选。")
  }

  /// 缺字段时等于 client-core 的 `ShuangpinHelpcodeModes::default()`：末尾直接、末尾分号、句中反引号、逐音节 / 结束开，其余关。
  func testMissingFieldsReadAsFactoryDefaults() {
    let on: Set<String> = ["direct", "trigger_semicolon", "mid_sentence_backtick", "direct_helpcode_slash"]
    for mode in HelpcodeSettingsView.modes {
      XCTAssertEqual(mode.fallback, on.contains(mode.field), mode.field)
      XCTAssertEqual(HelpcodeSettingsView.modeValue(mode, stored: [:]), on.contains(mode.field), mode.field)
    }
    XCTAssertFalse(HelpcodeSettingsView.modeValue(mode("direct"), stored: ["direct": false]))
  }

  /// 辅助码关着时全部置灰；句中三枚触发键随句中间接，两种结束方式随逐音节直接。
  func testGreyOutFollowsTheSwitches() {
    for mode in HelpcodeSettingsView.modes {
      XCTAssertFalse(HelpcodeSettingsView.modeEnabled(mode, helpcodeOn: false, stored: ["mid_sentence": true, "direct_helpcode": true]))
    }
    XCTAssertTrue(HelpcodeSettingsView.modeEnabled(mode("direct"), helpcodeOn: true, stored: [:]))
    XCTAssertFalse(HelpcodeSettingsView.modeEnabled(mode("mid_sentence_semicolon"), helpcodeOn: true, stored: [:]))
    XCTAssertTrue(HelpcodeSettingsView.modeEnabled(mode("mid_sentence_semicolon"), helpcodeOn: true, stored: ["mid_sentence": true]))
    XCTAssertFalse(HelpcodeSettingsView.modeEnabled(mode("direct_helpcode_slash"), helpcodeOn: true, stored: [:]))
    XCTAssertTrue(HelpcodeSettingsView.modeEnabled(mode("direct_helpcode_slash"), helpcodeOn: true, stored: ["direct_helpcode": true]))
  }

  /// 引擎只在末尾直接和逐音节直接都关着时认末尾间接触发键（`apply_request_options`）：出厂值的末尾直接开着，这三行置灰；两者都关时才可改，句中间接不影响它们。
  func testEndTriggersFollowDirectAndDirectHelpcode() {
    for field in ["trigger_semicolon", "trigger_backtick", "trigger_uppercase"] {
      XCTAssertFalse(HelpcodeSettingsView.modeEnabled(mode(field), helpcodeOn: true, stored: [:]), field)
      XCTAssertFalse(HelpcodeSettingsView.modeEnabled(mode(field), helpcodeOn: true, stored: ["direct": true]), field)
      XCTAssertFalse(HelpcodeSettingsView.modeEnabled(mode(field), helpcodeOn: true, stored: ["direct": false, "direct_helpcode": true]), field)
      XCTAssertTrue(HelpcodeSettingsView.modeEnabled(mode(field), helpcodeOn: true, stored: ["direct": false]), field)
      XCTAssertTrue(HelpcodeSettingsView.modeEnabled(mode(field), helpcodeOn: true, stored: ["direct": false, "mid_sentence": true]), field)
      XCTAssertFalse(HelpcodeSettingsView.modeEnabled(mode(field), helpcodeOn: false, stored: ["direct": false]), field)
    }
    // 置灰不改值：出厂的末尾分号仍读作开。
    XCTAssertTrue(HelpcodeSettingsView.modeValue(mode("trigger_semicolon"), stored: [:]))
    XCTAssertEqual(mode("direct").info, "完整音节后直接接辅助码；打开时末尾间接触发不生效")
  }

  /// 触屏键盘上的分号、反引号和 / 按字面上屏，用到它们的行在说明里写明要外接键盘。
  func testTouchUnreachableKeysSayHardwareKeyboard() {
    let hardware: Set<String> = ["trigger_semicolon", "trigger_backtick", "mid_sentence_backtick", "mid_sentence_semicolon", "direct_helpcode_slash"]
    for mode in HelpcodeSettingsView.modes {
      XCTAssertEqual(mode.info.contains("外接键盘"), hardware.contains(mode.field), mode.field)
    }
  }

  /// 只改一个字段，`modes` 里其他键（macOS 同步来的 manual_tab）留着；缺双拼对象时按出厂值补齐必填的 schema。
  func testMergeKeepsOtherKeys() {
    var document: [String: Any] = ["shuangpin_helpcode": ["enabled": true, "schema": "xiaohe", "modes": ["manual_tab": true]]]
    HelpcodeSettingsView.mergeMode("trigger_uppercase", true, document: &document)
    let helpcode = document["shuangpin_helpcode"] as? [String: Any]
    XCTAssertEqual(helpcode?["schema"] as? String, "xiaohe")
    XCTAssertEqual(helpcode?["modes"] as? [String: Bool], ["manual_tab": true, "trigger_uppercase": true])

    var empty: [String: Any] = [:]
    HelpcodeSettingsView.mergeMode("direct", false, document: &empty)
    let created = empty["shuangpin_helpcode"] as? [String: Any]
    XCTAssertEqual(created?["schema"] as? String, "lantian")
    XCTAssertEqual(created?["modes"] as? [String: Bool], ["direct": false])
  }

  /// 保存经 client-core 的写入端归一：打开句中间接后，句中反引号（出厂开）占走反引号，末尾反引号被清掉；读回的就是页面显示的值。
  func testSavedModesComeBackNormalized() {
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      HelpcodeSettingsView.mergeMode("trigger_backtick", true, document: &$0)
    })
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      HelpcodeSettingsView.mergeMode("mid_sentence", true, document: &$0)
    })
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    let stored = (document?["shuangpin_helpcode"] as? [String: Any])?["modes"] as? [String: Any] ?? [:]
    XCTAssertTrue(HelpcodeSettingsView.modeValue(mode("mid_sentence"), stored: stored))
    XCTAssertFalse(HelpcodeSettingsView.modeValue(mode("trigger_backtick"), stored: stored))
    XCTAssertTrue(HelpcodeSettingsView.modeValue(mode("direct"), stored: stored))
    XCTAssertTrue(HelpcodeSettingsView.modeValue(mode("trigger_semicolon"), stored: stored))
  }
}

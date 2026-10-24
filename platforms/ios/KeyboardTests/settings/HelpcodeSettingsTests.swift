import XCTest

/// The settings app's helpcode page merges single fields into the helpcode objects of the shared document; the keyboard has to act on them, and needs the Engine's tables to act with.
@MainActor
final class HelpcodeSettingsTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-helpcode-\(UUID().uuidString)", isDirectory: true)
    XCTAssertNoThrow(try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true))
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// Every scheme the page offers has its table beside the dictionaries; a missing one reads as helpcode narrowing nothing.
  func testEveryOfferedSchemaShipsItsTable() throws {
    let resources = try XCTUnwrap(Bundle.main.url(forResource: "EngineResources", withExtension: nil))
    let tables = ["helpcode.txt", "zrm_helpcode_big_unique.txt", "shouyou2_0_helpcode.txt",
                  "shouyouplus_helpcode.txt", "xiaohe_helpcode.txt", "jiajia_helpcode.txt",
                  "wubi86_helpcode.txt"]
    for table in tables {
      let path = resources.appendingPathComponent("helpcodes/\(table)").path
      XCTAssertTrue(FileManager.default.fileExists(atPath: path), "\(table) is not bundled")
    }
  }

  /// A Shift letter narrows the composition by helpcode, and turning helpcode off in the app reaches the session the keyboard already has.
  func testReloadTurnsHelpcodeOffInTheLiveSession() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed, bridge.cancel().diagnosticText ?? "运行时初始化失败")
    let plain = compose(bridge, "shi", helpcode: nil)
    // 自然码 Y is the 讠 radical: 识 试 诗 lead instead of 是.
    let narrowed = compose(bridge, "shi", helpcode: "Y")
    XCTAssertEqual(plain.candidates.first, "是")
    XCTAssertEqual(narrowed.preedit, "shiY")
    XCTAssertEqual(narrowed.candidates.first, "识")

    update { $0["enabled"] = false }
    await reload(bridge)

    // 关掉辅助码后，Shift 字母成为大写字面片段；只转换前面的 shi，不再按讠部首筛出「识」。
    let off = compose(bridge, "shi", helpcode: "Y")
    let lowercase = compose(bridge, "shiy", helpcode: nil)
    XCTAssertEqual(off.preedit, "shiY")
    XCTAssertEqual(off.candidates.first, plain.candidates.first.map { $0 + "Y" })
    XCTAssertNotEqual(off.candidates.first, lowercase.candidates.first)
    XCTAssertNotEqual(off.candidates.first, "识")
  }

  /// "Show in the candidate bar" puts each candidate's helpcode in the annotations the strip draws.
  func testShownHelpcodeReachesTheCandidateAnnotations() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed, bridge.cancel().diagnosticText ?? "运行时初始化失败")
    XCTAssertEqual(compose(bridge, "shi", helpcode: nil).candidateAnnotations.first, "", "全拼 ships with helpcode hidden")

    update { $0["show_in_candidate_window"] = true }
    await reload(bridge)

    let shown = compose(bridge, "shi", helpcode: nil)
    let first = shown.candidateAnnotations.first ?? ""
    // Bracketed the way the Engine appends it on desktop; the strip drops the brackets when it draws.
    XCTAssertTrue(first.hasPrefix("(") && first.hasSuffix(")"), "\(first) is not a bracketed helpcode")
    XCTAssertTrue(first.dropFirst().dropLast().allSatisfy { $0.isLetter }, "\(first) is not a helpcode")
  }

  /// #9416：小鹤拼音 yh 后输入自然码 m，裸码与 Shift 辅助码都要把「杨」放进当前候选页。
  func testXiaoheZiranmaYangCandidateAndCommit() async throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed, bridge.cancel().diagnosticText ?? "运行时初始化失败")
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var helpcode = $0["shuangpin_helpcode"] as? [String: Any] ?? [:]
      helpcode["enabled"] = true
      helpcode["schema"] = "ziranma"
      helpcode["show_in_candidate_window"] = true
      $0["shuangpin_helpcode"] = helpcode
    })
    await reload(bridge)
    _ = bridge.switch(toShuangpinProfile: "xiaohe")

    for (code, shifted) in [("m", false), ("M", true)] {
      _ = bridge.cancel()
      _ = bridge.handleCharacter("y")
      let plain = bridge.handleCharacter("h")
      XCTAssertTrue(plain.candidates.contains("阳"), "未输入辅助码时保留原读音候选")
      let narrowed = bridge.handleCharacter(code, shifted: shifted)
      XCTAssertNil(narrowed.diagnosticText)
      XCTAssertTrue(narrowed.candidates.prefix(2).contains("杨"), "辅助码结果前两项应包含「杨」，不依赖上屏学习后的顺序")
      XCTAssertFalse(narrowed.candidates.contains("阳"), "非木部的高频候选不应挤在辅助码结果中")
      let index = try XCTUnwrap(narrowed.candidates.firstIndex(of: "杨"))
      XCTAssertEqual(narrowed.candidateAnnotations[index], "(mY)")
      let committed = bridge.selectCandidate(at: UInt(index))
      XCTAssertEqual(committed.commitText, "杨")
      XCTAssertTrue(committed.preedit.isEmpty)
    }
  }

  /// 在设置 App 里导入的码表（#7176）装进键盘读的插件目录，并选中到发起导入的那一份；已经打开的键盘重读设置后按它缩小候选，删掉后退回内置方案。
  func testImportedTableReachesTheLiveSessionAndRemovalFallsBack() async throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed, bridge.cancel().diagnosticText ?? "运行时初始化失败")
    // 自然码 Y 是讠旁，「识」排第一。
    XCTAssertEqual(compose(bridge, "shi", helpcode: "Y").candidates.first, "识")

    // 十万条合成码表，只有末行「是」以 y 开头，其余 shi 音的字都以 q 开头。
    let table = state.appendingPathComponent("测试码表.txt")
    var text = "# name: 测试码表\n识=qa\n时=qb\n事=qc\n十=qd\n使=qe\n市=qf\n世=qg\n式=qh\n实=qi\n"
    for index in 0..<99_990 {
      text += "\(UnicodeScalar(0x20000 + index)!)=ab\n"
    }
    text += "是=ya\n"
    try text.write(to: table, atomically: true, encoding: .utf8)
    let message = HelpcodeSettingsView.importTable(table, into: HelpcodeSettingsView.quanpin, stateRoot: state)
    XCTAssertEqual(message, "已导入「测试码表」，共 100000 条，已切换到这套辅助码")

    let packs = HelpcodeSettingsView.installedPacks(stateRoot: state)
    XCTAssertEqual(packs.map(\.name), ["测试码表"])
    let pack = try XCTUnwrap(packs.first)
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: HelpcodeSettingsView.quanpin, in: document), pack.id)
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: HelpcodeSettingsView.shuangpin, in: document), "")

    await reload(bridge)
    XCTAssertEqual(compose(bridge, "shi", helpcode: "Y").candidates.first, "是")

    XCTAssertEqual(HelpcodeSettingsView.remove(pack, stateRoot: state), "已删除「测试码表」，已改回内置方案")
    XCTAssertEqual(HelpcodeSettingsView.installedPacks(stateRoot: state), [])
    let cleared = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: HelpcodeSettingsView.quanpin, in: cleared), "")

    await reload(bridge)
    XCTAssertEqual(compose(bridge, "shi", helpcode: "Y").candidates.first, "识")
  }

  /// 坏表整张拒绝，提示里是 client-services 给的文件名、原因和行号；GBK 码表在调用 host-api 之前就被拒；什么都没装上。
  func testRejectedTablesInstallNothing() throws {
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    let duplicate = state.appendingPathComponent("mine.txt")
    try "你=ab\n你=cd\n".write(to: duplicate, atomically: true, encoding: .utf8)
    XCTAssertEqual(HelpcodeSettingsView.importTable(duplicate, into: HelpcodeSettingsView.shuangpin, stateRoot: state),
                   "无法导入：mine.txt 里「你」出现了不止一次（第 1 行和第 2 行）")

    let gbk = state.appendingPathComponent("gbk.txt")
    try Data([0xC4, 0xE3, 0x3D, 0x6E, 0x69, 0x0A]).write(to: gbk)
    XCTAssertEqual(HelpcodeSettingsView.importTable(gbk, into: HelpcodeSettingsView.shuangpin, stateRoot: state),
                   "码表不是 UTF-8 编码，请先另存为 UTF-8 再导入")

    let large = state.appendingPathComponent("large.txt")
    try Data(repeating: 0x23, count: HelpcodeChoicePolicy.maximumTableBytes + 1).write(to: large)
    XCTAssertEqual(HelpcodeSettingsView.importTable(large, into: HelpcodeSettingsView.shuangpin, stateRoot: state),
                   "码表超过 1 MiB，无法导入")

    XCTAssertEqual(HelpcodeSettingsView.installedPacks(stateRoot: state), [])
  }

  private func update(_ change: (inout [String: Any]) -> Void) {
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var helpcode = $0["quanpin_helpcode"] as? [String: Any] ?? [:]
      change(&helpcode)
      $0["quanpin_helpcode"] = helpcode
    })
  }

  private func reload(_ bridge: MetasequoiaInputSessionBridge) async {
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")
  }

  /// The snapshot after typing `letters` from an empty composition, then a Shift `helpcode` letter; the composition is then abandoned.
  private func compose(_ bridge: MetasequoiaInputSessionBridge, _ letters: String, helpcode: String?) -> MetasequoiaInputSnapshot {
    var snapshot = bridge.cancel()
    for letter in letters { snapshot = bridge.handleCharacter(String(letter)) }
    if let helpcode { snapshot = bridge.handleCharacter(helpcode, shifted: true) }
    _ = bridge.cancel()
    return snapshot
  }
}

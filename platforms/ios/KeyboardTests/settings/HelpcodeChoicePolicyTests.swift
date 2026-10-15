import XCTest

/// 辅助码页在内置方案和导入的码表之间选择的规则（#7176），与共享设置页 `helpcode-page.tsx` 和 Android 的 `HelpcodeChoicePolicy` 相同。
final class HelpcodeChoicePolicyTests: XCTestCase {
  private let quanpin = HelpcodeSettingsView.quanpin
  private let shuangpin = HelpcodeSettingsView.shuangpin

  /// 选包只写这一份的包键：`schema` 不动，`plugins` 里的按键音包和另一份的选择都原样保留。
  func testChoosingAPackWritesOnlyThatFamilysPackKey() {
    var document: [String: Any] = [
      "shuangpin_helpcode": ["enabled": true, "schema": "xiaohe", "show_in_candidate_window": true],
      "plugins": ["key_sound": ["pack": "msime-bubble"], "helpcode_pack_quanpin": "custom-other"],
    ]
    HelpcodeChoicePolicy.choose("pack:custom-mine", for: shuangpin, document: &document)

    let plugins = document["plugins"] as? [String: Any]
    XCTAssertEqual(plugins?["helpcode_pack_shuangpin"] as? String, "custom-mine")
    XCTAssertEqual(plugins?["helpcode_pack_quanpin"] as? String, "custom-other")
    XCTAssertEqual((plugins?["key_sound"] as? [String: Any])?["pack"] as? String, "msime-bubble")
    XCTAssertEqual(HelpcodeSettingsView.schema(of: shuangpin, in: document), "xiaohe")
  }

  /// 选内置方案写 `schema`，原来选着包时把包清成空字符串，退回内置方案。
  func testChoosingABuiltInSchemaClearsTheSelectedPack() {
    var document: [String: Any] = [
      "quanpin_helpcode": ["enabled": true, "schema": "ziranma", "show_in_candidate_window": false],
      "plugins": ["helpcode_pack_quanpin": "custom-mine", "helpcode_pack_shuangpin": "custom-mine"],
    ]
    HelpcodeChoicePolicy.choose("xiaohe", for: quanpin, document: &document)

    XCTAssertEqual(HelpcodeSettingsView.schema(of: quanpin, in: document), "xiaohe")
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: quanpin, in: document), "")
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: shuangpin, in: document), "custom-mine")
  }

  /// 没选包时选内置方案不碰 `plugins`：文档里不会凭空多出一个空的包键。
  func testChoosingABuiltInSchemaWithoutAPackLeavesPluginsAlone() {
    var document: [String: Any] = ["plugins": ["key_sound": ["pack": "default"]]]
    HelpcodeChoicePolicy.choose("jiajia", for: quanpin, document: &document)

    XCTAssertEqual(document["plugins"] as? [String: [String: String]], ["key_sound": ["pack": "default"]])
    // 缺对象时按该系列的引擎默认值补齐，再写方案。
    let object = document["quanpin_helpcode"] as? [String: Any]
    XCTAssertEqual(object?["schema"] as? String, "jiajia")
    XCTAssertEqual(object?["enabled"] as? Bool, true)
    XCTAssertEqual(object?["show_in_candidate_window"] as? Bool, false)
  }

  /// 删除包时，两份里指向它的选择都清掉；指向别的包的选择不动。
  func testRemovingAPackClearsEverySelectionOfIt() {
    var document: [String: Any] = [
      "plugins": ["helpcode_pack_quanpin": "custom-mine", "helpcode_pack_shuangpin": "custom-mine", "effect_style": "flash"],
    ]
    XCTAssertTrue(HelpcodeChoicePolicy.clearSelections(of: "custom-mine", document: &document))
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: quanpin, in: document), "")
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: shuangpin, in: document), "")
    XCTAssertEqual((document["plugins"] as? [String: Any])?["effect_style"] as? String, "flash")

    var other: [String: Any] = ["plugins": ["helpcode_pack_shuangpin": "custom-other"]]
    XCTAssertFalse(HelpcodeChoicePolicy.clearSelections(of: "custom-mine", document: &other))
    XCTAssertEqual(HelpcodeChoicePolicy.selectedPack(of: shuangpin, in: other), "custom-other")
  }

  /// 列表里每个包一项「名称（插件）」；选中的包已不在时补一项「id（插件，未找到）」，让用户看得见并能改掉它。
  func testPackOptionsShowAMissingSelection() {
    let packs = [HelpcodeChoicePolicy.Pack(id: "custom-mine", name: "我的码表")]
    XCTAssertEqual(HelpcodeChoicePolicy.packOptions(packs, selectedPack: "custom-mine"),
                   [.init(value: "pack:custom-mine", label: "我的码表（插件）")])
    XCTAssertEqual(HelpcodeChoicePolicy.packOptions(packs, selectedPack: "custom-gone"),
                   [.init(value: "pack:custom-mine", label: "我的码表（插件）"),
                    .init(value: "pack:custom-gone", label: "custom-gone（插件，未找到）")])
    XCTAssertEqual(HelpcodeChoicePolicy.selectedValue(schema: "lantian", selectedPack: ""), "lantian")
    XCTAssertEqual(HelpcodeChoicePolicy.selectedValue(schema: "lantian", selectedPack: "custom-gone"), "pack:custom-gone")
  }

  /// 目录里只取辅助码表包；没有名字的用 id 显示，读不懂的目录得到空列表。
  func testCatalogKeepsOnlyHelpcodePacks() {
    let catalog: [String: Any] = ["packages": [
      ["kind": "sound", "id": "msime-bubble", "name": "气泡"],
      ["kind": "helpcode", "id": "custom-mine", "name": "我的码表"],
      ["kind": "helpcode", "id": "custom-bare", "name": ""],
      ["kind": "helpcode", "id": ""],
    ], "issues": []]
    XCTAssertEqual(HelpcodeChoicePolicy.packs(inCatalog: catalog),
                   [.init(id: "custom-mine", name: "我的码表"), .init(id: "custom-bare", name: "custom-bare")])
    XCTAssertEqual(HelpcodeChoicePolicy.packs(inCatalog: NSNull()), [])
  }

  /// 码表严格按 UTF-8 解码；超过上限的不读进来。
  func testDecodingIsStrict() {
    XCTAssertEqual(try HelpcodeChoicePolicy.decode(Data("你=ni\n".utf8)).get(), "你=ni\n")
    // 「你」的 GBK 编码，不是合法的 UTF-8。
    XCTAssertEqual(HelpcodeChoicePolicy.decode(Data([0xC4, 0xE3, 0x3D, 0x6E, 0x69])), .failure(.notUTF8))
    XCTAssertEqual(HelpcodeChoicePolicy.decode(nil), .failure(.tooLarge))
  }

  /// 码表不合格时把 client-services 的原因原样告诉用户，其余失败码给固定的提示。
  func testImportFailureMessages() {
    typealias Failure = MetasequoiaInputSessionBridge.PluginFailure
    XCTAssertEqual(HelpcodeChoicePolicy.importFailure(Failure(code: "plugin_invalid", detail: "mine.txt 第 2 行：码必须是 1 到 2 个小写字母")),
                   "无法导入：mine.txt 第 2 行：码必须是 1 到 2 个小写字母")
    XCTAssertEqual(HelpcodeChoicePolicy.importFailure(Failure(code: "plugin_storage")), "保存码表失败，请重试")
    XCTAssertEqual(HelpcodeChoicePolicy.importFailure(Failure(code: "invalid")), "无法导入这个码表，请确认它是 1 MiB 以内的 UTF-8 文本")
    XCTAssertEqual(HelpcodeChoicePolicy.importFailure(Failure(code: "plugin_invalid")), "导入失败，请重试")
  }
}

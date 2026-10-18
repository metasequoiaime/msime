import XCTest
@testable import MSIMEBackend

/// macOS 设置同步仍在 Swift 里合并的那一半（`BackendAccountClient.mergedPreferences`，字段表和云端文档由 Rust 给），以及 iOS App 和键盘扩展共用的版本身份（`MSIMEAppEdition`）。iOS 的设置映射和版本过滤在 Rust（client-services 的 `account::ios_settings`、client-core 的 `edition`）。
final class BackendPreferencesTests: XCTestCase {
  func testMergePreservesOtherPlatformsAndReadRevision() throws {
    let base = BackendAccountClient.Preferences(revision: 42, settings: ["appearance.page_size": .integer(7), "input.schema": .string("quanpin")])
    let schema = BackendAccountClient.PreferenceSchema(fields: ["input.schema": .init(type: "string")], maximum_bytes: 65536, update_mode: "replace", revision_required: true)
    let merged = try BackendAccountClient.mergedPreferences(base, replacing: ["input.schema": .string("wubi")], schema: schema)
    XCTAssertEqual(merged.revision, 42)
    XCTAssertEqual(merged.settings["appearance.page_size"], .integer(7))
    XCTAssertEqual(merged.settings["input.schema"], .string("wubi"))
    XCTAssertThrowsError(try BackendAccountClient.mergedPreferences(base, replacing: ["platform.ios.nine_key": .boolean(true)], schema: schema),
                         "nothing the server knows is a failed upload, not an empty success")
    XCTAssertThrowsError(try BackendAccountClient.mergedPreferences(base, replacing: ["input.schema": .boolean(true)], schema: schema),
                         "a known key with the wrong type is still refused")
  }

  /// 本机比服务端新：服务端字段表里还没有的键（例如 backend v0.54.0 之前的 `platform.ios.keyboard_layout`）留在本机，其余照常上传，与 Android 和 Tauri 的 iOS 工程一样按字段表过滤，不让整次上传失败。
  func testKeysTheServerDoesNotKnowYetStayLocal() throws {
    let base = BackendAccountClient.Preferences(revision: 3, settings: ["platform.android.keyboard_layout": .string("nine_key")])
    let older = BackendAccountClient.PreferenceSchema(
      fields: ["input.schema": .init(type: "string"), "platform.ios.nine_key": .init(type: "boolean")],
      maximum_bytes: 65536, update_mode: "replace", revision_required: true)
    let local: [String: BackendPreferenceValue] = [
      "input.schema": .string("quanpin"), "platform.ios.nine_key": .boolean(false),
      "platform.ios.keyboard_layout": .string("fourteen_key"),
    ]
    let merged = try BackendAccountClient.mergedPreferences(base, replacing: local, schema: older)
    XCTAssertEqual(merged.settings, [
      "platform.android.keyboard_layout": .string("nine_key"),
      "input.schema": .string("quanpin"), "platform.ios.nine_key": .boolean(false),
    ])
    var fields = older.fields
    fields["platform.ios.keyboard_layout"] = .init(type: "string")
    let current = BackendAccountClient.PreferenceSchema(fields: fields, maximum_bytes: 65536, update_mode: "replace", revision_required: true)
    XCTAssertEqual(try BackendAccountClient.mergedPreferences(base, replacing: local, schema: current)
      .settings["platform.ios.keyboard_layout"], .string("fourteen_key"))
  }

  func testPhotoSizedPrivateSettingsStayIntactWithinNegotiatedLimit() throws {
    let photo = String(repeating: "A", count: 4 * ((512000 + 2) / 3))
    let json = "{\"photo\":\"" + photo + "\"}"
    let base = BackendAccountClient.Preferences(revision: 1, settings: [:])
    let key = "platform.ios.custom_keyboard_skin"
    let schema = BackendAccountClient.PreferenceSchema(fields: [key: .init(type: "string")], maximum_bytes: 1024 * 1024, update_mode: "replace", revision_required: true)
    let merged = try BackendAccountClient.mergedPreferences(base, replacing: [key: .string(json)], schema: schema)
    XCTAssertEqual(merged.settings[key], .string(json))
    let old = BackendAccountClient.PreferenceSchema(fields: schema.fields, maximum_bytes: 65536, update_mode: "replace", revision_required: true)
    XCTAssertThrowsError(try BackendAccountClient.mergedPreferences(base, replacing: [key: .string(json)], schema: old))
  }

  /// 版本表里一个版本在 Info.plist 里的样子：full 什么也不写，其他版本写版本 id、方案和默认方案。取自 shared/contracts/editions.json，测试跟着版本表走，不自己抄一份方案列表。
  private func editionInfo(_ id: String) throws -> [String: Any] {
    let table = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
      .appendingPathComponent("../../contracts/editions.json").standardizedFileURL
    let document = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: table)) as? [String: Any])
    let editions = try XCTUnwrap(document["editions"] as? [[String: Any]])
    let edition = try XCTUnwrap(editions.first { $0["id"] as? String == id })
    if id == "full" { return [:] }
    var info: [String: Any] = ["MSIMEEdition": id, "MSIMEInputSchemes": try XCTUnwrap(edition["input_schemes"] as? [String]),
                               "MSIMEDefaultScheme": try XCTUnwrap(edition["default_scheme"] as? String)]
    if let mixed = (edition["preference_defaults"] as? [String: Any])?["wubi_mixed_pinyin"] as? Bool {
      info["MSIMEWubiMixedPinyinDefault"] = mixed
    }
    return info
  }

  func testFullKeepsTodaysAppGroupAndSchemes() throws {
    let info = try editionInfo("full")
    XCTAssertEqual(MSIMEAppEdition.identifier(in: info), "full")
    XCTAssertEqual(MSIMEAppEdition.appGroupIdentifier(in: info), "group.app.msime.ios")
    XCTAssertNil(MSIMEAppEdition.inputSchemes(in: info))
    XCTAssertEqual(MSIMEAppEdition.defaultScheme(in: info), "quanpin")
    XCTAssertFalse(MSIMEAppEdition.wubiMixedPinyinDefault(in: info))
    // 测试进程不是 App bundle，读到的就是 full。
    XCTAssertEqual(MSIMEAppEdition.appGroupIdentifier, "group.app.msime.ios")
    // 写明 full 的 Info.plist 和不写一样。
    XCTAssertEqual(MSIMEAppEdition.appGroupIdentifier(in: ["MSIMEEdition": "full", "MSIMEInputSchemes": ["wubi"]]), "group.app.msime.ios")
    XCTAssertNil(MSIMEAppEdition.inputSchemes(in: ["MSIMEEdition": "full", "MSIMEInputSchemes": ["wubi"]]))
  }

  func testOtherEditionsGetTheirOwnAppGroupAndDefaultScheme() throws {
    let wubi = try editionInfo("wubi")
    let pinyin = try editionInfo("pinyin")
    XCTAssertEqual(MSIMEAppEdition.appGroupIdentifier(in: wubi), "group.app.msime.ios.wubi")
    XCTAssertEqual(MSIMEAppEdition.appGroupIdentifier(in: pinyin), "group.app.msime.ios.pinyin")
    XCTAssertEqual(MSIMEAppEdition.inputSchemes(in: wubi), ["wubi"])
    XCTAssertEqual(MSIMEAppEdition.defaultScheme(in: wubi), "wubi")
    XCTAssertEqual(MSIMEAppEdition.defaultScheme(in: pinyin), "quanpin")
    // 五笔版的混拼默认开，拼音版没有这一项。
    XCTAssertTrue(MSIMEAppEdition.wubiMixedPinyinDefault(in: wubi))
    XCTAssertFalse(MSIMEAppEdition.wubiMixedPinyinDefault(in: pinyin))
    // 声明的默认方案不在方案里时退回第一个方案，回退到的方案一定能跑。
    XCTAssertEqual(MSIMEAppEdition.defaultScheme(in: ["MSIMEEdition": "wubi", "MSIMEInputSchemes": ["wubi"], "MSIMEDefaultScheme": "quanpin"]), "wubi")
  }
}

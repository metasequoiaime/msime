import XCTest
@testable import MSIMEBackend

private final class MalformedPreferencesProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool {
    ["/v1/users/me/preferences", "/v1/users/me/preferences/schema"].contains(request.url?.path)
  }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let body: String
    if request.url?.path.hasSuffix("/schema") == true {
      body = #"{"fields":{"bad key":{"type":"object"}},"maximum_bytes":0,"update_mode":"merge","revision_required":false}"#
    } else {
      body = #"{"revision":-1,"settings":{"bad key":true}}"#
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type":"application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(body.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class PreferenceBodyProtocol: URLProtocol {
  private static let lock = NSLock()
  private static var requestCount = 0

  static func reset() {
    lock.lock(); defer { lock.unlock() }
    requestCount = 0
  }

  static func requests() -> Int {
    lock.lock(); defer { lock.unlock() }
    return requestCount
  }

  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    Self.lock.lock(); Self.requestCount += 1; Self.lock.unlock()
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
      headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(#"{"revision":1,"settings":{}}"#.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendPreferencesTests: XCTestCase {
  func testMalformedPreferenceResponsesAreRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [MalformedPreferencesProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    do {
      _ = try await client.preferences(token: "session")
      XCTFail("malformed preferences accepted")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }
    do {
      _ = try await client.preferenceSchema(token: "session")
      XCTFail("malformed preference schema accepted")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }
    do {
      _ = try await client.putPreferences(.init(revision: 0, settings: [:]), token: "session")
      XCTFail("malformed preference update accepted")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 0) }
  }
  func testMergePreservesOtherPlatformsAndReadRevision() throws {
    let base = BackendAccountClient.Preferences(revision: 42, settings: ["appearance.page_size": .integer(7), "input.schema": .string("quanpin")])
    let schema = BackendAccountClient.PreferenceSchema(fields: ["input.schema": .init(type: "string")], maximum_bytes: 65536, update_mode: "replace", revision_required: true)
    let merged = try BackendAccountClient.mergedPreferences(base, replacing: ["input.schema": .string("wubi")], schema: schema)
    XCTAssertEqual(merged.revision, 42)
    XCTAssertEqual(merged.settings["appearance.page_size"], .integer(7))
    XCTAssertEqual(merged.settings["input.schema"], .string("wubi"))
    XCTAssertThrowsError(try BackendAccountClient.mergedPreferences(base, replacing: ["platform.ios.nine_key": .boolean(true)], schema: schema))
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

  func testPutPreferencesRejectsAnOversizedWholeDocumentBeforeSending() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [PreferenceBodyProtocol.self]
    let client = BackendAccountClient(configuration: configuration)
    let value = String(repeating: "x", count: 600_000)
    PreferenceBodyProtocol.reset()
    do {
      _ = try await client.putPreferences(
        .init(revision: 1, settings: ["first": .string(value), "second": .string(value)]), token: "session")
      XCTFail("oversized preference document sent")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 400)
    }
    XCTAssertEqual(PreferenceBodyProtocol.requests(), 0)
  }
  private let themes: Set<String> = ["system", "shuishan", "light", "paper", "night", "ink", "custom"]

  func testUnsupportedCloudValuesFailBeforeAnApplicationPlanExists() throws {
    for settings: [String: BackendPreferenceValue] in [
      ["input.schema": .string("shuangpin"), "input.shuangpin_schema": .string("unsupported")],
      ["input.schema": .string("wubi"), "input.wubi_schema": .string("wubi06")],
      ["platform.ios.sound_enabled": .string("true")],
      ["platform.ios.haptic_strength": .string("unsafe")],
      // 云端文档不带语言方案；键盘对这些方案不写 `input.schema`（ChineseInputScheme.cloudSchema）。
      ["input.schema": .string("cantonese")],
      ["input.schema": .string("zhuyin")],
      ["input.schema": .string("vietnamese")],
      ["input.schema": .string("tibetan")],
      ["input.schema": .string("stroke")]
    ] { XCTAssertThrowsError(try IOSPreferencePlan(settings, themes: themes)) }
    let japanese = try IOSPreferencePlan(["input.schema": .string("japanese"), "platform.ios.nine_key": .boolean(true)], themes: themes)
    XCTAssertEqual(japanese.scheme, "japaneseNineKey")
    let roman = try IOSPreferencePlan(["input.schema": .string("japanese"), "platform.ios.nine_key": .boolean(false)], themes: themes)
    XCTAssertEqual(roman.scheme, "japanese")
    let korean = try IOSPreferencePlan(["input.schema": .string("korean"), "platform.ios.nine_key": .boolean(true)], themes: themes)
    XCTAssertEqual(korean.scheme, "korean")
    let nine = try IOSPreferencePlan(["input.schema": .string("quanpin"), "platform.ios.nine_key": .boolean(true)], themes: themes)
    XCTAssertEqual(nine.scheme, "nineKey")
    XCTAssertNil(nine.sound)
    let shuangpin = try IOSPreferencePlan(["input.schema": .string("shuangpin"), "input.shuangpin_schema": .string("ziranma"), "input.character_set": .string("traditional")], themes: themes)
    XCTAssertEqual(shuangpin.scheme, "ziranma")
    XCTAssertEqual(shuangpin.traditional, true)
    XCTAssertNil(shuangpin.wubiProfile)
    let wubi98 = try IOSPreferencePlan(["input.schema": .string("wubi"), "input.wubi_schema": .string("wubi98")], themes: themes)
    XCTAssertEqual(wubi98.scheme, "wubi")
    XCTAssertEqual(wubi98.wubiProfile, "wubi98")
    XCTAssertEqual(try IOSPreferencePlan(["input.schema": .string("wubi"), "input.wubi_schema": .string("wubi86")], themes: themes).wubiProfile, "wubi86")
    // 不认识 98 五笔的设备上传的设置没有版本，应用时保留本机的版本。
    let older = try IOSPreferencePlan(["input.schema": .string("wubi")], themes: themes)
    XCTAssertEqual(older.scheme, "wubi")
    XCTAssertNil(older.wubiProfile)
  }

  func testThemeValuesAreCheckedAgainstTheCatalog() throws {
    let plan = try IOSPreferencePlan(["platform.ios.global_theme": .string("custom"), "platform.ios.custom_theme_base": .string("paper"), "platform.ios.custom_keyboard_skin": .string("{}")], themes: themes)
    XCTAssertEqual(plan.globalTheme, "custom")
    XCTAssertEqual(plan.customThemeBase, "paper")
    XCTAssertEqual(plan.customSkinJSON, "{}")
    XCTAssertEqual(try IOSPreferencePlan(["platform.ios.custom_theme_base": .string("system")], themes: themes).customThemeBase, "system")
    for settings: [String: BackendPreferenceValue] in [
      ["platform.ios.global_theme": .string("ocean")],
      ["platform.ios.global_theme": .boolean(true)],
      ["platform.ios.custom_theme_base": .string("custom")],
      ["platform.ios.custom_theme_base": .string("midnight")]
    ] { XCTAssertThrowsError(try IOSPreferencePlan(settings, themes: themes)) }
    let empty = try IOSPreferencePlan([:], themes: themes)
    XCTAssertNil(empty.globalTheme)
    XCTAssertNil(empty.customThemeBase)
    XCTAssertNil(empty.customSkinJSON)
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

  func testAccountSchemeFollowsTheEditionInBothDirections() throws {
    let local: [String: BackendPreferenceValue] = [
      "input.schema": .string("quanpin"), "platform.ios.nine_key": .boolean(true),
      "input.shuangpin_schema": .string("ziranma"), "input.wubi_schema": .string("wubi98"),
      "input.character_set": .string("traditional")
    ]
    // full 提供全部方案，两个方向都什么也不去掉。
    var full = local
    IOSPreferencePlan.filterUploaded(&full, offered: MSIMEAppEdition.inputSchemes(in: try editionInfo("full")))
    XCTAssertEqual(full, local)
    IOSPreferencePlan.filterDownloaded(&full, offered: nil)
    XCTAssertEqual(full, local)

    // 五笔版只有一个方案：方案和随它的九键开关不上传也不应用，双拼方案不上传，五笔版本照常上传。
    let wubi = MSIMEAppEdition.inputSchemes(in: try editionInfo("wubi"))
    var uploaded = local
    uploaded["input.schema"] = .string("wubi")
    IOSPreferencePlan.filterUploaded(&uploaded, offered: wubi)
    XCTAssertEqual(uploaded, ["input.wubi_schema": .string("wubi98"), "input.character_set": .string("traditional")])
    var downloaded = local
    IOSPreferencePlan.filterDownloaded(&downloaded, offered: wubi)
    XCTAssertNil(downloaded["input.schema"])
    XCTAssertNil(downloaded["platform.ios.nine_key"])
    XCTAssertEqual(downloaded["input.shuangpin_schema"], .string("ziranma"))

    // 拼音版：本版本的方案照常同步，账号里的五笔当作没有这一项，五笔版本不上传。
    let pinyin = MSIMEAppEdition.inputSchemes(in: try editionInfo("pinyin"))
    var pinyinUpload = local
    IOSPreferencePlan.filterUploaded(&pinyinUpload, offered: pinyin)
    XCTAssertEqual(pinyinUpload["input.schema"], .string("quanpin"))
    XCTAssertEqual(pinyinUpload["platform.ios.nine_key"], .boolean(true))
    XCTAssertNil(pinyinUpload["input.wubi_schema"])
    var fromWubi = local
    fromWubi["input.schema"] = .string("wubi")
    IOSPreferencePlan.filterDownloaded(&fromWubi, offered: pinyin)
    XCTAssertNil(fromWubi["input.schema"])
    XCTAssertNil(fromWubi["platform.ios.nine_key"])
    let plan = try IOSPreferencePlan(fromWubi, themes: themes)
    XCTAssertNil(plan.scheme)
    XCTAssertEqual(plan.traditional, true)
    // 不认识的取值不归版本过滤管，仍由 IOSPreferencePlan 按原来的规则拒绝。
    var unknown = local
    unknown["input.schema"] = .string("klingon")
    IOSPreferencePlan.filterDownloaded(&unknown, offered: pinyin)
    XCTAssertEqual(unknown["input.schema"], .string("klingon"))
    XCTAssertThrowsError(try IOSPreferencePlan(unknown, themes: themes))
  }
}

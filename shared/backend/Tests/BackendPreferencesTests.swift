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
  private let themes: Set<String> = ["system", "shuishan", "light", "paper", "night", "ink", "custom"]

  func testUnsupportedCloudValuesFailBeforeAnApplicationPlanExists() throws {
    for settings: [String: BackendPreferenceValue] in [
      ["input.schema": .string("shuangpin"), "input.shuangpin_schema": .string("unsupported")],
      ["input.schema": .string("wubi"), "input.wubi_schema": .string("wubi98")],
      ["platform.ios.sound_enabled": .string("true")],
      ["platform.ios.haptic_strength": .string("unsafe")]
    ] { XCTAssertThrowsError(try IOSPreferencePlan(settings, themes: themes)) }
    let japanese = try IOSPreferencePlan(["input.schema": .string("japanese"), "platform.ios.nine_key": .boolean(true)], themes: themes)
    XCTAssertEqual(japanese.scheme, "japaneseNineKey")
    let roman = try IOSPreferencePlan(["input.schema": .string("japanese"), "platform.ios.nine_key": .boolean(false)], themes: themes)
    XCTAssertEqual(roman.scheme, "japanese")
    let nine = try IOSPreferencePlan(["input.schema": .string("quanpin"), "platform.ios.nine_key": .boolean(true)], themes: themes)
    XCTAssertEqual(nine.scheme, "nineKey")
    XCTAssertNil(nine.sound)
    let shuangpin = try IOSPreferencePlan(["input.schema": .string("shuangpin"), "input.shuangpin_schema": .string("ziranma"), "input.character_set": .string("traditional")], themes: themes)
    XCTAssertEqual(shuangpin.scheme, "ziranma")
    XCTAssertEqual(shuangpin.traditional, true)
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
}

import Foundation
import XCTest

final class VoiceProviderCatalogTests: XCTestCase {
  private func fixture() throws -> Data {
    try JSONSerialization.data(withJSONObject: [
      "ok": true,
      "value": [
        "asr": [
          ["id": "synthetic", "endpoint": "https://example.invalid/unrelated",
           "models": ["second-model", "first-model"], "documentation": "https://example.invalid/guide"],
          ["id": "doubao", "endpoint": "wss://example.invalid/second", "models": [], "documentation": ""],
        ],
        "doubao_stream_endpoints": [
          ["id": "third", "endpoint": "wss://example.invalid/third", "title": "第三种接口"],
          ["id": "first", "endpoint": "wss://example.invalid/first", "title": "第一种接口"],
          ["id": "second", "endpoint": "wss://example.invalid/second", "title": "第二种接口"],
        ],
      ],
    ])
  }

  func testSyntheticCatalogOwnsOrderTitlesAndDefaultEndpoint() throws {
    let catalog = try VoiceProviderCatalog(data: fixture())
    XCTAssertEqual(catalog.asrProvider(id: "doubao")?.endpoint, "wss://example.invalid/second")
    XCTAssertEqual(catalog.doubaoStreamEndpoints.map(\.id), ["third", "first", "second"])
    XCTAssertEqual(catalog.doubaoStreamEndpoints.map(\.title), ["第三种接口", "第一种接口", "第二种接口"])
    XCTAssertEqual(catalog.doubaoStreamEndpoints[1].endpoint, "wss://example.invalid/first")
  }

  func testDefaultEndpointComesFromTheMatchingProvider() throws {
    var envelope = try XCTUnwrap(JSONSerialization.jsonObject(with: fixture()) as? [String: Any])
    var value = try XCTUnwrap(envelope["value"] as? [String: Any])
    value["asr"] = [["id": "doubao", "endpoint": "wss://example.invalid/third", "models": [], "documentation": ""]]
    envelope["value"] = value
    let catalog = try VoiceProviderCatalog(data: JSONSerialization.data(withJSONObject: envelope))
    XCTAssertEqual(catalog.asrProvider(id: "doubao")?.endpoint, "wss://example.invalid/third")
    XCTAssertEqual(catalog.doubaoStreamEndpoints.count, 3)
  }

  func testUnavailableAndMalformedCatalogsAreRejected() throws {
    for body in [
      "{}", "{\"ok\":false}",
      "{\"ok\":true,\"value\":{\"asr\":[],\"doubao_stream_endpoints\":[]}}",
      "{\"ok\":true,\"value\":{\"asr\":[{\"id\":\"doubao\",\"endpoint\":false}],\"doubao_stream_endpoints\":[]}}",
      "{\"ok\":true,\"value\":{\"asr\":[{\"id\":\"doubao\",\"endpoint\":\"wss://example.invalid\",\"models\":[],\"documentation\":\"\"}],\"doubao_stream_endpoints\":[{\"id\":\"test\",\"endpoint\":\"wss://example.invalid\",\"title\":null}]}}",
    ] {
      XCTAssertThrowsError(try VoiceProviderCatalog(data: Data(body.utf8)))
    }
  }

  func testMalformedProviderMetadataIsRejected() throws {
    for fields: [String: Any] in [
      ["models": "model", "documentation": "https://example.invalid/guide"],
      ["models": ["model", 1], "documentation": "https://example.invalid/guide"],
      ["models": [], "documentation": false],
      ["documentation": "https://example.invalid/guide"],
      ["models": []],
    ] {
      var envelope = try XCTUnwrap(JSONSerialization.jsonObject(with: fixture()) as? [String: Any])
      var value = try XCTUnwrap(envelope["value"] as? [String: Any])
      var provider = fields
      provider["id"] = "doubao"
      provider["endpoint"] = "wss://example.invalid/second"
      value["asr"] = [provider]
      envelope["value"] = value
      XCTAssertThrowsError(try VoiceProviderCatalog(data: JSONSerialization.data(withJSONObject: envelope)))
    }
  }

  func testProviderLookupRetainsMetadataAndRejectsUnknownIDs() throws {
    let catalog = try VoiceProviderCatalog(data: fixture())
    let provider = try XCTUnwrap(catalog.asrProvider(id: "synthetic"))
    XCTAssertEqual(provider.endpoint, "https://example.invalid/unrelated")
    XCTAssertEqual(provider.models, ["second-model", "first-model"])
    XCTAssertEqual(provider.documentation, "https://example.invalid/guide")
    XCTAssertEqual(catalog.asrProvider(id: "doubao")?.models, [])
    XCTAssertNil(catalog.asrProvider(id: "unknown"))
  }

  func testHTTPPresetsReadSharedMetadataAndPreserveSavedValues() throws {
    let catalog = try XCTUnwrap(VoiceProviderCatalog.shared)
    let suite = "msime-asr-catalog-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    let providers: [(VoiceProviderPreset, String)] = [
      (.everyAPI, "everyapi"), (.openAI, "openai"), (.siliconFlow, "siliconflow"),
      (.groq, "groq"), (.mistral, "mistral"), (.bailian, "bailian"),
    ]
    for (preset, id) in providers {
      let shared = try XCTUnwrap(catalog.asrProvider(id: id))
      XCTAssertEqual(preset.endpoint, shared.endpoint)
      XCTAssertEqual(preset.models, shared.models)
      XCTAssertEqual(preset.documentation?.absoluteString, shared.documentation)
      let fresh = CustomServiceConfiguration.loadVoicePreset(preset, defaults: defaults)
      XCTAssertEqual(fresh.endpoint, shared.endpoint)
      XCTAssertEqual(fresh.model, shared.models.first ?? "")
      let prefix = "service.voice.presets.\(preset.rawValue)"
      defaults.set("https://example.invalid/saved", forKey: prefix + ".endpoint")
      defaults.set("saved-custom-model", forKey: prefix + ".model")
      let saved = CustomServiceConfiguration.loadVoicePreset(preset, defaults: defaults)
      XCTAssertEqual(saved.endpoint, "https://example.invalid/saved")
      XCTAssertEqual(saved.model, "saved-custom-model")
    }
  }

  func testPlatformPresetsAndDoubaoCompatibilityStayUnchanged() {
    for provider in [VoiceProviderPreset.local, .system, .custom] {
      XCTAssertEqual(provider.endpoint, "")
      XCTAssertEqual(provider.models, [])
      XCTAssertNil(provider.documentation)
    }
    XCTAssertEqual(VoiceProviderPreset.openAI.models.first, "gpt-4o-mini-transcribe")
    XCTAssertEqual(VoiceProviderPreset.doubao.models, ["volc.seedasr.sauc.duration"])
    XCTAssertNil(VoiceProviderPreset.doubao.documentation)
  }

  func testSharedCatalogReadsTheRealCABIAndRetainsItsSnapshot() throws {
    let catalog = try XCTUnwrap(VoiceProviderCatalog.shared)
    XCTAssertFalse(try XCTUnwrap(catalog.asrProvider(id: "doubao")).endpoint.isEmpty)
    XCTAssertTrue(catalog.doubaoStreamEndpoints.contains { $0.endpoint == catalog.asrProvider(id: "doubao")?.endpoint })
    XCTAssertTrue(catalog.doubaoStreamEndpoints.allSatisfy { !$0.id.isEmpty && !$0.title.isEmpty })
    let again = try XCTUnwrap(VoiceProviderCatalog.load())
    XCTAssertEqual(catalog.doubaoStreamEndpoints, again.doubaoStreamEndpoints)
    XCTAssertEqual(catalog.asrProvider(id: "doubao")?.endpoint, again.asrProvider(id: "doubao")?.endpoint)
  }
}

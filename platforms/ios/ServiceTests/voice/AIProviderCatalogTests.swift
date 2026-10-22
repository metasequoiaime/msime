import Foundation
import XCTest

final class AIProviderCatalogTests: XCTestCase {
  func testEveryNativePresetHasSharedMetadataWithoutChangingItsStoredIdentifier() throws {
    XCTAssertEqual(AIProviderPreset.allCases.map(\.rawValue), [
      "everyAPI", "openAI", "anthropic", "gemini", "deepSeek", "qwen", "kimi", "zhipu",
      "siliconFlow", "groq", "openRouter", "custom",
    ])
    for preset in AIProviderPreset.allCases {
      let shared = try XCTUnwrap(AIProviderCatalog.provider(id: preset.rawValue.lowercased()))
      XCTAssertFalse(shared.title.isEmpty)
      XCTAssertEqual(preset.title, shared.title)
      XCTAssertEqual(preset.endpoint, shared.endpoint)
      if preset != .groq {
        XCTAssertEqual(preset.models, shared.models)
        XCTAssertEqual(preset.documentation?.absoluteString, shared.documentation.isEmpty ? nil : shared.documentation)
      }
      XCTAssertEqual(try JSONDecoder().decode(AIProviderPreset.self, from: JSONEncoder().encode(preset)), preset)
    }
    XCTAssertNil(AIProviderPreset(rawValue: "ollama"))
    XCTAssertNil(AIProviderCatalog.provider(id: "unknown"))
    XCTAssertNil(AIProviderCatalog.provider(id: "OpenAI"))
    XCTAssertNil(AIProviderCatalog.provider(id: " openai "))
  }

  func testGroqAndCustomKeepTheirNativeModelAndDocumentationPolicies() {
    XCTAssertEqual(AIProviderPreset.groq.models, ["llama-3.3-70b-versatile", "openai/gpt-oss-120b"])
    XCTAssertEqual(AIProviderPreset.groq.documentation?.absoluteString, "https://console.groq.com/docs/openai")
    XCTAssertEqual(AIProviderPreset.custom.endpoint, "")
    XCTAssertEqual(AIProviderPreset.custom.models, [])
    XCTAssertNil(AIProviderPreset.custom.documentation)
  }

  func testFreshPresetsUseTheFirstSuggestedModelAndSavedFieldsKeepPriority() throws {
    let suite = "msime-ai-catalog-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    for provider in AIProviderPreset.allCases {
      let fresh = CustomServiceConfiguration.loadPreset(provider, defaults: defaults)
      XCTAssertEqual(fresh.endpoint, provider.endpoint)
      XCTAssertEqual(fresh.model, provider.models.first ?? "")
      let prefix = "service.ai.presets.\(provider.rawValue)"
      defaults.set("https://example.invalid/saved", forKey: prefix + ".endpoint")
      defaults.set("saved-synthetic-model", forKey: prefix + ".model")
      let restored = CustomServiceConfiguration.loadPreset(provider, defaults: defaults)
      XCTAssertEqual(restored.endpoint, "https://example.invalid/saved")
      XCTAssertEqual(restored.model, "saved-synthetic-model")
    }
  }
}

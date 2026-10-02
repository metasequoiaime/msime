import XCTest

/// 「候选栏 AI 候选」: the shared document says where to send, the Keychain key is handed over only for that endpoint.
final class AICandidatePreferenceTests: XCTestCase {
  func testTurningOnWritesTheSavedConfigurationButNoKey() {
    let existing: [String: Any] = ["prompt_custom_1": "keep", "tokens": ["deepseek": "synced"]]
    let assistant = AICandidatePreference.assistant(
      existing, enabled: true, limit: 5, provider: "deepSeek",
      endpoint: " https://api.deepseek.com/chat/completions ", model: " deepseek-chat ")
    XCTAssertEqual(assistant["enabled"] as? Bool, true)
    XCTAssertEqual(assistant["provider"] as? String, "deepseek")
    XCTAssertEqual(assistant["endpoint"] as? String, "https://api.deepseek.com/chat/completions")
    XCTAssertEqual(assistant["model"] as? String, "deepseek-chat")
    XCTAssertEqual(assistant["candidate_limit"] as? Int, 5)
    XCTAssertEqual(assistant["prompt_custom_1"] as? String, "keep")
    XCTAssertEqual(assistant["tokens"] as? [String: String], ["deepseek": "synced"], "a key from elsewhere is neither copied nor erased")
    XCTAssertNil(assistant["token"])
  }

  func testTurningOffKeepsWhereItPointedAndClampsTheLimit() {
    let on = AICandidatePreference.assistant(nil, enabled: true, limit: 3, provider: "openAI",
                                             endpoint: "https://api.openai.com/v1/chat/completions", model: "m")
    let off = AICandidatePreference.assistant(on, enabled: false, limit: 40, provider: "", endpoint: "", model: "")
    XCTAssertEqual(off["enabled"] as? Bool, false)
    XCTAssertEqual(off["endpoint"] as? String, "https://api.openai.com/v1/chat/completions")
    XCTAssertEqual(off["candidate_limit"] as? Int, 10)
    XCTAssertFalse(AICandidatePreference.isEnabled(["ai_assistant": off]))
    XCTAssertEqual(AICandidatePreference.limit(["ai_assistant": ["candidate_limit": 0]]), AICandidatePreference.defaultLimit)
    XCTAssertEqual(AICandidatePreference.limit(nil), 3)
  }

  func testTheKeyIsOnlyHandedOverForTheDocumentsOwnEndpoint() {
    let endpoint = "https://api.deepseek.com/chat/completions"
    let preferences: [String: Any] = ["ai_assistant": ["enabled": true, "endpoint": endpoint]]
    XCTAssertEqual(AICandidatePreference.credentialEndpoint(preferences, configuredEndpoint: endpoint + " "), endpoint)
    XCTAssertNil(AICandidatePreference.credentialEndpoint(preferences, configuredEndpoint: "https://evil.example/v1"))
    XCTAssertNil(AICandidatePreference.credentialEndpoint(preferences, configuredEndpoint: nil))
    XCTAssertNil(AICandidatePreference.credentialEndpoint(["ai_assistant": ["enabled": false, "endpoint": endpoint]],
                                                          configuredEndpoint: endpoint))
    XCTAssertNil(AICandidatePreference.credentialEndpoint(nil, configuredEndpoint: endpoint))
  }

  func testPromptSlotsReadAndWriteTheDesktopFields() {
    XCTAssertEqual(AICandidatePreference.promptID(nil), "custom_1")
    XCTAssertEqual(AICandidatePreference.promptID(["ai_assistant": ["prompt_id": "custom_9"]]), "custom_1", "an unknown slot reads as the first")
    let saved: [String: Any] = ["ai_assistant": ["prompt_custom_1": "", "prompt_custom_2": "two"]]
    XCTAssertEqual(AICandidatePreference.prompt(saved, slot: "custom_1"), "", "an empty slot means the built-in prompt")
    XCTAssertEqual(AICandidatePreference.prompt(saved, slot: "custom_2"), "two")
    XCTAssertEqual(AICandidatePreference.prompt(saved, slot: "custom_3"), "")

    let second = AICandidatePreference.assistant(["enabled": true, "prompt_custom_1": "one"], promptSlot: "custom_2", text: "two")
    XCTAssertEqual(second["prompt_id"] as? String, "custom_2")
    XCTAssertEqual(second["prompt_custom_2"] as? String, "two")
    XCTAssertEqual(second["prompt_custom_1"] as? String, "one", "other slots are kept")
    XCTAssertEqual(second["enabled"] as? Bool, true)
    XCTAssertEqual(AICandidatePreference.promptID(["ai_assistant": second]), "custom_2")

    let blank = AICandidatePreference.assistant(second, promptSlot: "custom_2", text: " \n ")
    XCTAssertEqual(blank["prompt_custom_2"] as? String, "", "whitespace is stored empty so the built-in prompt applies")
    let ignored = AICandidatePreference.assistant(second, promptSlot: "prompt", text: "x")
    XCTAssertEqual(ignored["prompt_id"] as? String, "custom_2", "only the three slots can be written")
  }
}

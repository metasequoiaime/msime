import XCTest

final class VoicePolishTests: XCTestCase {
  func testPresetsComeFromTheSharedPromptHeader() {
    var settings = VoicePolishSettings()
    XCTAssertTrue(settings.systemPrompt.hasPrefix("你是语音转写整理助手。"))
    settings.promptID = "faithful"
    XCTAssertTrue(settings.systemPrompt.hasPrefix("你是语音转写校对助手。"))
    settings.promptID = "zh2en"
    XCTAssertTrue(settings.systemPrompt.hasPrefix("你是中文口述英译助手。"))
    settings.promptID = "casual"
    XCTAssertTrue(settings.systemPrompt.hasPrefix("你是口语整理助手。"))
  }

  func testCustomSlotsFallBackTheWayTheDesktopDoes() {
    var settings = VoicePolishSettings()
    let cleanup = settings.systemPrompt
    settings.promptID = "custom_2"
    XCTAssertEqual(settings.systemPrompt, cleanup)
    settings.customPrompts[1] = "只改错别字"
    XCTAssertEqual(settings.systemPrompt, "只改错别字")

    settings.promptID = "custom_1"
    XCTAssertEqual(settings.systemPrompt, cleanup)
    settings.customPrompts[0] = "第一槽"
    XCTAssertEqual(settings.systemPrompt, "第一槽")
  }

  func testReadsAndWritesTheSharedVoiceFieldsOnly() {
    var document: [String: Any] = ["voice_input": [
      "polish_enabled": true, "polish_prompt_id": "zh2en", "polish_prompt_custom_3": "三",
      "language": "en-US", "sound_enabled": false, "end_sound": false, "asr_provider": "doubao",
      "stream_inline_preedit": false, "mute_system_audio": true,
    ]]
    var settings = VoicePolishSettings(document)
    XCTAssertTrue(settings.polishEnabled)
    XCTAssertEqual(settings.promptID, "zh2en")
    XCTAssertEqual(settings.customPrompts, ["", "", "三"])
    XCTAssertEqual(settings.language, "en")
    XCTAssertFalse(settings.soundEnabled)
    XCTAssertTrue(settings.startSound)
    XCTAssertFalse(settings.endSound)
    XCTAssertFalse(settings.streamLive)
    XCTAssertTrue(settings.muteOthers)

    settings.promptID = "custom_1"
    settings.customPrompts[0] = "  \n"
    settings.polishEnabled = false
    settings.startSound = false
    settings.endSound = true
    settings.streamLive = true
    settings.muteOthers = false
    settings.write(into: &document)
    let voice = document["voice_input"] as? [String: Any]
    XCTAssertEqual(voice?["polish_text"] as? Bool, false)
    XCTAssertEqual(voice?["polish_enabled"] as? Bool, false)
    XCTAssertEqual(voice?["polish_prompt_id"] as? String, "custom_1")
    XCTAssertEqual(voice?["polish_prompt_custom_1"] as? String, "")
    XCTAssertEqual(voice?["polish_prompt_custom_3"] as? String, "三")
    XCTAssertEqual(voice?["asr_provider"] as? String, "doubao")
    XCTAssertEqual(voice?["start_sound"] as? Bool, false)
    XCTAssertEqual(voice?["end_sound"] as? Bool, true)
    XCTAssertEqual(voice?["sound_enabled"] as? Bool, false)
    XCTAssertEqual(voice?["stream_inline_preedit"] as? Bool, true)
    XCTAssertEqual(voice?["mute_system_audio"] as? Bool, false)
  }

  func testUnknownValuesFallBackToTheDefaults() {
    let settings = VoicePolishSettings(["voice_input": ["polish_prompt_id": "custom", "language": "fr"]])
    XCTAssertEqual(settings.promptID, "cleanup")
    XCTAssertEqual(settings.language, "zh-cn")
    XCTAssertFalse(settings.polishEnabled)
    XCTAssertTrue(settings.streamLive)
    XCTAssertFalse(settings.muteOthers)
  }

  func testTranscriptionLanguageFollowsTheSharedNormalization() {
    var settings = VoicePolishSettings()
    XCTAssertEqual(settings.transcriptionLanguage(for: .openAI), "zh")
    XCTAssertNil(settings.transcriptionLanguage(for: .siliconFlow))
    XCTAssertNil(settings.transcriptionLanguage(for: .doubao))
    settings.language = "en"
    XCTAssertEqual(settings.transcriptionLanguage(for: .groq), "en")
    settings.language = "auto"
    XCTAssertNil(settings.transcriptionLanguage(for: .openAI))
  }

  func testLanguageIdsRoundTripAsAndroidWritesThem() {
    let legacy = VoicePolishSettings(["voice_input": ["language": "en-us"]])
    XCTAssertEqual(legacy.language, "en")
    var document: [String: Any] = [:]
    legacy.write(into: &document)
    XCTAssertEqual((document["voice_input"] as? [String: Any])?["language"] as? String, "en")

    let cantonese = VoicePolishSettings(["voice_input": ["language": "yue"]])
    XCTAssertEqual(cantonese.language, "yue")
    XCTAssertEqual(cantonese.transcriptionLanguage(for: .openAI), "yue")

    let auto = VoicePolishSettings(["voice_input": ["language": "auto"]])
    XCTAssertEqual(auto.language, "auto")
    XCTAssertNil(auto.transcriptionLanguage(for: .groq))
    XCTAssertEqual(VoicePolishSettings.languages.map(\.id), ["zh-cn", "yue", "en", "auto"])
  }

  func testTranscriptionBodyCarriesTheLanguageOnlyWhenGiven() throws {
    let with = try AppServicesBridge.transcriptionBody(Data([1, 2]), model: "whisper-1", language: "zh")
    let without = try AppServicesBridge.transcriptionBody(Data([1, 2]), model: "whisper-1")
    let withText = String(decoding: try XCTUnwrap(with["body"] as? Data), as: UTF8.self)
    let withoutText = String(decoding: try XCTUnwrap(without["body"] as? Data), as: UTF8.self)
    XCTAssertTrue(withText.contains("name=\"language\"\r\n\r\nzh\r\n"))
    XCTAssertFalse(withoutText.contains("name=\"language\""))
  }

  func testTheTranscriptIsFramedAsData() {
    XCTAssertEqual(VoicePolishSettings.userMessage("你好"), "<asr_text>\n你好\n</asr_text>")
  }

  func testThePolishServiceFollowsAISettingsUntilOneIsSavedForIt() throws {
    let suite = "msime-polish-service-tests-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    var keys: [String: String] = [:]
    let read: (String, URL) throws -> String = { keys["\($0)|\($1.host ?? "")"] ?? "" }

    XCTAssertFalse(VoicePolishService.load(defaults: defaults).separate)
    XCTAssertThrowsError(try VoicePolishService.resolved(defaults: defaults, readToken: read)) {
      XCTAssertTrue($0.localizedDescription.contains("AI 设置"))
    }
    var ai = CustomServiceConfiguration.loadPreset(.openAI, defaults: defaults)
    try ai.save(.ai, token: "", defaults: defaults)
    keys["ai|api.openai.com"] = "ai-key"
    var resolved = try VoicePolishService.resolved(defaults: defaults, readToken: read)
    XCTAssertEqual(resolved.configuration.endpoint, AIProviderPreset.openAI.endpoint)
    XCTAssertEqual(resolved.token, "ai-key")

    var service = VoicePolishService.load(defaults: defaults)
    service.separate = true
    service.select(.groq)
    XCTAssertEqual(service.endpoint, "https://api.groq.com/openai/v1/chat/completions")
    XCTAssertEqual(service.model, "llama-3.3-70b-versatile")
    service.endpoint = "http://api.groq.com/openai/v1/chat/completions"
    XCTAssertThrowsError(try service.save(token: "x", defaults: defaults) { _, _ in XCTFail("wrote a key for a bad endpoint") })
    XCTAssertFalse(VoicePolishService.load(defaults: defaults).separate)

    service.select(.groq)
    service.model = "  openai/gpt-oss-120b "
    var written: [String] = []
    try service.save(token: "groq-key", defaults: defaults) { token, url in
      written.append("\(token)|\(url.host ?? "")")
      keys["\(ServiceTokenStore.polishScope)|\(url.host ?? "")"] = token
    }
    XCTAssertEqual(written, ["groq-key|api.groq.com"])
    let loaded = VoicePolishService.load(defaults: defaults)
    XCTAssertTrue(loaded.separate)
    XCTAssertEqual(loaded.provider, .groq)
    XCTAssertEqual(loaded.model, "openai/gpt-oss-120b")
    resolved = try VoicePolishService.resolved(defaults: defaults, readToken: read)
    XCTAssertEqual(resolved.configuration.endpoint, AIProviderPreset.groq.endpoint)
    XCTAssertEqual(resolved.configuration.model, "openai/gpt-oss-120b")
    XCTAssertEqual(resolved.token, "groq-key")

    // An empty key keeps the saved one; turning the switch off goes back to AI 设置 but keeps the separate service for later.
    try loaded.save(token: "", defaults: defaults) { _, _ in XCTFail("an empty key overwrote the saved one") }
    var off = loaded
    off.separate = false
    try off.save(token: "", defaults: defaults)
    XCTAssertEqual(try VoicePolishService.resolved(defaults: defaults, readToken: read).token, "ai-key")
    XCTAssertEqual(VoicePolishService.load(defaults: defaults).provider, .groq)
  }
}

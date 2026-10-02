import Foundation

@_silgen_name("msime_ios_voice_polish_prompt")
private func msimeIOSVoicePolishPrompt(_ id: UnsafePointer<CChar>?, _ custom1: UnsafePointer<CChar>?,
                                       _ custom2: UnsafePointer<CChar>?,
                                       _ custom3: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

/// The shared document's `voice_input` fields this app acts on: recognition language, the start and end cue, and the polish pass after recognition.
///
/// The desktop configures a separate polish provider and key in the document. On iOS the service lives in `VoicePolishService`, with its key in the Keychain, so only the preset and the three custom prompts come from the document.
struct VoicePolishSettings: Equatable {
  static let presets: [(id: String, title: String)] = [
    ("cleanup", "精炼整理"), ("faithful", "忠实校对"), ("zh2en", "中翻英"), ("casual", "口语整理"),
    ("custom_1", "自定义一"), ("custom_2", "自定义二"), ("custom_3", "自定义三"),
  ]
  static let languages: [(id: String, title: String)] = [
    ("zh-cn", "中文（简体）"), ("en-us", "English"), ("auto", "自动识别"),
  ]
  static let customSlots = ["custom_1", "custom_2", "custom_3"]

  var polishEnabled = false
  var promptID = "cleanup"
  var customPrompts = ["", "", ""]
  var language = "zh-cn"
  var soundEnabled = true
  /// The desktop's per-cue switches under the master switch; a cue plays when both are on.
  var startSound = true
  var endSound = true
  /// The desktop's `stream_inline_preedit`: Doubao recognizes while the user speaks. Windows writes the partial text into the document; iOS records in the app, so the partial text shows on the page instead.
  var streamLive = true
  /// The desktop's `mute_system_audio`. iOS cannot mute other apps, but a recording session can take the audio from them, which stops their playback; off, they keep playing under the recording. Off unless asked for, as the shared default is on this host.
  var muteOthers = false

  init() {}

  init(_ preferences: [String: Any]?) {
    let voice = preferences?["voice_input"] as? [String: Any] ?? [:]
    // The shared settings page reads either flag as on and writes both.
    polishEnabled = voice["polish_text"] as? Bool == true || voice["polish_enabled"] as? Bool == true
    let id = voice["polish_prompt_id"] as? String ?? ""
    promptID = Self.presets.contains { $0.id == id } ? id : "cleanup"
    customPrompts = Self.customSlots.map { voice["polish_prompt_\($0)"] as? String ?? "" }
    let language = (voice["language"] as? String ?? "").lowercased()
    self.language = Self.languages.contains { $0.id == language } ? language : "zh-cn"
    soundEnabled = voice["sound_enabled"] as? Bool ?? true
    startSound = voice["start_sound"] as? Bool ?? true
    endSound = voice["end_sound"] as? Bool ?? true
    streamLive = voice["stream_inline_preedit"] as? Bool ?? true
    muteOthers = voice["mute_system_audio"] as? Bool ?? false
  }

  /// Writes the fields this page owns and keeps the rest of `voice_input` (desktop providers, hotkeys, Doubao settings) as it was.
  func write(into preferences: inout [String: Any]) {
    var voice = preferences["voice_input"] as? [String: Any] ?? [:]
    voice["polish_text"] = polishEnabled
    voice["polish_enabled"] = polishEnabled
    voice["polish_prompt_id"] = promptID
    for (slot, prompt) in zip(Self.customSlots, customPrompts) {
      voice["polish_prompt_\(slot)"] = prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "" : prompt
    }
    voice["language"] = language
    voice["sound_enabled"] = soundEnabled
    voice["start_sound"] = startSound
    voice["end_sound"] = endSound
    voice["stream_inline_preedit"] = streamLive
    voice["mute_system_audio"] = muteOthers
    preferences["voice_input"] = voice
  }

  /// The index of the selected custom slot, or nil for a built-in preset.
  var customSlot: Int? { Self.customSlots.firstIndex(of: promptID) }

  /// The system prompt the polish request carries, resolved by the shared header the desktop hosts use.
  var systemPrompt: String {
    let values = [promptID] + customPrompts
    let pointers = values.map { strdup($0) }
    defer { pointers.forEach { free($0) } }
    guard let result = msimeIOSVoicePolishPrompt(pointers[0], pointers[1], pointers[2], pointers[3])
    else { return "" }
    defer { free(result) }
    return String(cString: result)
  }

  /// The recognized text framed the way every host frames it, so the model treats it as data rather than instructions.
  static func userMessage(_ transcript: String) -> String { "<asr_text>\n\(transcript)\n</asr_text>" }

  /// The `language` field of a transcription request: the language part of the tag, or nothing to let the service detect it. SiliconFlow's SenseVoice rejects the field, so it never gets one; Doubao's streaming protocol does not take one either.
  func transcriptionLanguage(for provider: VoiceProviderPreset) -> String? {
    guard provider != .siliconFlow, provider != .doubao, language != "auto" else { return nil }
    return language.split(whereSeparator: { $0 == "-" || $0 == "_" }).first.map(String.init)
  }
}

/// The chat service the polish pass after recognition goes to. The desktop gives the pass its own provider, endpoint, model and key (`voice_input.polish_*`); on iOS it follows 「AI 设置」 unless the user saves one of its own here. The separate key stays in the Keychain under its own scope, apart from the AI key even on the same host, so it never reaches the synced document.
struct VoicePolishService: Equatable {
  var separate = false
  var provider: AIProviderPreset = .deepSeek
  var endpoint = AIProviderPreset.deepSeek.endpoint
  var model = AIProviderPreset.deepSeek.models.first ?? ""

  static func load(defaults: UserDefaults = .standard) -> Self {
    var result = Self()
    result.separate = defaults.bool(forKey: "service.polish.separate")
    if let provider = AIProviderPreset(rawValue: defaults.string(forKey: "service.polish.provider") ?? "") {
      result.select(provider)
    }
    result.endpoint = defaults.string(forKey: "service.polish.endpoint") ?? result.endpoint
    result.model = defaults.string(forKey: "service.polish.model") ?? result.model
    return result
  }

  /// Switches the preset and fills in its endpoint and first model.
  mutating func select(_ provider: AIProviderPreset) {
    self.provider = provider
    endpoint = provider.endpoint
    model = provider.models.first ?? ""
  }

  var configuration: CustomServiceConfiguration {
    var configuration = CustomServiceConfiguration()
    configuration.provider = provider
    configuration.endpoint = endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
    configuration.model = model.trimmingCharacters(in: .whitespacesAndNewlines)
    return configuration
  }

  /// Saves the choice. A separate service needs an HTTPS endpoint and a model; an empty key keeps the one already saved for that host.
  func save(token: String, defaults: UserDefaults = .standard,
            writeToken: (String, URL) throws -> Void = { try ServiceTokenStore.write($0, scope: ServiceTokenStore.polishScope, url: $1) }) throws {
    if separate {
      let url = try configuration.validatedURL()
      if !token.isEmpty { try writeToken(token, url) }
      defaults.set(provider.rawValue, forKey: "service.polish.provider")
      defaults.set(url.absoluteString, forKey: "service.polish.endpoint")
      defaults.set(configuration.model, forKey: "service.polish.model")
    }
    defaults.set(separate, forKey: "service.polish.separate")
  }

  /// The configuration and key a polish request goes out with: the saved separate service, or 「AI 设置」.
  static func resolved(defaults: UserDefaults = .standard,
                       readToken: (String, URL) throws -> String = { try ServiceTokenStore.read(scope: $0, url: $1) })
    throws -> (configuration: CustomServiceConfiguration, token: String) {
    let service = load(defaults: defaults)
    if service.separate {
      let configuration = service.configuration
      guard let url = try? configuration.validatedURL() else { throw ServiceFailure(message: "请先保存润色服务。") }
      return (configuration, try readToken(ServiceTokenStore.polishScope, url))
    }
    let configuration = CustomServiceConfiguration.load(.ai, defaults: defaults)
    guard let url = try? configuration.validatedURL() else { throw ServiceFailure(message: "请先在“AI 设置”里保存服务。") }
    return (configuration, try readToken(CustomServiceKind.ai.rawValue, url))
  }
}

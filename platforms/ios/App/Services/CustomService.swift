import Foundation
import Security

enum CustomServiceKind: String {
  case ai, voice
  var title: String { self == .ai ? "AI 设置" : "语音设置" }
  var example: String {
    self == .ai ? "https://你的服务/v1/chat/completions" : "https://你的服务/v1/audio/transcriptions"
  }
}

// Official endpoint/model documentation, checked 2026-09-07. These presets use the
// providers' Chat Completions compatibility APIs; request codecs remain in Engine.
enum AIProviderPreset: String, CaseIterable, Codable, Sendable {
  case everyAPI, openAI, anthropic, gemini, deepSeek, qwen, kimi, zhipu, siliconFlow, groq, openRouter, custom

  var title: String {
    switch self {
    case .everyAPI: "EveryAPI"
    case .openAI: "OpenAI"
    case .anthropic: "Anthropic · Claude"
    case .gemini: "Google · Gemini"
    case .deepSeek: "DeepSeek"
    case .qwen: "通义千问 · 阿里云百炼"
    case .kimi: "Kimi · 月之暗面"
    case .zhipu: "智谱 · GLM"
    case .siliconFlow: "硅基流动"
    case .groq: "Groq"
    case .openRouter: "OpenRouter"
    case .custom: "自定义"
    }
  }
  var endpoint: String {
    switch self {
    case .everyAPI: "https://api.everyapi.ai/v1/chat/completions"
    case .openAI: "https://api.openai.com/v1/chat/completions"
    case .anthropic: "https://api.anthropic.com/v1/chat/completions"
    case .gemini: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
    case .deepSeek: "https://api.deepseek.com/chat/completions"
    case .qwen: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
    case .kimi: "https://api.moonshot.cn/v1/chat/completions"
    case .zhipu: "https://open.bigmodel.cn/api/paas/v4/chat/completions"
    case .siliconFlow: "https://api.siliconflow.cn/v1/chat/completions"
    case .groq: "https://api.groq.com/openai/v1/chat/completions"
    case .openRouter: "https://openrouter.ai/api/v1/chat/completions"
    case .custom: ""
    }
  }
  var models: [String] {
    switch self {
    case .everyAPI: ["deepseek-v4-flash", "deepseek-v4-pro", "claude-sonnet-5", "glm-5.3-flash"]
    case .openAI: ["gpt-4.1-mini"]
    case .anthropic: ["claude-sonnet-4-6", "claude-opus-5"]
    case .gemini: ["gemini-3.8-flash", "gemini-2.5-flash"]
    case .deepSeek: ["deepseek-v4-flash", "deepseek-v4-pro"]
    case .qwen: ["qwen-plus"]
    case .kimi: ["kimi-k2.6", "kimi-k2.5"]
    case .zhipu: ["glm-4.7", "glm-4.7-flashx"]
    case .siliconFlow: ["Qwen/Qwen3.6-27B"]
    case .groq: ["llama-3.3-70b-versatile", "openai/gpt-oss-120b"]
    case .openRouter: ["openrouter/auto"]
    case .custom: []
    }
  }
  var documentation: URL? {
    let address: String
    switch self {
    case .everyAPI: address = "https://everyapi.ai/models"
    case .openAI: address = "https://developers.openai.com/api/docs/models/gpt-4.1-mini"
    case .anthropic: address = "https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk"
    case .gemini: address = "https://ai.google.dev/gemini-api/docs/openai"
    case .deepSeek: address = "https://api-docs.deepseek.com/"
    case .qwen: address = "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope"
    case .kimi: address = "https://platform.kimi.com/docs/api/chat"
    case .zhipu: address = "https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7"
    case .siliconFlow: address = "https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation"
    case .groq: address = "https://console.groq.com/docs/openai"
    case .openRouter: address = "https://openrouter.ai/docs/quickstart"
    case .custom: return nil
    }
    return URL(string: address)
  }
}

// File transcription presets use Engine's multipart file/model codec.
// Official provider documentation checked 2026-09-07.
// `local` and `system` recognize on the phone and carry no endpoint, model or key; their raw values are the shared `voice_input.provider` names.
enum VoiceProviderPreset: String, CaseIterable, Codable, Sendable {
  case local, system, everyAPI, doubao, openAI, siliconFlow, groq, mistral, custom

  /// Recognized on the phone: no endpoint, model or key, and audio never leaves the device (for `system`, whenever the device can recognize the language by itself).
  var isOnDevice: Bool { self == .local || self == .system }

  var title: String {
    switch self {
    case .local: "本地模型 · 离线识别"
    case .system: "系统语音识别"
    case .everyAPI: "EveryAPI"
    case .doubao: "豆包 · WebSocket"
    case .openAI: "OpenAI"
    case .siliconFlow: "硅基流动 · SenseVoice"
    case .groq: "Groq · Whisper"
    case .mistral: "Mistral · Voxtral"
    case .custom: "自定义"
    }
  }
  var endpoint: String {
    switch self {
    case .local, .system: ""
    case .everyAPI: "https://api.everyapi.ai/v1/audio/transcriptions"
    case .doubao: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async"
    case .openAI: "https://api.openai.com/v1/audio/transcriptions"
    case .siliconFlow: "https://api.siliconflow.cn/v1/audio/transcriptions"
    case .groq: "https://api.groq.com/openai/v1/audio/transcriptions"
    case .mistral: "https://api.mistral.ai/v1/audio/transcriptions"
    case .custom: ""
    }
  }
  /// Doubao's two streaming recognizers, as on Windows and the desktop settings page: nostream uploads while the user speaks and answers with whole sentences, which Volcengine recommends for input methods; async answers incrementally.
  static let doubaoStreamEndpoints: [(endpoint: String, title: String)] = [
    ("wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream", "整句流式（准确率更高）"),
    ("wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async", "双向流式（增量结果）"),
  ]
  var models: [String] {
    switch self {
    case .local, .system: []
    case .everyAPI: ["openai/whisper-large-v3-turbo", "volc.seedasr.sauc.duration"]
    case .doubao: ["volc.seedasr.sauc.duration"]
    case .openAI: ["gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"]
    case .siliconFlow: ["FunAudioLLM/SenseVoiceSmall"]
    case .groq: ["whisper-large-v3-turbo", "whisper-large-v3"]
    case .mistral: ["voxtral-mini-latest"]
    case .custom: []
    }
  }
  var documentation: URL? {
    let address: String
    switch self {
    case .local, .system: return nil
    case .everyAPI: address = "https://everyapi.ai/models"
    case .doubao: return nil
    case .openAI: address = "https://developers.openai.com/api/docs/guides/speech-to-text"
    case .siliconFlow: address = "https://siliconflow.readme.io/reference/createaudiotranscriptions"
    case .groq: address = "https://console.groq.com/docs/speech-to-text"
    case .mistral: address = "https://docs.mistral.ai/studio/audio/speech_to_text/offline_transcription"
    case .custom: return nil
    }
    return URL(string: address)
  }
}

struct ServiceFailure: LocalizedError {
  let message: String
  var errorDescription: String? { message }
}

struct CustomServiceConfiguration: Codable, Sendable, Equatable {
  private static let maximumPromptBytes = 32 * 1024
  private static let maximumDoubaoFieldBytes = 8 * 1024
  var provider: AIProviderPreset = .custom
  var voiceProvider: VoiceProviderPreset = .custom
  var voiceAppKey = ""
  var voiceResourceID = ""
  var doubaoEnableITN = true
  var doubaoEnablePunctuation = true
  var doubaoEnableDDC = false
  var doubaoBoostingTableID = ""
  var endpoint = ""
  var model = ""
  var prompt = "请润色以下文字，保持原意，只返回修改后的文字。"

  static func load(_ kind: CustomServiceKind, defaults: UserDefaults = .standard) -> Self {
    var result = Self()
    if kind == .ai {
      result.provider = AIProviderPreset(rawValue: defaults.string(forKey: "service.ai.provider") ?? "") ?? .custom
    }
    if kind == .voice {
      result.voiceProvider = VoiceProviderPreset(rawValue: defaults.string(forKey: "service.voice.provider") ?? "") ?? .custom
      result.voiceAppKey = defaults.string(forKey: "service.voice.app_key") ?? ""
      result.voiceResourceID = defaults.string(forKey: "service.voice.resource_id") ?? ""
      result.doubaoEnableITN = defaults.object(forKey: "service.voice.enable_itn") as? Bool ?? true
      result.doubaoEnablePunctuation = defaults.object(forKey: "service.voice.enable_punc") as? Bool ?? true
      result.doubaoEnableDDC = defaults.bool(forKey: "service.voice.enable_ddc")
      result.doubaoBoostingTableID = defaults.string(forKey: "service.voice.boosting_table_id") ?? ""
    }
    result.endpoint = defaults.string(forKey: "service.\(kind.rawValue).endpoint") ?? ""
    result.model = defaults.string(forKey: "service.\(kind.rawValue).model") ?? ""
    result.prompt = defaults.string(forKey: "service.\(kind.rawValue).prompt") ?? result.prompt
    return result
  }

  static func loadPreset(_ provider: AIProviderPreset, defaults: UserDefaults = .standard) -> Self {
    let prefix = "service.ai.presets.\(provider.rawValue)"
    var result = Self()
    result.provider = provider
    result.endpoint = defaults.string(forKey: prefix + ".endpoint") ?? provider.endpoint
    result.model = defaults.string(forKey: prefix + ".model") ?? provider.models.first ?? ""
    result.prompt = defaults.string(forKey: prefix + ".prompt") ?? result.prompt
    return result
  }

  static func loadVoicePreset(_ provider: VoiceProviderPreset, defaults: UserDefaults = .standard) -> Self {
    let prefix = "service.voice.presets.\(provider.rawValue)"
    var result = Self()
    result.voiceProvider = provider
    result.endpoint = defaults.string(forKey: prefix + ".endpoint") ?? provider.endpoint
    result.model = defaults.string(forKey: prefix + ".model") ?? provider.models.first ?? ""
    return result
  }

  private func storeVoicePreset(in defaults: UserDefaults) {
    let prefix = "service.voice.presets.\(voiceProvider.rawValue)"
    defaults.set(endpoint, forKey: prefix + ".endpoint")
    defaults.set(model, forKey: prefix + ".model")
  }

  private func storePreset(in defaults: UserDefaults) {
    let prefix = "service.ai.presets.\(provider.rawValue)"
    defaults.set(endpoint, forKey: prefix + ".endpoint")
    defaults.set(model, forKey: prefix + ".model")
    defaults.set(prompt, forKey: prefix + ".prompt")
  }

  static func validatedEndpoint(_ value: String, allowWebSocket: Bool = false,
                                maximumBytes: Int? = nil) -> URL? {
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard maximumBytes.map({ trimmed.utf8.count <= $0 }) ?? true,
          !trimmed.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
          let url = URL(string: trimmed),
          (url.scheme?.lowercased() == "https" || (allowWebSocket && url.scheme?.lowercased() == "wss")),
          let host = url.host, !host.isEmpty,
          url.user == nil, url.password == nil, url.fragment == nil else { return nil }
    return url
  }

  private static func validDoubaoField(_ value: String) -> Bool {
    value.utf8.count <= maximumDoubaoFieldBytes
      && !value.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
  }

  private func validateDoubaoFields() throws {
    guard Self.validDoubaoField(voiceAppKey), Self.validDoubaoField(voiceResourceID),
          Self.validDoubaoField(doubaoBoostingTableID) else {
      throw ServiceFailure(message: "豆包服务字段过长或包含非法字符。")
    }
  }

  func validatedURL(requiresModel: Bool = true, allowWebSocket: Bool = false) throws -> URL {
    guard let url = Self.validatedEndpoint(endpoint, allowWebSocket: allowWebSocket, maximumBytes: 2_048),
      (!requiresModel || (!model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.utf8.count <= 256))
    else { throw ServiceFailure(message: "请填写完整的 HTTPS 接口地址和模型名称。") }
    return url
  }

  func save(_ kind: CustomServiceKind, token: String, defaults: UserDefaults = .standard) throws {
    guard prompt.utf8.count <= Self.maximumPromptBytes else {
      throw ServiceFailure(message: "润色提示词过长。")
    }
    if kind == .voice && voiceProvider.isOnDevice {
      // Only the choice is saved; the cloud endpoint, model and key stay as they were, and each cloud service's own preset, for switching back.
      defaults.set(voiceProvider.rawValue, forKey: "service.voice.provider")
      return
    }
    if kind == .voice && voiceProvider == .doubao { try validateDoubaoFields() }
    let url = try validatedURL(allowWebSocket: kind == .voice && voiceProvider == .doubao)
    if !token.isEmpty { try ServiceTokenStore.write(token, kind: kind, url: url) }
    if kind == .ai {
      storePreset(in: defaults)
      defaults.set(provider.rawValue, forKey: "service.ai.provider")
    }
    if kind == .voice {
      storeVoicePreset(in: defaults)
      defaults.set(voiceProvider.rawValue, forKey: "service.voice.provider")
      defaults.set(voiceAppKey, forKey: "service.voice.app_key")
      defaults.set(voiceResourceID, forKey: "service.voice.resource_id")
      defaults.set(doubaoEnableITN, forKey: "service.voice.enable_itn")
      defaults.set(doubaoEnablePunctuation, forKey: "service.voice.enable_punc")
      defaults.set(doubaoEnableDDC, forKey: "service.voice.enable_ddc")
      defaults.set(doubaoBoostingTableID, forKey: "service.voice.boosting_table_id")
    }
    defaults.set(url.absoluteString, forKey: "service.\(kind.rawValue).endpoint")
    defaults.set(model.trimmingCharacters(in: .whitespacesAndNewlines), forKey: "service.\(kind.rawValue).model")
    defaults.set(prompt, forKey: "service.\(kind.rawValue).prompt")
  }
}

extension CustomServiceConfiguration {
  /// Build the Doubao WebSocket handshake from persisted voice settings and
  /// the separately stored access key. Credentials are returned only to the
  /// caller and are never logged or serialized into diagnostics.
  func doubaoHandshake(accessKey: String, requestID: String = UUID().uuidString) throws -> DoubaoHandshake {
    try validateDoubaoFields()
    guard Self.validDoubaoField(accessKey) else {
      throw ServiceFailure(message: "豆包服务字段过长或包含非法字符。")
    }
    return try DoubaoHandshake(
      appKey: voiceAppKey,
      accessKey: accessKey,
      resourceID: voiceResourceID,
      requestID: requestID
    )
  }
}

enum ServiceTokenStore {
  /// The voice polish pass's own key, when it does not follow 「AI 设置」; kept apart from the AI key even on the same host.
  static let polishScope = "polish"

  private static func query(_ scope: String, _ url: URL) -> [String: Any] {
    [kSecClass as String: kSecClassGenericPassword,
     kSecAttrService as String: "app.msime.ios.custom-services",
     kSecAttrAccount as String: "\(scope)|\(url.scheme ?? "")://\(url.host?.lowercased() ?? ""):\(url.port ?? 443)"]
  }
  static func read(_ kind: CustomServiceKind, url: URL) throws -> String { try read(scope: kind.rawValue, url: url) }
  static func write(_ token: String, kind: CustomServiceKind, url: URL) throws {
    try write(token, scope: kind.rawValue, url: url)
  }
  static func read(scope: String, url: URL) throws -> String {
    var query = query(scope, url)
    query[kSecReturnData as String] = true
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return "" }
    guard status == errSecSuccess, let data = result as? Data,
      let text = String(data: data, encoding: .utf8)
    else { throw ServiceFailure(message: "无法读取钥匙串，请解锁设备后重试。") }
    return text
  }
  static func write(_ token: String, scope: String, url: URL) throws {
    let query = query(scope, url)
    var status: OSStatus
    if token.isEmpty {
      status = SecItemDelete(query as CFDictionary)
      if status == errSecItemNotFound { return }
    } else {
      let attributes = [kSecValueData as String: Data(token.utf8)]
      status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
      if status == errSecItemNotFound {
        var item = query.merging(attributes) { _, new in new }
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        status = SecItemAdd(item as CFDictionary, nil)
      }
    }
    guard status == errSecSuccess else { throw ServiceFailure(message: "无法保存钥匙串，请解锁设备后重试。") }
  }
}

final class NoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
  func urlSession(_ session: URLSession, task: URLSessionTask,
                  willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
                  completionHandler: @escaping (URLRequest?) -> Void) {
    completionHandler(nil)
  }
}

enum CustomServiceClient {
  static func request(kind: CustomServiceKind, configuration: CustomServiceConfiguration,
                      text: String = "", wav: Data? = nil, pcm: Data? = nil, token: String,
                      language: String? = nil, generation: UInt64 = 1, doubaoClient: DoubaoVoiceClient? = nil,
                      sessionConfiguration: URLSessionConfiguration = .ephemeral) async throws -> String {
    if kind == .voice && configuration.voiceProvider == .doubao {
      guard let doubaoClient else { throw ServiceFailure(message: "豆包语音需要原生 host codec。") }
      guard let pcm, !pcm.isEmpty else { throw ServiceFailure(message: "请先录音。") }
      let url = try configuration.validatedURL(requiresModel: false, allowWebSocket: true)
      let handshake = try configuration.doubaoHandshake(accessKey: token)
      do {
        return try await doubaoClient.transcribe(endpoint: url, handshake: handshake,
                                                  generation: generation, pcm: pcm)
      } catch DoubaoVoiceClient.Failure.emptyTranscript {
        throw ServiceFailure(message: "豆包未返回可用的语音文本。")
      }
    }
    let request = try makeRequest(kind: kind, configuration: configuration, prompt: configuration.prompt,
                                  text: text, wav: wav, token: token, language: language)
    let session = URLSession(configuration: sessionConfiguration, delegate: NoRedirects(), delegateQueue: nil)
    defer { session.invalidateAndCancel() }
    let (bytes, response) = try await session.bytes(for: request)
    try requireSuccess(response)
    var data = Data()
    for try await byte in bytes {
      guard data.count < 1024 * 1024 else { throw ServiceFailure(message: "服务响应过大。") }
      data.append(byte)
    }
    try Task.checkCancellation()
    return try AppServicesBridge.parseResponse(data, voice: kind == .voice)
  }

  /// Doubao recognition while the recording is still running (see `DoubaoVoiceClient.transcribeLive`).
  static func streamDoubao(configuration: CustomServiceConfiguration, token: String, generation: UInt64,
                           client: DoubaoVoiceClient, pcm: AsyncStream<Data>,
                           partial: @escaping (String) -> Void) async throws -> String {
    let url = try configuration.validatedURL(requiresModel: false, allowWebSocket: true)
    let handshake = try configuration.doubaoHandshake(accessKey: token)
    do {
      return try await client.transcribeLive(endpoint: url, handshake: handshake, generation: generation,
                                             pcm: pcm, partial: partial)
    } catch DoubaoVoiceClient.Failure.emptyTranscript {
      throw ServiceFailure(message: "豆包未返回可用的语音文本。")
    }
  }

  /// 「测试连接」, the desktop's credential test: a one-word chat for AI and one second of silence for voice. Any 2xx proves the endpoint, model and key, whatever the reply says; for Doubao, an empty transcript means the handshake was accepted.
  static func test(kind: CustomServiceKind, configuration: CustomServiceConfiguration, token: String,
                   doubaoClient: DoubaoVoiceClient? = nil,
                   sessionConfiguration: URLSessionConfiguration = .ephemeral) async throws {
    // 16 kHz mono 16-bit, the format the recorder produces.
    let silence = Data(count: 32_000)
    if kind == .voice && configuration.voiceProvider == .doubao {
      guard let doubaoClient else { throw ServiceFailure(message: "豆包语音需要原生 host codec。") }
      let url = try configuration.validatedURL(requiresModel: false, allowWebSocket: true)
      do {
        _ = try await doubaoClient.transcribe(endpoint: url, handshake: configuration.doubaoHandshake(accessKey: token),
                                              generation: 1, pcm: silence)
      } catch DoubaoVoiceClient.Failure.emptyTranscript {}
      return
    }
    var request = try makeRequest(kind: kind, configuration: configuration, prompt: "Reply OK", text: "OK",
                                  wav: kind == .voice ? silentWAV(silence) : nil, token: token)
    request.timeoutInterval = 20
    let session = URLSession(configuration: sessionConfiguration, delegate: NoRedirects(), delegateQueue: nil)
    defer { session.invalidateAndCancel() }
    let (_, response) = try await session.bytes(for: request)
    try requireSuccess(response)
  }

  static func makeRequest(kind: CustomServiceKind, configuration: CustomServiceConfiguration, prompt: String,
                          text: String, wav: Data?, token: String, language: String? = nil) throws -> URLRequest {
    let url = try configuration.validatedURL()
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.timeoutInterval = 60
    if !token.isEmpty {
      if url.host?.caseInsensitiveCompare("api.anthropic.com") == .orderedSame {
        request.setValue(token, forHTTPHeaderField: "x-api-key")
        request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")
      } else {
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
      }
    }
    if kind == .voice {
      guard let wav else { throw ServiceFailure(message: "请先录音。") }
      let multipart = try AppServicesBridge.transcriptionBody(wav, model: configuration.model, language: language)
      request.httpBody = multipart["body"] as? Data
      request.setValue(multipart["contentType"] as? String, forHTTPHeaderField: "Content-Type")
    } else {
      guard text.count <= 10000 else { throw ServiceFailure(message: "每次最多处理一万字。") }
      request.httpBody = try AppServicesBridge.polishBody(configuration.model, prompt: prompt, text: text)
      request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    }
    return request
  }

  private static func requireSuccess(_ response: URLResponse) throws {
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
      let status = (response as? HTTPURLResponse)?.statusCode ?? 0
      throw ServiceFailure(message: "服务请求失败（HTTP \(status)），请检查地址、模型和密钥。")
    }
  }

  /// A 16 kHz mono 16-bit PCM WAV around `pcm`.
  static func silentWAV(_ pcm: Data) -> Data {
    func le32(_ value: UInt32) -> Data { withUnsafeBytes(of: value.littleEndian) { Data($0) } }
    func le16(_ value: UInt16) -> Data { withUnsafeBytes(of: value.littleEndian) { Data($0) } }
    var wav = Data("RIFF".utf8) + le32(UInt32(36 + pcm.count)) + Data("WAVEfmt ".utf8)
    wav += le32(16) + le16(1) + le16(1) + le32(16_000) + le32(32_000) + le16(2) + le16(16)
    wav += Data("data".utf8) + le32(UInt32(pcm.count)) + pcm
    return wav
  }
}

// Service-configuration metadata only; generation/transcription codecs stay in Engine.
enum ModelCatalogClient {
  struct Page: Decodable {
    struct Model: Decodable {
      let id: String
      let supported_endpoint_types: [String]?
      let chat_completions_bridge: Bool?
      let active: Bool?
    }
    let data: [Model]
    let has_more: Bool?
    let last_id: String?
  }

  static func modelsURL(configuration: CustomServiceConfiguration) throws -> URL {
    let endpoint = try configuration.validatedURL(requiresModel: false)
    var parts = URLComponents(url: endpoint, resolvingAgainstBaseURL: false)!
    var path = parts.path
    while path.hasSuffix("/") { path.removeLast() }
    for suffix in ["/chat/completions", "/audio/transcriptions"] where path.hasSuffix(suffix) {
      path.removeLast(suffix.count)
      break
    }
    parts.path = path + "/models"
    guard let url = parts.url else { throw ServiceFailure(message: "无法确定模型列表地址。") }
    return url
  }

  static func fetch(configuration: CustomServiceConfiguration, kind: CustomServiceKind,
                    token: String, sessionConfiguration: URLSessionConfiguration = .ephemeral) async throws -> [String] {
    let key = token.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !key.isEmpty else { throw ServiceFailure(message: "请先填写 API Key，或使用已保存的密钥。") }
    let baseURL = try modelsURL(configuration: configuration)
    let anthropic = baseURL.host == "api.anthropic.com"
    let session = URLSession(configuration: sessionConfiguration, delegate: NoRedirects(), delegateQueue: nil)
    defer { session.invalidateAndCancel() }
    var models = Set<String>()
    var cursor: String?
    var cursors = Set<String>()
    for _ in 0..<10 {
      var parts = URLComponents(url: baseURL, resolvingAgainstBaseURL: false)!
      if anthropic {
        var query = (parts.queryItems ?? []).filter { !["limit", "after_id"].contains($0.name) }
        query.append(URLQueryItem(name: "limit", value: "1000"))
        if let cursor { query.append(URLQueryItem(name: "after_id", value: cursor)) }
        parts.queryItems = query
      }
      var request = URLRequest(url: parts.url!)
      request.timeoutInterval = 30
      request.setValue("application/json", forHTTPHeaderField: "Accept")
      if anthropic {
        request.setValue(key, forHTTPHeaderField: "x-api-key")
        request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")
      } else { request.setValue("Bearer \(key)", forHTTPHeaderField: "Authorization") }
      let (bytes, response) = try await session.bytes(for: request)
      let status = (response as? HTTPURLResponse)?.statusCode ?? 0
      guard (200..<300).contains(status) else {
        let detail = status == 401 || status == 403 ? "请检查 API Key 和账号权限。" : "服务商可能不支持模型列表接口，可继续使用预设或自定义模型。"
        throw ServiceFailure(message: "获取模型失败（HTTP \(status)）。\(detail)")
      }
      var data = Data()
      for try await byte in bytes {
        guard data.count < 1024 * 1024 else { throw ServiceFailure(message: "模型列表响应过大。") }
        data.append(byte)
      }
      try Task.checkCancellation()
      guard let page = try? JSONDecoder().decode(Page.self, from: data) else {
        throw ServiceFailure(message: "模型列表格式不兼容，可继续使用预设或自定义模型。")
      }
      for model in page.data {
        guard model.active != false, !model.id.isEmpty, model.id.count <= 256 else { continue }
        if let endpoints = model.supported_endpoint_types, !endpoints.isEmpty {
          let supported = kind == .voice ? endpoints.contains("audio-transcription")
            : endpoints.contains("openai") || model.chat_completions_bridge == true
          if !supported { continue }
        }
        models.insert(model.id)
        guard models.count <= 5000 else { throw ServiceFailure(message: "模型数量过多，请缩小服务商的模型授权范围。") }
      }
      if page.has_more != true {
        guard !models.isEmpty else { throw ServiceFailure(message: "未返回可用模型，请检查密钥权限，或使用自定义模型。") }
        return models.sorted()
      }
      guard anthropic, let next = page.last_id, !next.isEmpty, cursors.insert(next).inserted else {
        throw ServiceFailure(message: "模型列表分页格式不兼容，请使用预设或自定义模型。")
      }
      cursor = next
    }
    throw ServiceFailure(message: "模型列表分页过多，请使用预设或自定义模型。")
  }
}

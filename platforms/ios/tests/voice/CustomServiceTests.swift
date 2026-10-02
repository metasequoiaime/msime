import XCTest
import Foundation

final class FixtureProtocol: URLProtocol, @unchecked Sendable {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let denied = request.url?.path == "/denied"
    let data = Data((denied ? "private server detail" : "{\"choices\":[{\"message\":{\"content\":\"润色结果\"}}]}").utf8)
    let response = HTTPURLResponse(url: request.url!, statusCode: denied ? 401 : 200,
                                   httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
  func testDoubaoVoicePresetUsesNativeWebSocketEndpoint() throws {
    let config = CustomServiceConfiguration.loadVoicePreset(.doubao)
    XCTAssertEqual(config.endpoint, "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async")
    XCTAssertEqual(config.model, "volc.seedasr.sauc.duration")
    XCTAssertEqual(config.voiceProvider, .doubao)
    XCTAssertThrowsError(try config.validatedURL())
    XCTAssertNoThrow(try config.validatedURL(allowWebSocket: true))
  }

  func testDoubaoOptionsRoundTripThroughCodable() throws {
    var config = CustomServiceConfiguration.loadVoicePreset(.doubao)
    config.voiceAppKey = "fixture-app"
    config.voiceResourceID = "fixture-resource"
    config.doubaoEnableITN = false
    config.doubaoEnablePunctuation = false
    config.doubaoEnableDDC = true
    config.doubaoBoostingTableID = "fixture-table"
    let data = try JSONEncoder().encode(config)
    let decoded = try JSONDecoder().decode(CustomServiceConfiguration.self, from: data)
    XCTAssertEqual(decoded, config)
  }

}

final class CustomServiceTests: XCTestCase {
  func testPresetsAreUsableAndKeepSeparateSavedConfigurations() throws {
    let suite = "msime-provider-tests-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    var custom = CustomServiceConfiguration()
    custom.endpoint = "https://custom.invalid/v1/chat/completions"
    custom.model = "custom-model"
    try custom.save(.ai, token: "", defaults: defaults)
    for provider in AIProviderPreset.allCases where provider != .custom {
      var configuration = CustomServiceConfiguration.loadPreset(provider, defaults: defaults)
      XCTAssertEqual(configuration.provider, provider)
      XCTAssertEqual(try configuration.validatedURL().absoluteString, provider.endpoint)
      XCTAssertNotNil(provider.documentation)
      configuration.model = "my-\(provider.rawValue)-model"
      try configuration.save(.ai, token: "", defaults: defaults)
      XCTAssertEqual(CustomServiceConfiguration.load(.ai, defaults: defaults).provider, provider)
    }
    for provider in AIProviderPreset.allCases where provider != .custom {
      XCTAssertEqual(CustomServiceConfiguration.loadPreset(provider, defaults: defaults).model,
                     "my-\(provider.rawValue)-model")
    }
    XCTAssertEqual(CustomServiceConfiguration.loadPreset(.custom, defaults: defaults).endpoint, custom.endpoint)
    XCTAssertEqual(CustomServiceConfiguration.loadPreset(.custom, defaults: defaults).model, custom.model)
  }

  func testVoicePresetsPreserveCustomAndDoNotChangeAI() throws {
    let suite = "msime-voice-tests-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    let ai = CustomServiceConfiguration.loadPreset(.deepSeek, defaults: defaults)
    try ai.save(.ai, token: "", defaults: defaults)
    var customVoice = CustomServiceConfiguration.loadVoicePreset(.custom, defaults: defaults)
    customVoice.endpoint = "https://custom.invalid/audio/transcriptions"
    customVoice.model = "custom-model"
    try customVoice.save(.voice, token: "", defaults: defaults)
    for provider in VoiceProviderPreset.allCases where provider != .custom && !provider.isOnDevice {
      var config = CustomServiceConfiguration.loadVoicePreset(provider, defaults: defaults)
      XCTAssertEqual(try config.validatedURL(allowWebSocket: provider == .doubao).absoluteString,
                     provider.endpoint)
      if provider == .doubao {
        XCTAssertNil(provider.documentation)
      } else {
        XCTAssertNotNil(provider.documentation)
      }
      config.model = "saved-\(provider.rawValue)"
      try config.save(.voice, token: "", defaults: defaults)
      XCTAssertEqual(CustomServiceConfiguration.load(.voice, defaults: defaults).voiceProvider, provider)
    }
    for provider in VoiceProviderPreset.allCases where provider != .custom && !provider.isOnDevice {
      XCTAssertEqual(CustomServiceConfiguration.loadVoicePreset(provider, defaults: defaults).model,
                     "saved-\(provider.rawValue)")
    }
    let custom = CustomServiceConfiguration.loadVoicePreset(.custom, defaults: defaults)
    XCTAssertEqual(custom.model, "custom-model")
    XCTAssertEqual(custom.endpoint, "https://custom.invalid/audio/transcriptions")
    XCTAssertEqual(CustomServiceConfiguration.load(.ai, defaults: defaults).endpoint, ai.endpoint)
    XCTAssertEqual(CustomServiceConfiguration.load(.ai, defaults: defaults).provider, .deepSeek)
  }

  func testConfigurationRejectsUnsafeOrIncompleteEndpoints() {
    for endpoint in ["http://example.invalid/v1", "https://user:password@example.invalid/v1", "https://example.invalid/v1#fragment", ""] {
      var configuration = CustomServiceConfiguration()
      configuration.endpoint = endpoint
      configuration.model = "fixture"
      XCTAssertThrowsError(try configuration.validatedURL())
    }
    var configuration = CustomServiceConfiguration()
    configuration.endpoint = "https://example.invalid/v1/chat/completions"
    XCTAssertThrowsError(try configuration.validatedURL())
    configuration.model = "fixture"
    XCTAssertEqual(try configuration.validatedURL().path, "/v1/chat/completions")
  }

  func testEngineCodecsPreserveTextAndAudioAndRejectMalformedResponses() throws {
    let text = "你好\n\"测试\""
    let data = try AppServicesBridge.polishBody("fixture", prompt: "润色", text: text)
    let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    let messages = try XCTUnwrap(json["messages"] as? [[String: String]])
    XCTAssertEqual(messages.last?["content"], text)
    let wav = Data([0x52, 0x49, 0x46, 0x46, 0, 255, 17])
    let multipart = try AppServicesBridge.transcriptionBody(wav, model: "asr-fixture")
    let body = try XCTUnwrap(multipart["body"] as? Data)
    XCTAssertNotNil(body.range(of: wav))
    XCTAssertTrue(try XCTUnwrap(multipart["contentType"] as? String).contains("boundary="))
    XCTAssertEqual(try AppServicesBridge.parseResponse(Data("{\"text\":\"语音测试\"}".utf8), voice: true), "语音测试")
    XCTAssertThrowsError(try AppServicesBridge.parseResponse(Data("{\"error\":\"private\"}".utf8), voice: false))
  }

  func testTransportUsesConfiguredEndpointAndReportsHTTPFailure() async throws {
    let session = URLSessionConfiguration.ephemeral
    session.protocolClasses = [FixtureProtocol.self]
    var configuration = CustomServiceConfiguration()
    configuration.endpoint = "https://msime-tests.invalid/success"
    configuration.model = "fixture"
    let result = try await CustomServiceClient.request(kind: .ai, configuration: configuration,
      text: "你好", token: "fixture-token", sessionConfiguration: session)
    XCTAssertEqual(result, "润色结果")
    configuration.endpoint = "https://msime-tests.invalid/denied"
    do {
      _ = try await CustomServiceClient.request(kind: .ai, configuration: configuration,
        text: "你好", token: "fixture-token", sessionConfiguration: session)
      XCTFail("HTTP failure was accepted")
    } catch {
      XCTAssertTrue(error.localizedDescription.contains("401"))
      XCTAssertFalse(error.localizedDescription.contains("private server detail"))
    }
  }
}

final class CatalogFixtureProtocol: URLProtocol, @unchecked Sendable {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let anthropic = request.url?.host == "api.anthropic.com"
    let authorized = anthropic
      ? request.value(forHTTPHeaderField: "x-api-key") == "fixture" && request.value(forHTTPHeaderField: "anthropic-version") == "2023-06-01"
      : request.value(forHTTPHeaderField: "Authorization") == "Bearer fixture"
    let valid = authorized && request.httpMethod == "GET" && request.url!.path.hasSuffix("/models")
    var payload = "private failure detail"
    if valid {
      if anthropic {
        payload = request.url!.query!.contains("after_id=")
          ? #"{"data":[{"id":"claude-second"}],"has_more":false}"#
          : #"{"data":[{"id":"claude-first"}],"has_more":true,"last_id":"claude-first"}"#
      } else {
        payload = #"{"data":[{"id":"chat-model","supported_endpoint_types":["openai"]},{"id":"speech-model","supported_endpoint_types":["audio-transcription"]},{"id":"chat-model","supported_endpoint_types":["openai"]},{"id":"disabled","active":false},{"id":"response-model","supported_endpoint_types":["openai-response"],"chat_completions_bridge":true}]}"#
      }
    }
    let response = HTTPURLResponse(url: request.url!, statusCode: valid ? 200 : 401,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: Data(payload.utf8))
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

extension CustomServiceTests {
  func testModelCatalogUsesKeyWithoutRequiringAModelAndFiltersCapabilities() async throws {
    let session = URLSessionConfiguration.ephemeral
    session.protocolClasses = [CatalogFixtureProtocol.self]
    var config = CustomServiceConfiguration.loadPreset(.everyAPI)
    config.model = ""
    XCTAssertEqual(try ModelCatalogClient.modelsURL(configuration: config).absoluteString, "https://api.everyapi.ai/v1/models")
    let models = try await ModelCatalogClient.fetch(configuration: config, kind: .ai, token: "fixture", sessionConfiguration: session)
    XCTAssertEqual(models, ["chat-model", "response-model"])
    let voice = CustomServiceConfiguration.loadVoicePreset(.everyAPI)
    XCTAssertEqual(try ModelCatalogClient.modelsURL(configuration: voice).absoluteString, "https://api.everyapi.ai/v1/models")
    let voiceModels = try await ModelCatalogClient.fetch(configuration: voice, kind: .voice, token: "fixture", sessionConfiguration: session)
    XCTAssertEqual(voiceModels, ["speech-model"])
    let gemini = CustomServiceConfiguration.loadPreset(.gemini)
    XCTAssertEqual(try ModelCatalogClient.modelsURL(configuration: gemini).absoluteString,
      "https://generativelanguage.googleapis.com/v1beta/openai/models")
  }

  func testModelCatalogAnthropicPaginationAndAuthenticationFailures() async throws {
    let session = URLSessionConfiguration.ephemeral
    session.protocolClasses = [CatalogFixtureProtocol.self]
    let config = CustomServiceConfiguration.loadPreset(.anthropic)
    let models = try await ModelCatalogClient.fetch(configuration: config, kind: .ai, token: "fixture", sessionConfiguration: session)
    XCTAssertEqual(models, ["claude-first", "claude-second"])
    for key in ["", "invalid"] {
      do {
        _ = try await ModelCatalogClient.fetch(configuration: config, kind: .ai, token: key, sessionConfiguration: session)
        XCTFail("Invalid key accepted")
      } catch {
        XCTAssertFalse(error.localizedDescription.contains("private failure detail"))
        XCTAssertTrue(error.localizedDescription.contains(key.isEmpty ? "API Key" : "401"))
      }
    }
  }
}

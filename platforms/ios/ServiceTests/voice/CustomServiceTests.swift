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
}

final class CustomServiceTests: XCTestCase {
  func testDoubaoRequestUsesPCMCoordinatorAndHandshake() async throws {
    let transport = DoubaoRequestFixtureTransport()
    var packets: [(Int32, Int, Bool)] = []
    let codec = DoubaoVoiceCoordinator.FrameCodec(
      startFrame: { Data([0x01]) },
      audioFrame: { sequence, pcm, final in
        packets.append((sequence, pcm.count, final))
        return Data([UInt8(truncatingIfNeeded: sequence)])
      },
      decodeFrame: { frame in frame == Data([0xFF]) ? (true, "fixture transcript") : nil }
    )
    let client = DoubaoVoiceClient(transport: transport, codec: codec)
    var configuration = CustomServiceConfiguration.loadVoicePreset(.doubao)
    configuration.voiceAppKey = "fixture-app"
    configuration.voiceResourceID = "fixture-resource"
    let result = try await CustomServiceClient.request(
      kind: .voice, configuration: configuration, pcm: Data(repeating: 0x2A, count: 6_401),
      token: "fixture-access", generation: 19, doubaoClient: client)

    XCTAssertEqual(result, "fixture transcript")
    XCTAssertEqual(transport.handshake?.appKey, "fixture-app")
    XCTAssertEqual(transport.handshake?.accessKey, "fixture-access")
    XCTAssertEqual(transport.handshake?.resourceID, "fixture-resource")
    XCTAssertEqual(transport.sent, [Data([0x01]), Data([2]), Data([0xFD])])
    XCTAssertEqual(packets.map(\.0), [2, -3])
    XCTAssertEqual(packets.map(\.1), [6_400, 1])
    XCTAssertEqual(packets.map(\.2), [false, true])
  }

  func testDoubaoStreamsWhileRecordingAndReportsEachPartial() async throws {
    let transport = DoubaoLiveFixtureTransport()
    var packets: [(Int32, Int, Bool)] = []
    let codec = DoubaoVoiceCoordinator.FrameCodec(
      startFrame: { Data([0x01]) },
      audioFrame: { sequence, pcm, final in
        packets.append((sequence, pcm.count, final))
        return Data([UInt8(truncatingIfNeeded: sequence)])
      },
      decodeFrame: { frame in
        switch frame {
        case Data([0xA1]): (false, "你好")
        case Data([0xFF]): (true, "你好世界")
        default: nil
        }
      }
    )
    var configuration = CustomServiceConfiguration.loadVoicePreset(.doubao)
    configuration.voiceAppKey = "fixture-app"
    configuration.voiceResourceID = "fixture-resource"
    let (pcm, continuation) = AsyncStream<Data>.makeStream()
    continuation.yield(Data(repeating: 0x2A, count: 4_000))
    continuation.yield(Data(repeating: 0x2A, count: 2_401))
    continuation.finish()
    var partials: [String] = []
    let result = try await CustomServiceClient.streamDoubao(
      configuration: configuration, token: "fixture-access", generation: 7,
      client: DoubaoVoiceClient(transport: transport, codec: codec), pcm: pcm) { partials.append($0) }

    XCTAssertEqual(result, "你好世界")
    XCTAssertEqual(partials, ["你好", "你好世界"])
    XCTAssertEqual(transport.handshake?.accessKey, "fixture-access")
    XCTAssertEqual(transport.sentFrames, [Data([0x01]), Data([2]), Data([0xFD])])
    XCTAssertEqual(packets.map(\.0), [2, -3])
    XCTAssertEqual(packets.map(\.1), [6_400, 1])
    XCTAssertEqual(packets.map(\.2), [false, true])
  }

  func testDoubaoRequestDoesNotFallBackToMultipartWithoutHostCodec() async throws {
    let configuration = CustomServiceConfiguration.loadVoicePreset(.doubao)
    do {
      _ = try await CustomServiceClient.request(
        kind: .voice, configuration: configuration, pcm: Data([0x01]), token: "fixture-access")
      XCTFail("Doubao accepted a request without a host codec")
    } catch {
      XCTAssertTrue(error.localizedDescription.contains("原生 host codec"))
    }
  }

  func testDoubaoRejectsAnOversizedResponseFrame() async throws {
    let transport = DoubaoOversizedFrameFixtureTransport()
    let codec = DoubaoVoiceCoordinator.FrameCodec(
      startFrame: { Data([0x01]) },
      audioFrame: { _, _, _ in Data([0x02]) },
      decodeFrame: { frame in frame == Data([0xFF]) ? (true, "fixture transcript") : nil }
    )
    let configuration = CustomServiceConfiguration.loadVoicePreset(.doubao)
    do {
      _ = try await CustomServiceClient.request(
        kind: .voice, configuration: configuration, pcm: Data([0x01]), token: "fixture-access",
        generation: 1, doubaoClient: DoubaoVoiceClient(transport: transport, codec: codec))
      XCTFail("oversized response frame was accepted")
    } catch DoubaoVoiceCoordinator.Failure.responseTooLarge {
    }
  }

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

  func testSavingAnOversizedPolishPromptIsRejectedBeforeItReachesSharedDefaults() throws {
    let suite = "msime-provider-limit-tests-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    var configuration = CustomServiceConfiguration()
    configuration.endpoint = "https://custom.invalid/v1/chat/completions"
    configuration.model = "custom-model"
    configuration.prompt = String(repeating: "提示", count: 16_385)
    XCTAssertThrowsError(try configuration.save(.ai, token: "", defaults: defaults))
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

  func testOnDeviceProvidersSaveOnlyTheChoiceAndKeepTheCloudService() throws {
    let suite = "msime-voice-on-device-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    var customVoice = CustomServiceConfiguration.loadVoicePreset(.custom, defaults: defaults)
    customVoice.endpoint = "https://custom.invalid/audio/transcriptions"
    customVoice.model = "custom-model"
    try customVoice.save(.voice, token: "", defaults: defaults)
    for provider in [VoiceProviderPreset.local, .system] {
      XCTAssertTrue(provider.isOnDevice)
      XCTAssertEqual(provider.endpoint, "")
      XCTAssertNil(provider.documentation)
      let config = CustomServiceConfiguration.loadVoicePreset(provider, defaults: defaults)
      XCTAssertThrowsError(try config.validatedURL())
      XCTAssertNoThrow(try config.save(.voice, token: "", defaults: defaults))
      XCTAssertEqual(CustomServiceConfiguration.load(.voice, defaults: defaults).voiceProvider, provider)
      XCTAssertEqual(defaults.string(forKey: "service.voice.endpoint"), "https://custom.invalid/audio/transcriptions")
    }
    XCTAssertNil(defaults.string(forKey: "service.voice.presets.local.endpoint"))
    let custom = CustomServiceConfiguration.loadVoicePreset(.custom, defaults: defaults)
    XCTAssertEqual(custom.endpoint, "https://custom.invalid/audio/transcriptions")
    XCTAssertEqual(custom.model, "custom-model")
    XCTAssertFalse(VoiceProviderPreset.allCases.filter { !$0.isOnDevice }.contains { $0.endpoint.isEmpty && $0 != .custom })
  }

  func testDoubaoKeepsTheChosenStreamEndpointAndReadsBothResultShapes() throws {
    let suite = "msime-doubao-endpoint-\(UUID().uuidString)"
    let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite) }
    let endpoints = VoiceProviderPreset.doubaoStreamEndpoints.map(\.endpoint)
    XCTAssertTrue(endpoints.contains(VoiceProviderPreset.doubao.endpoint), "默认接口要在可选列表里")
    let nostream = try XCTUnwrap(endpoints.first { $0.hasSuffix("bigmodel_nostream") })
    var config = CustomServiceConfiguration.loadVoicePreset(.doubao, defaults: defaults)
    config.endpoint = nostream
    try config.save(.voice, token: "", defaults: defaults)
    XCTAssertEqual(CustomServiceConfiguration.loadVoicePreset(.doubao, defaults: defaults).endpoint, nostream)
    XCTAssertEqual(CustomServiceConfiguration.load(.voice, defaults: defaults).endpoint, nostream)

    XCTAssertEqual(DoubaoHostFrameCodec.transcript(in: ["result": ["text": "你好"]]), "你好")
    XCTAssertEqual(DoubaoHostFrameCodec.transcript(in: ["result": [["text": "你好，"], ["text": "世界"]]]), "你好，世界")
    XCTAssertEqual(DoubaoHostFrameCodec.transcript(in: ["result": [[String: Any]]()]), "")
    XCTAssertEqual(DoubaoHostFrameCodec.transcript(in: ["text": "网关"]), "网关")
    XCTAssertNil(DoubaoHostFrameCodec.transcript(in: [:]))
    XCTAssertNil(DoubaoHostFrameCodec.transcript(in: ["text": String(repeating: "字", count: 10_001)]))
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

  func testConfigurationRejectsOverlongModelAndEndpoint() {
    var configuration = CustomServiceConfiguration()
    configuration.endpoint = "https://example.invalid/v1/chat/completions"
    configuration.model = String(repeating: "m", count: 257)
    XCTAssertThrowsError(try configuration.validatedURL())

    configuration.model = "fixture"
    configuration.endpoint = "https://example.invalid/" + String(repeating: "a", count: 2_048)
    XCTAssertThrowsError(try configuration.validatedURL())

    configuration.endpoint = "https://example.invalid/" + String(repeating: "界", count: 700)
    XCTAssertThrowsError(try configuration.validatedURL())
  }

  func testDoubaoConfigurationRejectsOversizedCredentialFields() throws {
    var configuration = CustomServiceConfiguration.loadVoicePreset(.doubao)
    configuration.voiceAppKey = String(repeating: "a", count: 8_193)
    configuration.voiceResourceID = "fixture-resource"
    configuration.doubaoBoostingTableID = "fixture-table"
    XCTAssertThrowsError(try configuration.save(.voice, token: "fixture-access"))

    configuration.voiceAppKey = "fixture-app"
    configuration.voiceResourceID = "fixture-resource"
    configuration.doubaoBoostingTableID = String(repeating: "表", count: 2_731)
    XCTAssertThrowsError(try configuration.save(.voice, token: "fixture-access"))

    configuration.doubaoBoostingTableID = "fixture-table"
    configuration.voiceResourceID = String(repeating: "r", count: 8_193)
    XCTAssertThrowsError(try configuration.doubaoHandshake(accessKey: "fixture-access"))
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

  func testEngineCodecsRejectOversizedRecognitionAndPolishText() throws {
    let oversized = String(repeating: "字", count: 20_000)
    let voice = try JSONSerialization.data(withJSONObject: ["text": oversized])
    let voiceWithChatFallback = try JSONSerialization.data(withJSONObject: [
      "text": oversized,
      "choices": [["message": ["content": "错误协议回退"]]]
    ])
    let polish = try JSONSerialization.data(withJSONObject: [
      "choices": [["message": ["content": oversized]]]
    ])
    XCTAssertThrowsError(try AppServicesBridge.parseResponse(voice, voice: true))
    XCTAssertThrowsError(try AppServicesBridge.parseResponse(voiceWithChatFallback, voice: true))
    XCTAssertThrowsError(try AppServicesBridge.parseResponse(polish, voice: false))
  }

  func testPolishBodyRejectsAnOversizedPromptBeforeBuildingTheRequest() {
    XCTAssertThrowsError(try AppServicesBridge.polishBody(
      "fixture", prompt: String(repeating: "提示", count: 16_385), text: "合成文本"))
  }

  func testPolishBodyRejectsAnOversizedUnicodeTextBeforeBuildingTheRequest() {
    XCTAssertThrowsError(try AppServicesBridge.polishBody(
      "fixture", prompt: "润色", text: String(repeating: "😀", count: 8_193)))
  }

  func testTranscriptionBodyRejectsOversizedAudioBeforeBuildingTheRequest() {
    XCTAssertThrowsError(try AppServicesBridge.transcriptionBody(
      Data(repeating: 0x2A, count: 2_100_001), model: "asr-fixture"))
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

  func testConnectionTestAcceptsAnySuccessAndReportsHTTPFailure() async throws {
    let session = URLSessionConfiguration.ephemeral
    session.protocolClasses = [FixtureProtocol.self]
    var configuration = CustomServiceConfiguration()
    configuration.endpoint = "https://msime-tests.invalid/success"
    configuration.model = "fixture"
    try await CustomServiceClient.test(kind: .ai, configuration: configuration, token: "fixture-token",
                                       sessionConfiguration: session)
    // The fixture's chat reply is not a transcript; a voice test still passes on the status alone.
    try await CustomServiceClient.test(kind: .voice, configuration: configuration, token: "fixture-token",
                                       sessionConfiguration: session)
    configuration.endpoint = "https://msime-tests.invalid/denied"
    do {
      try await CustomServiceClient.test(kind: .ai, configuration: configuration, token: "fixture-token",
                                         sessionConfiguration: session)
      XCTFail("HTTP failure was accepted")
    } catch {
      XCTAssertTrue(error.localizedDescription.contains("401"))
    }
  }

  func testConnectionTestSendsOneSecondOfSilence() throws {
    let pcm = Data(count: 32_000)
    let wav = CustomServiceClient.silentWAV(pcm)
    XCTAssertEqual(wav.count, 44 + pcm.count)
    XCTAssertEqual(WAVPCMExtractor.extract(from: wav), pcm)
  }

  func testDoubaoConnectionTestTreatsAnEmptyTranscriptAsAccepted() async throws {
    let transport = DoubaoRequestFixtureTransport()
    let codec = DoubaoVoiceCoordinator.FrameCodec(
      startFrame: { Data([0x01]) },
      audioFrame: { _, _, _ in Data([0x02]) },
      decodeFrame: { _ in (true, "") }
    )
    var configuration = CustomServiceConfiguration.loadVoicePreset(.doubao)
    configuration.voiceAppKey = "fixture-app"
    configuration.voiceResourceID = "fixture-resource"
    try await CustomServiceClient.test(kind: .voice, configuration: configuration, token: "fixture-access",
                                       doubaoClient: DoubaoVoiceClient(transport: transport, codec: codec))
    XCTAssertEqual(transport.handshake?.accessKey, "fixture-access")
  }
}

private final class DoubaoRequestFixtureTransport: DoubaoVoiceTransport {
  var handshake: DoubaoHandshake?
  var sent: [Data] = []

  func start(endpoint: URL) async throws {}
  func start(endpoint: URL, handshake: DoubaoHandshake) async throws { self.handshake = handshake }
  func send(binary frame: Data) async throws { sent.append(frame) }
  func receive() async throws -> Data { Data([0xFF]) }
  func finish() {}
}

private final class DoubaoOversizedFrameFixtureTransport: DoubaoVoiceTransport {
  private var first = true

  func start(endpoint: URL) async throws {}
  func start(endpoint: URL, handshake: DoubaoHandshake) async throws {}
  func send(binary frame: Data) async throws {}
  func receive() async throws -> Data {
    if first {
      first = false
      return Data(repeating: 0x2A, count: 1_048_577)
    }
    return Data([0xFF])
  }
  func finish() {}
}

/// Answers with a partial result at once and holds the final one until the last packet has gone out, the order a live session sees.
private final class DoubaoLiveFixtureTransport: DoubaoVoiceTransport, @unchecked Sendable {
  private let lock = NSLock()
  private var sent: [Data] = []
  private var received = 0
  var handshake: DoubaoHandshake?

  var sentFrames: [Data] { lock.withLock { sent } }

  func start(endpoint: URL) async throws {}
  func start(endpoint: URL, handshake: DoubaoHandshake) async throws { self.handshake = handshake }
  func send(binary frame: Data) async throws { lock.withLock { sent.append(frame) } }
  func receive() async throws -> Data {
    let first = lock.withLock { () -> Bool in
      received += 1
      return received == 1
    }
    if first { return Data([0xA1]) }
    while !lock.withLock({ sent.contains(Data([0xFD])) }) { try await Task.sleep(nanoseconds: 1_000_000) }
    return Data([0xFF])
  }
  func finish() {}
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
  func testAnthropicAIRequestUsesNativeAuthenticationHeaders() throws {
    let configuration = CustomServiceConfiguration.loadPreset(.anthropic)
    let request = try CustomServiceClient.makeRequest(
      kind: .ai, configuration: configuration, prompt: "Reply OK", text: "OK", wav: nil,
      token: "fixture")
    XCTAssertEqual(request.value(forHTTPHeaderField: "x-api-key"), "fixture")
    XCTAssertEqual(request.value(forHTTPHeaderField: "anthropic-version"), "2023-06-01")
    XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))
  }

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

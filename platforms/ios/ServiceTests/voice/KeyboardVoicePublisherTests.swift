import XCTest

/// 哪些语音服务发布给键盘自己录音（`KeyboardVoicePublisher`），发布出去的请求字段是否与 App 自己发的一致。
final class KeyboardVoicePublisherTests: XCTestCase {
  private func configuration(_ provider: VoiceProviderPreset,
                             endpoint: String = "https://asr.example/v1/audio/transcriptions") -> CustomServiceConfiguration {
    var configuration = CustomServiceConfiguration()
    configuration.voiceProvider = provider
    configuration.endpoint = endpoint
    configuration.model = " whisper-1 "
    return configuration
  }

  /// 只有说完整段上传的联网服务能在键盘里识别；本地模型、系统识别和豆包仍在 App 里。
  func testOnlyWholeRecordingNetworkServicesArePublished() {
    for provider in VoiceProviderPreset.allCases {
      let published = KeyboardVoicePublisher.keyboardConfiguration(configuration(provider), settings: VoicePolishSettings())
      switch provider {
      case .local, .system, .doubao: XCTAssertNil(published, "\(provider)")
      case .everyAPI, .openAI, .siliconFlow, .groq, .mistral, .bailian, .custom: XCTAssertNotNil(published, "\(provider)")
      }
    }
  }

  /// 识别后润色要用 App 钥匙串里的润色密钥，打开时整条听写留在 App 里。
  func testPolishingKeepsDictationInTheApp() {
    var settings = VoicePolishSettings()
    settings.polishEnabled = true
    XCTAssertNil(KeyboardVoicePublisher.keyboardConfiguration(configuration(.openAI), settings: settings))
  }

  /// 语音接口只收 https，与 App 保存时的校验一样；不合格的地址不发布。
  func testAnEndpointTheAppWouldRefuseIsNotPublished() {
    XCTAssertNil(KeyboardVoicePublisher.keyboardConfiguration(
      configuration(.custom, endpoint: "http://asr.example/v1/audio/transcriptions"), settings: VoicePolishSettings()))
  }

  func testThePublishedRequestCarriesWhatTheAppWouldSend() throws {
    var settings = VoicePolishSettings()
    settings.language = "zh-cn"
    let openAI = try XCTUnwrap(KeyboardVoicePublisher.keyboardConfiguration(configuration(.openAI), settings: settings))
    XCTAssertEqual(openAI.endpoint, "https://asr.example/v1/audio/transcriptions")
    XCTAssertEqual(openAI.model, "whisper-1")
    XCTAssertFalse(openAI.chatAudio)
    XCTAssertEqual(openAI.language, settings.transcriptionLanguage(for: .openAI))
    XCTAssertEqual(openAI.language, "zh")
    // 硅基流动不收 language 字段，百炼走 chat_audio。
    XCTAssertNil(KeyboardVoicePublisher.keyboardConfiguration(configuration(.siliconFlow), settings: settings)?.language)
    XCTAssertEqual(KeyboardVoicePublisher.keyboardConfiguration(configuration(.bailian), settings: settings)?.chatAudio, true)

    // 提示音并上总开关：总开关关着时两声都不响。
    settings.soundEnabled = false
    let quiet = try XCTUnwrap(KeyboardVoicePublisher.keyboardConfiguration(configuration(.openAI), settings: settings))
    XCTAssertFalse(quiet.startSound)
    XCTAssertFalse(quiet.endSound)
    settings.soundEnabled = true
    settings.endSound = false
    let startOnly = try XCTUnwrap(KeyboardVoicePublisher.keyboardConfiguration(configuration(.openAI), settings: settings))
    XCTAssertTrue(startOnly.startSound)
    XCTAssertFalse(startOnly.endSound)
  }
}

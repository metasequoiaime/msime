import Foundation

/// 把「语音服务」选定的服务发布给键盘自己录音用（`KeyboardVoiceService`）。键盘里只做说完整段上传的联网识别：本地模型和系统识别要在 App 里跑，豆包走 App 的 WebSocket 会话，识别后润色要用 App 钥匙串里的润色密钥，这几种情况撤下发布，键盘照旧经后台会话交给 App。App 回到前台、保存语音服务、改语音设置后各刷新一次。
enum KeyboardVoicePublisher {
  /// `settings` 是共享文档里此刻的 `voice_input`：识别语言、提示音和识别后润色都从它来。
  static func refresh(settings: VoicePolishSettings, defaults: UserDefaults = .standard) {
    let configuration = CustomServiceConfiguration.load(.voice, defaults: defaults)
    guard let published = keyboardConfiguration(configuration, settings: settings),
          let url = try? configuration.validatedURL(),
          let token = try? ServiceTokenStore.read(.voice, url: url) else {
      KeyboardVoiceService.withdraw()
      return
    }
    do { try KeyboardVoiceService.publish(published, token: token) } catch { KeyboardVoiceService.withdraw() }
  }

  /// 键盘能自己识别时要发布的服务，不能时为 nil。
  static func keyboardConfiguration(_ configuration: CustomServiceConfiguration,
                                    settings: VoicePolishSettings) -> KeyboardVoiceConfiguration? {
    switch configuration.voiceProvider {
    case .everyAPI, .openAI, .siliconFlow, .groq, .mistral, .bailian, .custom: break
    case .local, .system, .doubao: return nil
    }
    guard !settings.polishEnabled, let url = try? configuration.validatedURL() else { return nil }
    return KeyboardVoiceConfiguration(
      endpoint: url.absoluteString,
      model: configuration.model.trimmingCharacters(in: .whitespacesAndNewlines),
      chatAudio: configuration.voiceProvider.usesChatAudio,
      language: settings.transcriptionLanguage(for: configuration.voiceProvider),
      startSound: settings.soundEnabled && settings.startSound,
      endSound: settings.soundEnabled && settings.endSound)
  }
}

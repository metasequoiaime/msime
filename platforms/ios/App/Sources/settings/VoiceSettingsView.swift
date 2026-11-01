import SwiftUI

/// 「语音输入」：设计稿里的精简页面（对应 Android `VoicePage`）。「识别」分组放识别语言、自动添加标点和长按空格说话的开关；「服务」分组通往完整的服务页（保留服务商配置、密钥、润色和 App 内录音）以及本地模型。
///
/// 识别语言存在共享文档的 `voice_input.language` 里，经 `VoicePolishSettings` 写入，`voice_input` 的其他字段保持不变。自动添加标点是服务页已经存在本 App defaults 里的豆包请求开关；只有豆包支持，其他识别服务下开关置灰。iOS 的语音入口是长按空格（`KeyboardLayoutPreference.spaceVoice`，本机 App Group）和「更多工具」里的「语音输入」磁贴，工具栏不放语音按钮，所以没有 Android「启动方式」那样的三选一；共享文档里的 `touch_voice_shortcut` 是 Android 的设置，这一页不读也不写，免得同步过去盖掉它。
struct VoiceSettingsView: View {
  @State private var settings = VoicePolishSettings(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var provider = CustomServiceConfiguration.load(.voice).voiceProvider
  @State private var punctuation = CustomServiceConfiguration.load(.voice).doubaoEnablePunctuation
  @State private var spaceVoice = KeyboardLayoutPreference.spaceVoice
  @State private var saveFailed = false

  /// 豆包的标点开关，与 `CustomServiceConfiguration` 读写的是同一个键。
  private static let punctuationKey = "service.voice.enable_punc"

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "识别", footer: saveFailed ? SettingsSaveFailure.message : nil) {
          DesignSelectRow(title: "识别语言",
                          options: VoicePolishSettings.languages.map {
                            DesignOption(title: $0.title, value: $0.id, identifier: "voiceLanguage_\($0.id)")
                          },
                          selection: Binding(get: { settings.language }, set: saveLanguage),
                          sheetTitle: "识别语言", identifier: "voiceLanguagePicker")
          DesignDivider()
          DesignToggleRow(title: "自动添加标点",
                          subtitle: provider == .doubao ? nil : "当前识别服务不支持，仅豆包语音识别可以自动加标点",
                          isOn: Binding(get: { provider == .doubao && punctuation }, set: savePunctuation))
            .disabled(provider != .doubao)
            .opacity(provider == .doubao ? 1 : 0.5)
            .accessibilityIdentifier("voicePunctuationToggle")
          DesignDivider()
          DesignToggleRow(title: "长按空格说话",
                          info: spaceVoice ? "按住空格说话，松开上屏，上滑取消" : Self.spaceVoiceOffHint,
                          isOn: Binding(get: { spaceVoice }, set: saveSpaceVoice))
            .accessibilityIdentifier("voiceSpaceToggle")
        }

        DesignGroup(title: "服务") {
          NavigationLink(destination: ServiceSettingsView(kind: .voice)) {
            DesignNavRowLabel(title: "识别服务", value: provider.title)
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("voiceServiceLink")
          DesignDivider()
          NavigationLink(destination: LocalSpeechModelsPage()) {
            DesignNavRowLabel(title: "本地语音模型", info: "离线识别，录音不离开设备")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("localSpeechModelsLink")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .designCanvasBackground()
    .navigationTitle("语音输入")
    .navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive(reload)
  }

  private func reload() {
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    settings = VoicePolishSettings(document)
    let configuration = CustomServiceConfiguration.load(.voice)
    provider = configuration.voiceProvider
    punctuation = configuration.doubaoEnablePunctuation
    spaceVoice = KeyboardLayoutPreference.spaceVoice
  }

  private func saveLanguage(_ language: String) {
    var updated = settings
    updated.language = language
    saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { updated.write(into: &$0) }
    if saveFailed { reload() } else { settings = updated }
  }

  private func savePunctuation(_ enabled: Bool) {
    punctuation = enabled
    UserDefaults.standard.set(enabled, forKey: Self.punctuationKey)
  }

  /// 长按空格说话只存在本机 App Group，键盘下次出现时读到。
  private func saveSpaceVoice(_ enabled: Bool) {
    KeyboardLayoutPreference.spaceVoice = enabled
    spaceVoice = enabled
  }

  /// 关掉长按空格说话后长按空格改为移动光标（键盘的 `handleSpaceVoiceHold`），空格键上的麦克风也随之消失；「更多工具」里的「语音输入」磁贴一直在，开关说明照实写出来（#10143）。
  static let spaceVoiceOffHint = "长按空格改为移动光标；要说话可在「更多工具」里点「语音输入」"
}

/// 单独一页的本地语音模型，内容与语音服务页里的那一节相同。
private struct LocalSpeechModelsPage: View {
  @StateObject private var manager = LocalSpeechModelManager()

  var body: some View {
    Form {
      LocalSpeechModelsSection(manager: manager, disabled: false).designRow()
    }
    .designPage()
    .navigationTitle("本地语音模型")
    .navigationBarTitleDisplayMode(.inline)
  }
}

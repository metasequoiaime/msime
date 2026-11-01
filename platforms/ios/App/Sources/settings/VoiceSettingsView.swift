import SwiftUI

/// 「语音输入」：设计稿里的精简页面（对应 Android `VoicePage`）。「识别」分组放识别语言、自动添加标点和长按空格说话的开关；「服务」分组通往完整的服务页（保留服务商配置、密钥、润色和 App 内录音）以及本地模型。
///
/// 「识别」分组末尾是点按开始的本地识别的停顿时间和判停灵敏度（#10310），与 Android `VoicePage`、鸿蒙语音页放在同一位置，分组说明讲它们的适用范围；存在共享文档的 `voice_input.pause_millis`、`stop_sensitivity` 里，只留在本机（`VoicePauseSettings`）；两行的标题和说明与鸿蒙语音页逐字相同；Android 的停顿时间说明多一句「锁定录音时由发送或取消结束」，因为只有 Android 能上滑锁定录音，其余与这里相同。
///
/// 识别语言存在共享文档的 `voice_input.language` 里，经 `VoicePolishSettings` 写入，`voice_input` 的其他字段保持不变。自动添加标点是服务页已经存在本 App defaults 里的豆包请求开关；只有豆包支持，其他识别服务下开关置灰。iOS 的语音入口是长按空格（`KeyboardLayoutPreference.spaceVoice`，本机 App Group）和「更多工具」里的「语音输入」磁贴，工具栏不放语音按钮，所以没有 Android「启动方式」那样的三选一；共享文档里的 `touch_voice_shortcut` 是 Android 的设置，这一页不读也不写，免得同步过去盖掉它。
struct VoiceSettingsView: View {
  @State private var settings = VoicePolishSettings(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var provider = CustomServiceConfiguration.load(.voice).voiceProvider
  @State private var punctuation = CustomServiceConfiguration.load(.voice).doubaoEnablePunctuation
  @State private var spaceVoice = KeyboardLayoutPreference.spaceVoice
  @State private var pause = VoicePauseSettings(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var saveFailed = false
  @State private var pauseSaveFailed = false

  /// 豆包的标点开关，与 `CustomServiceConfiguration` 读写的是同一个键。
  private static let punctuationKey = "service.voice.enable_punc"

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "识别", footer: saveFailed || pauseSaveFailed ? SettingsSaveFailure.message : Self.pauseFooter) {
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
          DesignDivider()
          DesignSliderRow(title: "本地停顿时间", subtitle: "本地识别出字后停顿这么久就结束录音；按住说话时由松手结束",
                          value: pauseSlider,
                          range: Double(VoicePauseSettings.pauseRange.lowerBound)...Double(VoicePauseSettings.pauseRange.upperBound),
                          step: Double(VoicePauseSettings.pauseStep), identifier: "voicePauseSlider") { "\(Int($0)) 毫秒" }
          DesignDivider()
          DesignSliderRow(title: "判停灵敏度", subtitle: "数值越高越容易判定说完、结束录音；调得太高，轻声说的字可能被当成停顿",
                          value: sensitivitySlider,
                          range: Double(VoicePauseSettings.sensitivityRange.lowerBound)...Double(VoicePauseSettings.sensitivityRange.upperBound),
                          step: 1, identifier: "voiceStopSensitivitySlider") { "\(Int($0))%" }
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
    pause = VoicePauseSettings(document)
  }

  /// 停顿时间滑块：拖动时每跨过一个步长写一次文档。
  private var pauseSlider: Binding<Double> {
    Binding(get: { Double(pause.pauseMilliseconds) }, set: { value in
      let step = VoicePauseSettings.pauseStep
      let rounded = Int((value / Double(step)).rounded()) * step
      guard rounded != pause.pauseMilliseconds, VoicePauseSettings.pauseRange.contains(rounded) else { return }
      var updated = pause
      updated.pauseMilliseconds = rounded
      savePause(updated)
    })
  }

  /// 判停灵敏度滑块：拖动时每跨过一个整数写一次文档。
  private var sensitivitySlider: Binding<Double> {
    Binding(get: { Double(pause.stopSensitivity) }, set: { value in
      let rounded = Int(value.rounded())
      guard rounded != pause.stopSensitivity, VoicePauseSettings.sensitivityRange.contains(rounded) else { return }
      var updated = pause
      updated.stopSensitivity = rounded
      savePause(updated)
    })
  }

  private func savePause(_ updated: VoicePauseSettings) {
    pause = updated
    pauseSaveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { updated.write(into: &$0) }
    if pauseSaveFailed { pause = VoicePauseSettings(MetasequoiaInputSessionBridge.loadSharedPreferences()) }
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

  /// 第一句与鸿蒙语音页相同。iOS 点按开始的听写来自「更多工具」里的「语音输入」；系统识别在 iOS 上不自己判停，和云服务一样点「说完了」结束，所以第二句与鸿蒙不同。
  static let pauseFooter = "这两项只用于点按开始的本地识别，只保存在本机，下次录音生效；判停灵敏度只对带语音活动检测的本地整段模型生效。系统识别和云服务点「说完了」结束。"

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

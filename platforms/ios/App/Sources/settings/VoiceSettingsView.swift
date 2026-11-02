import SwiftUI

/// 「语音输入」：设计稿里的精简页面（对应 Android `VoicePage`）。「识别」分组放识别语言、自动添加标点、语音标点和长按空格说话的开关；「服务」分组通往完整的服务页（保留服务商配置、密钥、润色和 App 内录音）以及本地模型。
///
/// 「识别」分组末尾是点按开始的本地识别的停顿时间和判停灵敏度（#10310），与 Android `VoicePage`、鸿蒙语音页放在同一位置，分组说明讲它们的适用范围；存在共享文档的 `voice_input.pause_millis`、`stop_sensitivity` 里，只留在本机（`VoicePauseSettings`）；两行的标题和说明与鸿蒙语音页逐字相同；Android 的停顿时间说明多一句「锁定录音时由发送或取消结束」，因为只有 Android 能上滑锁定录音，其余与这里相同。
///
/// 识别语言存在共享文档的 `voice_input.language` 里，经 `VoicePolishSettings` 写入，`voice_input` 的其他字段保持不变。自动添加标点是服务页已经存在本 App defaults 里的豆包请求开关；只有豆包支持，其他识别服务下开关置灰，说明指向「语音标点」。「语音标点」（#10595）写共享文档的 `voice_input.punctuation_mode`（`VoicePunctuation`，只改这一个键），键盘听写和 App 语音页在所有识别服务和润色之后、上屏或交给键盘之前按它处理；选项名称、说明与 Android `VoicePage` 逐字相同，`voice_input` 整组不进账号同步，只留在本机。iOS 的语音入口是长按空格（`KeyboardLayoutPreference.spaceVoice`，本机 App Group）和「更多工具」里的「语音输入」磁贴，工具栏不放语音按钮，所以没有 Android「启动方式」那样的三选一；共享文档里的 `touch_voice_shortcut` 是 Android 的设置，这一页不读也不写，免得同步过去盖掉它。
struct VoiceSettingsView: View {
  @State private var settings = VoicePolishSettings(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var provider = CustomServiceConfiguration.load(.voice).voiceProvider
  @State private var punctuation = CustomServiceConfiguration.load(.voice).doubaoEnablePunctuation
  @State private var punctuationMode = VoicePunctuation.mode(MetasequoiaInputSessionBridge.loadSharedPreferences())
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
                          subtitle: provider == .doubao ? nil : "只对豆包生效；其他识别服务的标点用下面的「语音标点」",
                          info: Self.doubaoPunctuationHint,
                          isOn: Binding(get: { provider == .doubao && punctuation }, set: savePunctuation))
            .disabled(provider != .doubao)
            .opacity(provider == .doubao ? 1 : 0.5)
            .accessibilityIdentifier("voicePunctuationToggle")
          DesignDivider()
          DesignSelectRow(title: "语音标点", info: Self.punctuationModeHint,
                          options: VoicePunctuation.modes.map {
                            DesignOption(title: VoicePunctuation.label($0), value: $0, identifier: "voicePunctuationMode_\($0)")
                          },
                          selection: Binding(get: { punctuationMode }, set: savePunctuationMode),
                          sheetTitle: "语音标点", sheetMessage: Self.punctuationModeSheetHint,
                          identifier: "voicePunctuationModePicker")
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
    punctuationMode = VoicePunctuation.mode(document)
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

  /// 只写 `voice_input.punctuation_mode`，下一次识别起作用；写失败时回读实际值。
  private func savePunctuationMode(_ mode: String) {
    saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { VoicePunctuation.write(mode, into: &$0) }
    if saveFailed { reload() } else { punctuationMode = mode }
  }

  /// 长按空格说话只存在本机 App Group，键盘下次出现时读到。
  private func saveSpaceVoice(_ enabled: Bool) {
    KeyboardLayoutPreference.spaceVoice = enabled
    spaceVoice = enabled
  }

  /// 与 Android `VoicePage` 的「自动添加标点」说明相同。
  static let doubaoPunctuationHint = "只决定豆包识别要不要在结果里加标点。本地模型、系统识别和其他服务自己会加标点，要去掉或替换它们，用下面的「语音标点」"

  /// 与 Android `VoicePage.PUNCTUATION_MODE_HINT` 相同。
  static let punctuationModeHint = "识别结果上屏前怎么处理标点，对所有识别服务和 AI 润色后的结果都生效。智能添加：识别出什么标点就上屏什么；省略句末标点：「你好。」上屏为「你好」；省略所有标点：「天啊！他是谁？」上屏为「天啊他是谁」；空格代替标点：上屏为「天啊 他是谁」。小数点、时间里的冒号和引号括号不受影响"

  /// 与 Android `VoicePage.PUNCTUATION_MODE_SHEET_HINT` 相同。
  static let punctuationModeSheetHint = "对所有识别服务和 AI 润色后的结果生效，下一次识别起作用"

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

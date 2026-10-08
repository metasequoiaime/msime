import SwiftUI

/// 「语音输入」：设计稿里的精简页面（对应 Android `VoicePage`）。「识别」分组放识别语言、自动添加标点和键盘启动语音输入的方式；「服务」分组通往完整的服务页（保留服务商配置、密钥、润色和 App 内录音）以及本地模型。
///
/// 识别语言存在共享文档的 `voice_input.language` 里，经 `VoicePolishSettings` 写入，`voice_input` 的其他字段保持不变。自动添加标点是服务页已经存在本 App defaults 里的豆包请求开关；只有豆包支持，其他识别服务下开关置灰。启动方式与 Android 的 `VoicePage` 一样没有自己的字段，由本机 App Group 的长按空格（`KeyboardLayoutPreference.spaceVoice`）和共享文档的工具栏语音按钮（`touch_voice_shortcut`）联合派生：长按空格优先，其次是工具栏按钮，都关时为「无」；选择时两个一起写。
struct VoiceSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var settings = VoicePolishSettings(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var provider = CustomServiceConfiguration.load(.voice).voiceProvider
  @State private var punctuation = CustomServiceConfiguration.load(.voice).doubaoEnablePunctuation
  @State private var trigger = VoiceTrigger(spaceVoice: KeyboardLayoutPreference.spaceVoice,
                                            voiceShortcut: KeyboardLayoutPreference.voiceShortcutEnabled)
  @State private var saveFailed = false

  /// 豆包的标点开关，与 `CustomServiceConfiguration` 读写的是同一个键。
  private static let punctuationKey = "service.voice.enable_punc"
  private static let saveFailure = "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "识别", footer: saveFailed ? Self.saveFailure : nil) {
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
          DesignSelectRow(title: "启动方式",
                          options: [DesignOption(title: "长按空格", value: VoiceTrigger.space, identifier: "voiceStartSpace"),
                                    DesignOption(title: "工具栏按钮", value: VoiceTrigger.toolbar, identifier: "voiceStartToolbar"),
                                    DesignOption(title: "无", value: VoiceTrigger.none, identifier: "voiceStartNone")],
                          selection: Binding(get: { trigger }, set: saveTrigger),
                          sheetTitle: "启动方式", identifier: "voiceStartPicker")
        }

        DesignGroup(title: "服务") {
          NavigationLink(destination: ServiceSettingsView(kind: .voice)) {
            DesignNavRowLabel(title: "识别服务", value: provider.title)
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("voiceServiceLink")
          DesignDivider()
          NavigationLink(destination: LocalSpeechModelsPage()) {
            DesignNavRowLabel(title: "本地语音模型", subtitle: "离线识别，录音不离开设备")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("localSpeechModelsLink")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("语音输入")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reload() }
    }
  }

  private func reload() {
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    settings = VoicePolishSettings(document)
    let configuration = CustomServiceConfiguration.load(.voice)
    provider = configuration.voiceProvider
    punctuation = configuration.doubaoEnablePunctuation
    trigger = VoiceTrigger(spaceVoice: KeyboardLayoutPreference.spaceVoice, voiceShortcut: Self.voiceShortcut(in: document))
  }

  /// 工具栏语音按钮以共享文档为准：从别的设备同步来、键盘还没打开过时，App Group 镜像里还是旧值。文档读不到或没有这一项时退回镜像。
  private static func voiceShortcut(in document: [String: Any]?) -> Bool {
    document?["touch_voice_shortcut"] as? Bool ?? KeyboardLayoutPreference.voiceShortcutEnabled
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

  /// 先写共享文档里的 `touch_voice_shortcut`，只改这一项：不经 `saveGeometry`，那样会拿 App Group 镜像里可能过期的间距和高度盖掉文档里的值。文档接受之后再写镜像和本机的长按空格开关，与 Android 的 `saveTrigger` 一样两个一起写；写入失败时页面回到已存的值。
  private func saveTrigger(_ next: VoiceTrigger) {
    let shortcut = next == .toolbar
    saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_voice_shortcut"] = shortcut }
    if !saveFailed {
      KeyboardLayoutPreference.voiceShortcutEnabled = shortcut
      KeyboardLayoutPreference.spaceVoice = next == .space
    }
    reload()
  }
}

/// 「语音输入」的启动方式，与 Android `VoicePage` 的 `TRIGGER_*` 相同。
enum VoiceTrigger: Hashable {
  case space, toolbar, none

  /// 由两个开关派生：长按空格优先，其次是工具栏按钮，都关时没有语音入口。
  init(spaceVoice: Bool, voiceShortcut: Bool) {
    self = spaceVoice ? .space : voiceShortcut ? .toolbar : .none
  }
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

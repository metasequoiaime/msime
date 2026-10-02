import AudioToolbox
import SwiftUI
import UIKit

struct SkinSettingsView: View {
  @EnvironmentObject private var navigation: AppNavigation
  @Environment(\.scenePhase) private var scenePhase
  @AppStorage(CustomKeyboardSkinStore.key, store: KeyboardFeedbackPreference.defaults)
  private var customSkinData = Data()
  @AppStorage(GlobalThemePreference.key, store: KeyboardFeedbackPreference.defaults)
  private var skin = GlobalThemeCatalog.systemId
  /// Every theme of the catalog as the shared document configures it, so the custom card shows the custom theme's base and design.
  @State private var themeCards: [KeyboardTheme] = GlobalThemeCatalog.ids.map { KeyboardTheme.resolve($0, document: nil) }
  @State private var customBase = GlobalThemeCatalog.systemId
  @State private var previewsNineKey = InputSchemePreference.scheme == .nineKey
  @State private var previewsDark = false
  @State private var savedDesigns = CustomSkinLibrary.designs.count
  @State private var themes: [String: String] = [:]
  @State private var themeSaveFailed = false
  @State private var skinSaveFailed = false
  @Environment(\.horizontalSizeClass) private var sizeClass
  @Environment(\.colorScheme) private var colorScheme
  /// The shared document as last read, for the candidate preview at the top.
  @State private var document: [String: Any]?

  var body: some View {
    Form {
      Section {
        CandidatePreviewCard(theme: selectedTheme, document: document, systemDark: colorScheme == .dark)
      }
      .listRowBackground(Color.clear)
      .listRowInsets(EdgeInsets())
      Section {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 12)], spacing: 12) {
          ForEach(themeCards, id: \.id) { themeCard($0) }
        }
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets())
      } header: {
        Text("皮肤")
      } footer: {
        Text(skinSaveFailed
          ? "主题没有保存，键盘可能正在写入同一份设置，请再试一次。"
          : "主题同时决定键盘和候选栏的配色，与电脑版同步。选择后预览立即更新，下次打开水杉键盘时应用。内置主题有固定的明暗，跟随系统和未设底色的自定义主题随「高级 · 明暗」里的键盘明暗切换。自定义主题当前以「\(GlobalThemeCatalog.title(customBase))」为底。")
      }
      Section {
        NavigationLink(destination: CustomSkinEditorView()) {
          SettingsRowLabel(title: "设计我的皮肤",
                           detail: savedDesigns == 0 ? "还没有命名保存的方案" : "本机保存了 \(savedDesigns) 套方案",
                           symbol: "paintbrush.pointed.fill")
        }.accessibilityIdentifier("customSkinEditorLink")
        Button { navigation.discoverSkins() } label: { Label("去社区找皮肤", systemImage: "square.grid.2x2") }
          .accessibilityIdentifier("skinCommunityLink")
      } header: {
        Text("自定义主题")
      }
      CustomThemeCandidateSection(onThemeChange: reloadThemes)
      Section("完整键盘预览") {
        Picker("键盘布局", selection: $previewsNineKey) {
          Text("26 键").tag(false)
          Text("9 键").tag(true)
        }.pickerStyle(.segmented).accessibilityIdentifier("skinPreviewLayout")
        KeyboardSkinPreview(skin: selectedTheme, nineKey: previewsNineKey)
          .id(customSkinData)
          .environment(\.colorScheme, previewScheme)
          .listRowInsets(EdgeInsets(top: 8, leading: 8, bottom: 8, trailing: 8))
        Toggle("预览深色外观", isOn: $previewsDark)
          .accessibilityIdentifier("skinPreviewDark")
      }
      appearanceSection
    }
    .navigationTitle("主题")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear { savedDesigns = CustomSkinLibrary.designs.count; reloadThemes() }
    .onChange(of: scenePhase) { if $0 == .active { savedDesigns = CustomSkinLibrary.designs.count; reloadThemes() } }
  }

  /// One card of the design's theme grid (dc.html L769-783): a 72pt swatch holding a candidate panel, then the name and 使用中 on the selected card, which also carries the accent ring.
  private func themeCard(_ option: KeyboardTheme) -> some View {
    let selected = skin == option.id
    let swatch = swatchColors(option)
    return Button {
      skinSaveFailed = !GlobalThemePreference.save(option.id)
      if !skinSaveFailed { reloadThemes() }
    } label: {
      VStack(alignment: .leading, spacing: 8) {
        ZStack {
          if swatch.preview == nil {
            KeyboardSkinBackdrop(skin: option)
          } else {
            Color(uiColor: swatch.background)
          }
          HStack(spacing: 8) {
            Text("1 候选").foregroundStyle(Color(uiColor: swatch.accent))
            Text("2 侯选").foregroundStyle(Color(uiColor: swatch.text))
          }
          .font(.system(size: 13)).lineLimit(1).fixedSize()
          .padding(.vertical, 5).padding(.horizontal, 9)
          .background(RoundedRectangle(cornerRadius: 4).fill(Color(uiColor: swatch.panel)))
          .shadow(color: .black.opacity(0.2), radius: 4, y: 2)
        }
        .frame(height: 72).frame(maxWidth: .infinity)
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        .environment(\.colorScheme, cardScheme(option))
        HStack {
          Text(option.title).foregroundStyle(.primary)
          Spacer(minLength: 4)
          if selected { Text("使用中").font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.accent) }
        }
        .font(.system(size: 13)).padding(.horizontal, 2)
      }
      .padding(8)
      .background(RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous).fill(MetasequoiaTheme.surface))
      .overlay {
        if selected {
          RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous).strokeBorder(MetasequoiaTheme.accent, lineWidth: 2)
        }
      }
      .contentShape(RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
    }
    .buttonStyle(.plain)
    .accessibilityElement(children: .ignore)
    .accessibilityLabel(option.title)
    .accessibilityHint(description(option))
    .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    .accessibilityIdentifier("skin_\(option.id)")
    .accessibilityValue(selected ? "已选择" : "未选择")
  }

  /// A built-in theme shows the catalog's preview colours; `system` and the custom theme have none, so they show their keyboard colours (and the design's backdrop) in the mode they preview in.
  private func swatchColors(_ option: KeyboardTheme) -> (preview: GlobalThemeEntry.Preview?, background: UIColor, panel: UIColor, accent: UIColor, text: UIColor) {
    if !option.isCustom, let preview = GlobalThemeCatalog.entry(option.id)?.preview {
      return (preview, preview.background, preview.panel, preview.accent, preview.text)
    }
    let traits = UITraitCollection(userInterfaceStyle: cardScheme(option) == .dark ? .dark : .light)
    return (nil, option.background.resolvedColor(with: traits), option.keyBackground.resolvedColor(with: traits),
            option.accent.resolvedColor(with: traits), option.keyForeground.resolvedColor(with: traits))
  }

  private func cardScheme(_ option: KeyboardTheme) -> ColorScheme {
    switch option.appearance {
    case .dark: .dark
    case .light: .light
    default: previewScheme
    }
  }

  /// 「键盘明暗」 from the shared document (see KeyboardAppearancePreference). iPad lists the panels beside the keyboard; the phone folds them away, since most people only ever set the keyboard.
  private var appearanceSection: some View {
    Section {
      Picker("颜色模式", selection: theme(AppAppearancePreference.globalKey, fallback: "system")) {
        ForEach(AppAppearancePreference.globalOptions, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier("globalTheme")
      Picker("设置界面", selection: theme(AppAppearancePreference.settingsKey)) {
        ForEach(AppAppearancePreference.settingsOptions, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier("settingsTheme")
      Picker("键盘", selection: theme(KeyboardAppearancePreference.keyboardKey)) {
        ForEach(KeyboardAppearancePreference.options, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier("keyboardTheme")
      if sizeClass == .regular {
        panelPickers
      } else {
        DisclosureGroup("面板明暗") { panelPickers }.accessibilityIdentifier("keyboardPanelThemes")
      }
    } header: {
      Text("高级 · 明暗")
    } footer: {
      Text(themeSaveFailed
        ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
        : "与电脑版的颜色模式、设置界面、屏幕键盘、手写、表情和语音主题同步。颜色模式是各处选“跟随”时的默认值；设置界面就是这个 App，立即生效。键盘选“跟随系统”时先看颜色模式，再跟随当前 App 的外观；面板选“跟随键盘”时和键盘一致。键盘和面板下次打开水杉键盘时应用。")
    }
  }

  private var panelPickers: some View {
    ForEach(KeyboardAppearancePreference.panels, id: \.key) { panel in
      Picker(panel.title, selection: theme(panel.key)) {
        ForEach(KeyboardAppearancePreference.panelOptions, id: \.id) { Text($0.title).tag($0.id) }
      }.accessibilityIdentifier(panel.key)
    }
  }

  private func theme(_ key: String, fallback: String = "follow") -> Binding<String> {
    Binding(get: { themes[key] ?? fallback }, set: { value in
      themes[key] = value
      themeSaveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = value }
      if themeSaveFailed { reloadThemes() }
      if key == AppAppearancePreference.globalKey || key == AppAppearancePreference.settingsKey {
        NotificationCenter.default.post(name: AppAppearancePreference.didChange, object: nil)
      }
      // The preview shows what the keyboard will draw, so an explicit keyboard theme turns it to match.
      if key == KeyboardAppearancePreference.keyboardKey, value != "follow" { previewsDark = value == "dark" }
    })
  }

  private var selectedTheme: KeyboardTheme {
    themeCards.first { $0.id == skin } ?? KeyboardTheme.resolve(skin, document: nil)
  }

  /// A theme with a fixed mode previews in it; `system` and a custom theme without a base follow the preview toggle.
  private var previewScheme: ColorScheme {
    switch selectedTheme.appearance {
    case .dark: .dark
    case .light: .light
    default: previewsDark ? .dark : .light
    }
  }

  private func description(_ theme: KeyboardTheme) -> String {
    if theme.id == GlobalThemeCatalog.systemId { return "iOS 原生键盘配色，随明暗切换" }
    if theme.isCustom {
      let base = GlobalThemeCatalog.title(customBase)
      return theme.design == nil ? "以「\(base)」为底，外部皮肤和候选颜色在下方「外部皮肤与候选颜色」里调整" : "我的键盘设计，以「\(base)」为底"
    }
    return theme.appearance == .dark ? "固定深色，键盘和候选栏同一套配色" : "固定浅色，键盘和候选栏同一套配色"
  }

  private func reloadThemes() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    document = preferences
    themeCards = GlobalThemeCatalog.ids.map { KeyboardTheme.resolve($0, document: preferences) }
    customBase = GlobalThemePreference.base(in: preferences)
    let keys = [AppAppearancePreference.settingsKey, KeyboardAppearancePreference.keyboardKey] + KeyboardAppearancePreference.panels.map(\.key)
    themes = keys.reduce(into: [:]) { themes, key in themes[key] = preferences[key] as? String ?? "follow" }
    themes[AppAppearancePreference.globalKey] = preferences[AppAppearancePreference.globalKey] as? String ?? "system"
  }
}

/// 输入习惯 is saved into the shared preference document (see InputHabitPreference), which the keyboard reloads the next time it appears; phone and iPad show the same controls.
struct DictionarySettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var habits = InputHabitPreference.mirrored
  @State private var saveFailed = false
  private var manifest: [String: Any] {
    guard let url = Bundle.main.url(forResource: "dictionary-manifest", withExtension: "json"),
          let data = try? Data(contentsOf: url),
          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
    return object
  }
  var body: some View {
    Form {
      Section {
        Toggle("学习常用词", isOn: habit(\.learning))
          .accessibilityIdentifier("dictionaryLearningToggle")
        Picker("调频方式", selection: habit(\.frequencyMode)) {
          ForEach(FrequencyAdjustmentMode.allCases, id: \.self) { mode in
            Text(mode.title).tag(mode)
          }
        }
        .accessibilityIdentifier("frequencyAdjustmentModePicker")
        .disabled(!habits.learning)
        Picker("触发频次", selection: habit(\.triggerCount)) {
          ForEach(FrequencyAdjustmentPreference.countRange, id: \.self) { Text("\($0)").tag($0) }
        }
        .accessibilityIdentifier("frequencyAdjustmentTriggerPicker")
        .disabled(!habits.learning || habits.frequencyMode == .disabled)
        Picker("线性调频步长", selection: habit(\.linearStep)) {
          ForEach(FrequencyAdjustmentPreference.countRange, id: \.self) { Text("\($0)").tag($0) }
        }
        .accessibilityIdentifier("frequencyAdjustmentLinearStepPicker")
        .disabled(!habits.learning || habits.frequencyMode != .linear)
        Toggle("显示英文释义", isOn: habit(\.glossEnabled))
          .accessibilityIdentifier("candidateGlossToggle")
        if habits.glossEnabled {
          Picker("第一种语言", selection: habit(\.primaryLanguage)) {
            ForEach(Array(CandidateTranslationPreference.languages.enumerated()), id: \.offset) { index, language in
              Text(language.title).tag(index)
            }
          }.accessibilityIdentifier("candidateTranslationPrimaryPicker")
          Picker("第二种语言", selection: habit(\.secondaryLanguage)) {
            Text("不显示").tag(-1)
            ForEach(Array(CandidateTranslationPreference.languages.enumerated()), id: \.offset) { index, language in
              Text(language.title).tag(index).disabled(index == habits.primaryLanguage)
            }
          }.accessibilityIdentifier("candidateTranslationSecondaryPicker")
          Toggle("联网补充释义", isOn: habit(\.onlineTranslations))
            .accessibilityIdentifier("candidateTranslationOnline")
          NavigationLink(destination: TranslationProviderSettingsView()) {
            Label("翻译服务", systemImage: "globe")
          }.accessibilityIdentifier("translationProviderLink")
          Text("离线词库只有英汉两个方向，其余语言以及词库答不上来的词要联网才有。只有在「翻译服务」里选了服务，键盘才会把当前页的中文候选词发给它，需要允许键盘完全访问。")
            .font(.footnote).foregroundStyle(.secondary)
        }
        if saveFailed {
          Text("设置没有保存，键盘可能正在写入同一份设置，请再试一次。")
            .font(.footnote).foregroundStyle(.red)
        }
      } header: {
        Text("输入习惯")
      } footer: {
        Text("开启后，引擎按所选调频方式调整候选排序，并学习支持的拼音组词。不调频保留词库原有顺序，只学习新词；一次置顶移到首位；折半移到当前名次与首位之间；线性按固定步数前移；一次置前把前五名前进一位、更靠后的提到第五名。触发频次是同一候选累计选中多少次后才调整一次。英文释义来自随键盘打包的离线词库，不联网。学习记录仅保存在设备上。关闭后停止新增学习，不清除已有记录；正在输入的内容结束后生效。")
      }
      Section {
        NavigationLink(destination: PersonalDictionaryView()) {
          Label("个人词库", systemImage: "text.badge.plus")
        }.accessibilityIdentifier("personalDictionaryLink")
        NavigationLink(destination: VocabularyReviewSettingsView()) {
          Label("背单词", systemImage: "character.book.closed")
        }.accessibilityIdentifier("vocabularyReviewSettingsLink")
      }
      Section("已安装词库") {
        Label("内置离线多方案词库", systemImage: "checkmark.circle.fill")
        Text("支持全拼 26 键、全拼 9 键、小鹤／自然码／微软／首道双拼、86 五笔、日语罗马字、韩语两套式、粤拼、大千注音（繁体输出）和越南语 Telex／VNI；粤拼和注音需要安装包里带有对应的语言词库，默认不启用，可在方案设置里打开；提供英文补全、快捷短语、表情及颜文字。")
          .foregroundStyle(.secondary)
        HStack {
          Text("更新方式")
          Spacer()
          Text("随 App 更新").foregroundStyle(.secondary)
        }
      }
      Section("词库信息") {
        if let profile = manifest["profile"] as? String {
          HStack { Text("规格"); Spacer(); Text(profile).foregroundStyle(.secondary) }
        }
        if let source = manifest["source"] as? [String: Any], let commit = source["commit"] as? String {
          VStack(alignment: .leading, spacing: 6) {
            Text("词库版本")
            Text(String(commit.prefix(12))).font(.system(.footnote, design: .monospaced))
              .foregroundStyle(.secondary)
          }
        }
        Text("词库保存在设备上，日常输入不需要联网。已启用日语整句转换，支持罗马字输入、假名及汉字混合候选。")
      }
      Section("候选词管理") {
        Label("长按候选词", systemImage: "hand.tap")
        Text("全拼 26 键、九键、双拼和五笔支持长按候选词：优先显示、固定到前五位中的某一位、取消固定或删除词条。删除需要再次确认，单个汉字由引擎保护。")
          .foregroundStyle(.secondary)
        Text("日语、韩语、粤拼、注音、越南语和本地工具暂不支持候选词管理。第三方词库文件（词在前、编码在前或 Rime 格式）在「个人词库」的「导入个人词库」里导入。")
          .foregroundStyle(.secondary)
      }
    }
    .navigationTitle("词库")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  private func reload() {
    habits = InputHabitPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
  }

  /// A binding that saves one field; a failed save reloads so the control shows what is actually stored.
  private func habit<Value>(_ field: WritableKeyPath<InputHabitSettings, Value>) -> Binding<Value> {
    Binding(get: { habits[keyPath: field] }, set: { value in
      if let saved = InputHabitPreference.update({ $0[keyPath: field] = value }) {
        habits = saved
        saveFailed = false
      } else {
        saveFailed = true
        reload()
      }
    })
  }
}

struct ServiceSettingsView: View {
  let kind: CustomServiceKind
  @Environment(\.scenePhase) private var scenePhase
  @State private var configuration: CustomServiceConfiguration
  @State private var providerDrafts: [AIProviderPreset: CustomServiceConfiguration] = [:]
  @State private var voiceProviderDrafts: [VoiceProviderPreset: CustomServiceConfiguration] = [:]
  @State private var showsProviders = false
  @State private var editsCustomModel = false
  @State private var fetchedModels: [String]?
  @State private var modelStatus = ""
  @State private var fetchingModels = false
  @State private var testingConnection = false
  @State private var token = ""
  @State private var keyboardAIEnabled = KeyboardAIService.configuration() != nil
  @State private var aiCandidatesEnabled = AICandidatePreference.isEnabled(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var aiCandidateLimit = AICandidatePreference.limit(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var input = ""
  @State private var voiceTransfer: VoiceTextHandoff?
  @State private var output = ""
  @State private var status = ""
  @State private var busy = false
  @State private var operation: Task<Void, Never>?
  @State private var requestID = UUID()
  @State private var voiceGeneration: UInt64 = 0
  @State private var voiceSettings = VoicePolishSettings(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var voiceSettingsSaveFailed = false
  @State private var polishService = VoicePolishService.load()
  /// The recognized text before polishing, so the user can take it instead of the polished result.
  @State private var transcript = ""
  @StateObject private var recorder = VoiceRecorder()
  /// Doubao's partial result while a live recording runs, and whether that recording's request is still open.
  @State private var liveText = ""
  @State private var recognizesLive = false
  /// The downloaded on-device models, for the 本地模型 provider.
  @StateObject private var localModels = LocalSpeechModelManager()

  init(kind: CustomServiceKind) {
    self.kind = kind
    _configuration = State(initialValue: CustomServiceConfiguration.load(kind))
    #if DEBUG && targetEnvironment(simulator)
    if kind == .voice && ProcessInfo.processInfo.arguments.contains("-voiceResultFixture") {
      _output = State(initialValue: "语音交接测试。")
    }
    #endif
  }

  var body: some View {
    Form {
      providerSection
      if kind == .ai || !configuration.voiceProvider.isOnDevice {
        configurationSection
      }
      if kind == .voice && configuration.voiceProvider == .local {
        LocalSpeechModelsSection(manager: localModels, disabled: busy || recorder.isRecording || recorder.isPreparing)
      }

      if kind == .ai {
        Section {
          Toggle("在键盘中启用 AI", isOn: $keyboardAIEnabled)
            .accessibilityIdentifier("keyboardAIEnabled")
            .onChange(of: keyboardAIEnabled) { enabled in
              if !enabled {
                do { try KeyboardAIService.disable() } catch { status = error.localizedDescription }
                publishAICandidates()
              }
            }
        } footer: {
          Text("开启后点击“保存配置”，即可在键盘“更多 → AI 润色”或“高情商回复”方案中使用。需要允许完全访问；每次发送前会预览文字。")
        }
        if keyboardAIEnabled {
          Section {
            Toggle("候选栏 AI 候选", isOn: $aiCandidatesEnabled)
              .accessibilityIdentifier("aiCandidatesEnabled")
              .onChange(of: aiCandidatesEnabled) { _ in publishAICandidates() }
            if aiCandidatesEnabled {
              Stepper("候选数量：\(aiCandidateLimit)", value: $aiCandidateLimit, in: AICandidatePreference.limits)
                .accessibilityIdentifier("aiCandidateLimit")
                .onChange(of: aiCandidateLimit) { _ in publishAICandidates() }
              NavigationLink("提示词") { AICandidatePromptView() }
                .accessibilityIdentifier("aiCandidatePrompt")
            }
          } footer: {
            Text("打全拼时把已输入的拼音和前文发给上面保存的服务，把联想结果补在候选栏里，与云候选相同。密钥只留在本机钥匙串，不写入可同步的设置。")
          }
        }
        Section("AI 润色") {
          TextEditor(text: $input).frame(minHeight: 100)
            .accessibilityLabel("待润色文字").accessibilityIdentifier("aiInputText")
          Button(busy ? "正在处理…" : "发送并润色") { send() }
            .disabled(busy || input.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
      } else {
        Section {
          Button(recorder.isRecording ? "停止录音" : "开始录音") {
            if recorder.isRecording { recorder.stop() }
            else {
              let onDevice = recognizesOnDevice
              if onDevice && configuration.voiceProvider == .local && localModels.selectedDirectory == nil {
                status = "请先在“本地模型”中下载并选用一个模型。"
                return
              }
              let live = streamsLive || onDevice
              if live { guard save() else { return } }
              operation = Task {
                // The recording session silences system sounds, so the start cue has to finish before it opens.
                if voiceSettings.soundEnabled && voiceSettings.startSound { await VoiceCue.playStart() }
                do {
                  if live {
                    if let pcm = try await recorder.startStreaming(quietensOthers: voiceSettings.muteOthers) {
                      if onDevice { recognizeOnDevice(pcm) } else { recognizeLive(pcm) }
                    }
                  } else {
                    try await recorder.start(quietensOthers: voiceSettings.muteOthers)
                  }
                }
                catch is CancellationError {} catch { status = error.localizedDescription }
              }
            }
          }
          .disabled(busy || recorder.isPreparing || (recognizesLive && !recorder.isRecording))
          if recorder.isRecording {
            Label("正在录音，最长 60 秒", systemImage: "mic.fill").foregroundStyle(.red)
            VoiceLevelWaveform(levels: recorder.levels)
          }
          if recognizesLive {
            Text(liveText.isEmpty ? (recorder.isRecording ? "正在聆听…" : "正在识别…") : liveText)
              .foregroundStyle(liveText.isEmpty ? .secondary : .primary)
              .accessibilityIdentifier("voiceLiveText")
          }
          if recorder.audio != nil && !recorder.isRecording && !recognizesLive {
            Text(recognizesOnDevice ? "录音已保留在本机，可以重新识别。" : "录音已准备好，尚未上传。")
            Button(busy ? "正在识别…" : recognizesOnDevice ? "重新识别录音" : "发送录音并识别") { send() }.disabled(busy)
            Button("删除录音", role: .destructive) { recorder.discard() }.disabled(busy)
          }
        } header: {
          Text("语音转文字")
        } footer: {
          Text(recognizesOnDevice
            ? "在水杉 App 中录音，由本机识别，边说边显示文字，再将结果发送到键盘或复制。iOS 键盘扩展不能直接录音。"
            : "在水杉 App 中录音识别，再将结果发送到键盘或复制。iOS 键盘扩展不能直接录音。")
        }
        voiceOptionsSection
        voicePolishSection
      }
      if busy {
        Button("取消请求") { cancelRequest(); status = "已取消" }
      }
      if !status.isEmpty {
        Section { Text(status).accessibilityIdentifier("serviceStatus") }
      }
      if !output.isEmpty {
        Section("结果") {
          Text(output).textSelection(.enabled)
          if kind == .voice && !transcript.isEmpty && transcript != output {
            Button("改用识别原文") { output = transcript; status = "已改用识别原文" }
              .accessibilityIdentifier("useVoiceTranscript")
          }
          Button("复制结果") { UIPasteboard.general.string = output; status = "已复制" }
          if kind == .voice {
            Button(voiceTransfer == nil ? "发送到键盘" : "更新待插入结果") {
              do {
                voiceTransfer = try VoiceTextHandoffStore().save(output)
                status = "已发送到本机键盘。返回目标 App，打开键盘“更多 → 语音结果”，确认后插入。"
              } catch { status = error.localizedDescription }
            }.accessibilityIdentifier("sendVoiceToKeyboard")
          }
        }
      }
      if kind == .voice, let voiceTransfer {
        Section {
          Text(voiceTransfer.text).lineLimit(3)
          Text("有效至 \(voiceTransfer.expiresAt.formatted(date: .omitted, time: .shortened))")
            .font(.caption).foregroundStyle(.secondary)
          Button("清除待插入结果", role: .destructive) {
            do {
              try VoiceTextHandoffStore().discard(voiceTransfer.id)
              self.voiceTransfer = nil
              status = "已清除待插入结果"
            } catch { status = error.localizedDescription; refreshVoiceTransfer() }
          }.accessibilityIdentifier("clearVoiceTransfer")
        } header: { Text("等待键盘插入") } footer: {
          Text("需允许键盘完全访问。仅在本机共享最新一条文字，10 分钟内有效；离开页面仍会保留，点击插入后移除。")
        }
      }
      Section {
        Text(kind == .ai
          ? "仅在点击发送时，将上方文字发送到你配置的服务。键盘日常输入不会自动上传。"
          : configuration.voiceProvider == .local
          ? "录音只在本机识别，不会上传。离开页面会清除本地录音。"
          : configuration.voiceProvider == .system
          ? "录音交给 iOS 的语音识别；设备支持当前语言的本机识别时不离开设备，否则按系统设置由 Apple 处理。离开页面会清除本地录音。"
          : "仅在点击发送时，将本次录音发送到你配置的服务。离开页面会清除本地录音。")
          .font(.footnote).foregroundStyle(.secondary)
      }
    }
    .navigationTitle(kind.title)
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      ToolbarItemGroup(placement: .keyboard) {
        Spacer()
        Button("完成") {
          UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        }.accessibilityIdentifier("serviceDismissKeyboard")
      }
    }
    .tint(Color(uiColor: MetasequoiaTheme.forestUIColor))
    .sheet(isPresented: $showsProviders) {
      ProviderPickerView(options: providerOptions, selected: selectedProviderID) { id in
        if kind == .ai, let provider = AIProviderPreset(rawValue: id) { selectProvider(provider) }
        if kind == .voice, let provider = VoiceProviderPreset(rawValue: id) { selectVoiceProvider(provider) }
      }
    }
    .onChange(of: configuration.endpoint) { _ in fetchedModels = nil; modelStatus = "" }
    .onChange(of: voiceSettings) { _ in
      guard kind == .voice else { return }
      voiceSettingsSaveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { voiceSettings.write(into: &$0) }
    }
    .onChange(of: recorder.isRecording) { recording in
      // Also covers the recorder stopping itself at the 60-second limit.
      if !recording && voiceSettings.soundEnabled && voiceSettings.endSound { VoiceCue.playEnd() }
    }
    .task {
      guard kind == .voice else { return }
      while !Task.isCancelled {
        refreshVoiceTransfer()
        do { try await Task.sleep(nanoseconds: 2_000_000_000) } catch { break }
      }
    }
    .onDisappear { cancelAndClear() }
    .onChange(of: scenePhase) { phase in
      if phase == .background { cancelAndClear() }
      if phase == .active && kind == .voice { refreshVoiceTransfer() }
    }
  }

  private var voiceOptionsSection: some View {
    Section {
      Picker("识别语言", selection: $voiceSettings.language) {
        ForEach(VoicePolishSettings.languages, id: \.id) { Text($0.title).tag($0.id) }
      }
      .accessibilityIdentifier("voiceLanguage")
      if configuration.voiceProvider == .doubao {
        Toggle("边说边识别", isOn: $voiceSettings.streamLive)
          .accessibilityIdentifier("voiceStreamLive")
      }
      Toggle("录音时暂停其他声音", isOn: $voiceSettings.muteOthers)
        .accessibilityIdentifier("voiceMuteOthers")
      Toggle("录音提示音", isOn: $voiceSettings.soundEnabled)
        .accessibilityIdentifier("voiceSoundEnabled")
      // 和桌面「开始录音提示音」「结束录音提示音」同两个键。总开关关着时两项都不响,收起来免得看着像还开着。
      if voiceSettings.soundEnabled {
        Toggle("开始录音时", isOn: $voiceSettings.startSound)
          .padding(.leading, 16).accessibilityIdentifier("voiceStartSound")
        Toggle("结束录音时", isOn: $voiceSettings.endSound)
          .padding(.leading, 16).accessibilityIdentifier("voiceEndSound")
      }
    } footer: {
      Text(configuration.voiceProvider == .local
        ? "本地模型边说边显示文字；“自动识别”时由模型判断中文、英文、粤语等语言。开始和结束录音时各有一声系统提示音，可以分别关掉。打开“录音时暂停其他声音”会让正在播放的音乐和视频在录音期间停下；关着时它们继续播放，但声音可能被一起录进去。"
        : configuration.voiceProvider == .system
        ? "系统语音识别按这里的语言识别；选“自动识别”时使用 iPhone 的首选语言。开始和结束录音时各有一声系统提示音，可以分别关掉。打开“录音时暂停其他声音”会让正在播放的音乐和视频在录音期间停下；关着时它们继续播放，但声音可能被一起录进去。"
        : configuration.voiceProvider == .doubao
        ? "豆包自动判断语言，不使用这里的选择。边说边识别时录音同步发给豆包，结果随说随显示，停止录音即得到结果；关掉则录完再发送。开始和结束录音时各有一声系统提示音，可以分别关掉。打开“录音时暂停其他声音”会让正在播放的音乐和视频在录音期间停下；关着时它们继续播放，但声音可能被一起录进去。"
        : configuration.voiceProvider == .siliconFlow
        ? "当前服务自动判断语言，不使用这里的选择。开始和结束录音时各有一声系统提示音，可以分别关掉。打开“录音时暂停其他声音”会让正在播放的音乐和视频在录音期间停下；关着时它们继续播放，但声音可能被一起录进去。"
        : "识别语言随录音一起发送；选“自动识别”时由服务判断。开始和结束录音时各有一声系统提示音，可以分别关掉。打开“录音时暂停其他声音”会让正在播放的音乐和视频在录音期间停下；关着时它们继续播放，但声音可能被一起录进去。")
    }
  }

  private var voicePolishSection: some View {
    Section {
      Toggle("识别后自动润色", isOn: $voiceSettings.polishEnabled)
        .accessibilityIdentifier("voicePolishEnabled")
      if voiceSettings.polishEnabled {
        Picker("润色方式", selection: Binding(get: { voiceSettings.promptID }, set: { voiceSettings.select($0) })) {
          ForEach(VoicePolishSettings.presets, id: \.id) { Text($0.title).tag($0.id) }
        }
        .accessibilityIdentifier("voicePolishPreset")
        if let slot = voiceSettings.customSlot {
          TextEditor(text: $voiceSettings.customPrompts[slot]).frame(minHeight: 100)
            .accessibilityLabel("自定义润色提示词").accessibilityIdentifier("voicePolishCustomPrompt")
        } else {
          // The desktop's prompt box: a built-in preset can be edited too, and the edit is kept until 恢复默认 or another preset is picked.
          DisclosureGroup(voiceSettings.legacyPrompt.isEmpty ? "查看或修改提示词" : "提示词（已修改）") {
            TextEditor(text: $voiceSettings.presetPromptText).font(.footnote).frame(minHeight: 140)
              .accessibilityLabel("润色提示词").accessibilityIdentifier("voicePolishPresetPrompt")
            if !voiceSettings.legacyPrompt.isEmpty {
              Button("恢复默认") { voiceSettings.legacyPrompt = "" }
                .accessibilityIdentifier("voicePolishPromptReset")
            }
          }
          .accessibilityIdentifier("voicePolishPromptDisclosure")
        }
        NavigationLink {
          VoicePolishServiceView(service: $polishService)
        } label: {
          LabeledContent("润色服务", value: polishService.separate ? polishService.provider.title : "跟随 AI 设置")
        }
        .accessibilityIdentifier("voicePolishService")
      }
      if voiceSettingsSaveFailed {
        Text("未能保存语音设置，请重试。").foregroundStyle(.red)
      }
    } header: {
      Text("识别后润色")
    } footer: {
      Text(polishService.separate
        ? "识别完成后把文字发给单独设置的润色服务整理，可随时改用识别原文。自定义提示词留空时使用“精炼整理”；内置方式的提示词也可以修改，换一种方式或点“恢复默认”即回到原文。"
        : CustomServiceConfiguration.load(.ai).endpoint.isEmpty
        ? "润色使用“AI 设置”里保存的服务，目前尚未设置；也可以在“润色服务”里单独设置。识别结果会原样保留。"
        : "识别完成后把文字发给“AI 设置”里保存的服务整理，可随时改用识别原文。自定义提示词留空时使用“精炼整理”；内置方式的提示词也可以修改，换一种方式或点“恢复默认”即回到原文。")
    }
  }

  private func refreshVoiceTransfer() {
    do { voiceTransfer = try VoiceTextHandoffStore().read() }
    catch { status = error.localizedDescription }
  }

  private var selectedProviderID: String {
    kind == .ai ? configuration.provider.rawValue : configuration.voiceProvider.rawValue
  }

  private var providerOptions: [ProviderOption] {
    if kind == .ai {
      return AIProviderPreset.allCases.map { ProviderOption(id: $0.rawValue, title: $0.title, endpoint: $0.endpoint) }
    }
    return VoiceProviderPreset.allCases.map { provider in
      let note: String? = switch provider {
      case .local: "在本机识别，录音不离开设备"
      case .system: "使用 iOS 自带的语音识别"
      default: nil
      }
      return ProviderOption(id: provider.rawValue, title: provider.title, endpoint: provider.endpoint, note: note)
    }
  }

  private var providerSection: some View {
    Section {
      Button { showsProviders = true } label: {
        HStack(spacing: 14) {
          ProviderIcon(id: selectedProviderID, size: 48)
          VStack(alignment: .leading, spacing: 4) {
            Text(kind == .ai ? configuration.provider.title : configuration.voiceProvider.title)
              .font(.headline).foregroundStyle(.primary)
            Text("切换服务商").font(.subheadline).foregroundStyle(.secondary)
          }
          Spacer()
          Image(systemName: "chevron.up.chevron.down").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
        }.contentShape(Rectangle()).padding(.vertical, 6)
      }
      .buttonStyle(.plain)
      .accessibilityIdentifier(kind == .ai ? "aiProviderPicker" : "voiceProviderPicker")
      if let documentation = kind == .ai ? configuration.provider.documentation : configuration.voiceProvider.documentation {
        Link(destination: documentation) {
          Label("接入说明与 API Key", systemImage: "arrow.up.right.square")
            .font(.subheadline)
        }
      }
    } footer: {
      Text(kind == .voice && configuration.voiceProvider.isOnDevice
        ? "在本机识别，不需要 API Key。"
        : "选择服务商后自动填入接口和模型，填写对应 API Key 即可使用。")
    }
    .disabled(busy || recorder.isRecording || recorder.isPreparing)
  }

  private var presetModels: [String] {
    fetchedModels ?? (kind == .ai ? configuration.provider.models : configuration.voiceProvider.models)
  }

  private var usesCustomModel: Bool {
    editsCustomModel || !presetModels.contains(configuration.model)
  }

  private var modelSelection: some View {
    VStack(alignment: .leading, spacing: 8) {
      Text("模型").font(.caption).foregroundStyle(.secondary)
      if !presetModels.isEmpty {
        Menu {
          ForEach(presetModels, id: \.self) { model in
            Button {
              configuration.model = model
              editsCustomModel = false
            } label: {
              if configuration.model == model && !usesCustomModel {
                Label(model, systemImage: "checkmark")
              } else { Text(model) }
            }
          }
          Divider()
          Button("自定义模型…") { editsCustomModel = true }
        } label: {
          HStack {
            Text(usesCustomModel ? "自定义模型" : configuration.model)
              .multilineTextAlignment(.leading)
            Spacer(minLength: 8)
            Image(systemName: "chevron.down").font(.caption.weight(.semibold))
          }.contentShape(Rectangle())
        }
        .accessibilityIdentifier("serviceModelPicker")
        .accessibilityLabel("模型")
        .accessibilityValue(configuration.model)
      }
      if presetModels.isEmpty || usesCustomModel {
        TextField("输入模型名称", text: $configuration.model)
          .textInputAutocapitalization(.never).autocorrectionDisabled()
          .accessibilityIdentifier("serviceModel")
      }
    }.padding(.vertical, 4)
  }

  private var configurationSection: some View {
      Section {
        VStack(alignment: .leading, spacing: 8) {
          Text("接口地址").font(.caption).foregroundStyle(.secondary)
          TextField(kind.example, text: $configuration.endpoint)
          .keyboardType(.URL).textInputAutocapitalization(.never).autocorrectionDisabled()
          .accessibilityLabel("API 接口地址").accessibilityIdentifier("serviceEndpoint")
          .disabled(kind == .ai ? configuration.provider != .custom : configuration.voiceProvider != .custom)
        }.padding(.vertical, 4)
        VStack(alignment: .leading, spacing: 8) {
          Label("API Key", systemImage: "key.horizontal").font(.caption).foregroundStyle(.secondary)
          SecureField("留空保留已保存密钥", text: Binding(get: { token }, set: { value in
            token = value; fetchedModels = nil; modelStatus = ""
          }))
          .textInputAutocapitalization(.never).autocorrectionDisabled()
          .accessibilityIdentifier("serviceToken")
        }.padding(.vertical, 4)
        if kind == .voice && configuration.voiceProvider == .doubao {
          VStack(alignment: .leading, spacing: 8) {
            Text("Doubao App Key").font(.caption).foregroundStyle(.secondary)
            SecureField("可选", text: $configuration.voiceAppKey)
              .textInputAutocapitalization(.never).autocorrectionDisabled()
              .accessibilityIdentifier("doubaoAppKey")
            Text("Doubao Resource ID").font(.caption).foregroundStyle(.secondary)
            TextField("可选", text: $configuration.voiceResourceID)
              .textInputAutocapitalization(.never).autocorrectionDisabled()
              .accessibilityIdentifier("doubaoResourceID")
            Picker("流式接口", selection: $configuration.endpoint) {
              ForEach(VoiceProviderPreset.doubaoStreamEndpoints, id: \.endpoint) { option in
                Text(option.title).tag(option.endpoint)
              }
            }
            .accessibilityIdentifier("doubaoStreamEndpoint")
            Text("整句流式边说边传，说完返回整句，官方称准确率更高、推荐用于输入法；双向流式返回增量结果。")
              .font(.footnote).foregroundStyle(.secondary)
            Toggle("ITN", isOn: $configuration.doubaoEnableITN)
            Toggle("标点", isOn: $configuration.doubaoEnablePunctuation)
            Toggle("DDC", isOn: $configuration.doubaoEnableDDC)
            TextField("Boosting table ID（可选）", text: $configuration.doubaoBoostingTableID)
              .textInputAutocapitalization(.never).autocorrectionDisabled()
              .accessibilityIdentifier("doubaoBoostingTableID")
          }.padding(.vertical, 4)
        }
        Button { fetchModels() } label: {
          HStack {
            Label(fetchingModels ? "正在获取模型…" : "获取模型列表", systemImage: "arrow.clockwise")
            Spacer()
            if fetchingModels { ProgressView() }
          }
        }
        .disabled(kind == .voice && configuration.voiceProvider == .doubao)
        .accessibilityIdentifier("fetchServiceModels")
        Button { testConnection() } label: {
          HStack {
            Label(testingConnection ? "正在测试…" : "测试连接", systemImage: "checkmark.seal")
            Spacer()
            if testingConnection { ProgressView() }
          }
        }
        .disabled(busy)
        .accessibilityIdentifier("testServiceConnection")
        if !modelStatus.isEmpty {
          Text(modelStatus).font(.footnote).foregroundStyle(.secondary)
            .accessibilityIdentifier("serviceModelsStatus")
        }
        modelSelection
        if kind == .ai {
          TextField("润色要求", text: $configuration.prompt)
            .accessibilityIdentifier("servicePrompt")
        }
        Button { save() } label: {
          Label("保存配置", systemImage: "checkmark.circle.fill").frame(maxWidth: .infinity)
        }
          .buttonStyle(.borderedProminent)
          .tint(Color(uiColor: MetasequoiaTheme.forestUIColor))
          .accessibilityIdentifier("saveServiceConfiguration")
        Button("删除此服务的密钥", role: .destructive) {
          do {
            let url = try configuration.validatedURL(
              allowWebSocket: kind == .voice && configuration.voiceProvider == .doubao)
            try ServiceTokenStore.write("", kind: kind, url: url)
            if kind == .ai { try KeyboardAIService.disable(); keyboardAIEnabled = false; publishAICandidates() }
            token = ""
            fetchedModels = nil
            modelStatus = ""
            status = "已删除此服务的密钥"
          } catch { status = error.localizedDescription }
        }
      } header: {
        Text("连接配置")
      } footer: {
        Text("填写完整接口地址。密钥保存在本机钥匙串，按服务地址分别保存。")
      }
      .disabled(busy || recorder.isRecording)

  }

  private func selectProvider(_ provider: AIProviderPreset) {
    guard provider != configuration.provider else { return }
    fetchedModels = nil
    modelStatus = ""
    editsCustomModel = false
    providerDrafts[configuration.provider] = configuration
    configuration = providerDrafts[provider] ?? CustomServiceConfiguration.loadPreset(provider)
    // Unsaved key text must never follow an endpoint change. Saved keys are origin-scoped.
    token = ""
    status = ""
    output = ""
  }

  private func selectVoiceProvider(_ provider: VoiceProviderPreset) {
    guard provider != configuration.voiceProvider else { return }
    fetchedModels = nil
    modelStatus = ""
    editsCustomModel = false
    voiceProviderDrafts[configuration.voiceProvider] = configuration
    configuration = voiceProviderDrafts[provider] ?? CustomServiceConfiguration.loadVoicePreset(provider)
    token = ""
    status = ""
    output = ""
  }

  private func fetchModels() {
    if kind == .voice && configuration.voiceProvider == .doubao {
      modelStatus = "豆包语音模型由当前接口固定提供。"
      return
    }
    let config = configuration
    do {
      let url = try config.validatedURL(requiresModel: false)
      let enteredKey = token.trimmingCharacters(in: .whitespacesAndNewlines)
      let key = try enteredKey.isEmpty ? ServiceTokenStore.read(kind, url: url) : enteredKey
      guard !key.isEmpty else { modelStatus = "请先填写 API Key，或使用已保存的密钥。"; return }
      UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
      busy = true
      fetchingModels = true
      modelStatus = ""
      requestID = UUID()
      let id = requestID
      operation = Task {
        do {
          let models = try await ModelCatalogClient.fetch(configuration: config, kind: kind, token: key)
          try Task.checkCancellation()
          guard requestID == id else { return }
          fetchedModels = models
          if !models.contains(configuration.model) { configuration.model = models[0] }
          editsCustomModel = false
          modelStatus = "已获取 \(models.count) 个模型。请选择支持当前功能的模型。"
        } catch {
          if requestID == id && !Task.isCancelled { modelStatus = error.localizedDescription }
        }
        if requestID == id { busy = false; fetchingModels = false }
      }
    } catch { modelStatus = error.localizedDescription }
  }

  /// Check the endpoint, model and key as typed, before saving, the way the desktop settings page does. A key left blank uses the saved one.
  private func testConnection() {
    let config = configuration
    let doubao = kind == .voice && config.voiceProvider == .doubao
    do {
      let url = try config.validatedURL(requiresModel: !doubao, allowWebSocket: doubao)
      let enteredKey = token.trimmingCharacters(in: .whitespacesAndNewlines)
      let key = try enteredKey.isEmpty ? ServiceTokenStore.read(kind, url: url) : enteredKey
      guard !key.isEmpty else { modelStatus = "请先填写 API Key，或使用已保存的密钥。"; return }
      UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
      let client = doubao ? DoubaoVoiceClient(
        transport: DoubaoWebSocketTransport(),
        codec: DoubaoHostFrameCodec.make(enableITN: config.doubaoEnableITN, punctuation: config.doubaoEnablePunctuation,
                                         DDC: config.doubaoEnableDDC, boostingTable: config.doubaoBoostingTableID)) : nil
      busy = true
      testingConnection = true
      modelStatus = ""
      requestID = UUID()
      let id = requestID
      operation = Task {
        do {
          try await CustomServiceClient.test(kind: kind, configuration: config, token: key, doubaoClient: client)
          try Task.checkCancellation()
          if requestID == id { modelStatus = "连接成功，API Key 和模型配置有效。" }
        } catch is CancellationError {
        } catch {
          if requestID == id && !Task.isCancelled { modelStatus = "测试失败：\(error.localizedDescription)" }
        }
        if requestID == id { busy = false; testingConnection = false }
      }
    } catch { modelStatus = error.localizedDescription }
  }

  /// Point the shared `ai_assistant` at the configuration the keyboard can actually sign for (the one published to the Keychain), or switch it off when there is none. The key itself stays in the Keychain.
  private func publishAICandidates() {
    let published = KeyboardAIService.configuration()
    let enabled = aiCandidatesEnabled && published != nil
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      preferences["ai_assistant"] = AICandidatePreference.assistant(
        preferences["ai_assistant"] as? [String: Any], enabled: enabled, limit: aiCandidateLimit,
        provider: published?.provider.rawValue ?? "", endpoint: published?.endpoint ?? "",
        model: published?.model ?? "")
    }
    if !written { status = "候选栏 AI 设置未能保存，请稍后重试。" }
    else if aiCandidatesEnabled && published == nil { status = "请先保存配置，候选栏 AI 候选会在保存后生效。" }
  }

  @discardableResult private func save() -> Bool {
    do {
      try configuration.save(kind, token: token)
      token = ""
      if kind == .ai && keyboardAIEnabled {
        try KeyboardAIService.publish(configuration, token: ServiceTokenStore.read(.ai, url: configuration.validatedURL()))
      }
      status = "配置已保存"
      if kind == .ai && keyboardAIEnabled { publishAICandidates() }
      return true
    } catch { status = error.localizedDescription; return false }
  }
  private func send() {
    if recognizesOnDevice {
      // A recording whose on-device pass failed is recognized again from the saved audio.
      guard let pcm = recorder.pcmAudio, save() else { return }
      recognizeOnDevice(AsyncStream { continuation in
        stride(from: 0, to: pcm.count, by: 3_200).forEach { continuation.yield(pcm.subdata(in: $0..<min($0 + 3_200, pcm.count))) }
        continuation.finish()
      })
      return
    }
    guard save() else { return }
    busy = true
    status = ""
    output = ""
    transcript = ""
    let config = configuration
    let polish = kind == .voice && voiceSettings.polishEnabled ? voiceSettings : nil
    let language = kind == .voice ? voiceSettings.transcriptionLanguage(for: config.voiceProvider) : nil
    let text = input
    let audio = recorder.audio
    let pcm = recorder.pcmAudio
    let generation: UInt64
    let doubaoClient: DoubaoVoiceClient?
    if config.voiceProvider == .doubao {
      voiceGeneration &+= 1
      generation = voiceGeneration
      let codec = DoubaoHostFrameCodec.make(
        enableITN: config.doubaoEnableITN,
        punctuation: config.doubaoEnablePunctuation,
        DDC: config.doubaoEnableDDC,
        boostingTable: config.doubaoBoostingTableID)
      doubaoClient = DoubaoVoiceClient(transport: DoubaoWebSocketTransport(), codec: codec)
    } else {
      generation = 0
      doubaoClient = nil
    }
    requestID = UUID()
    let id = requestID
    operation = Task {
      do {
        let tokenURL = try config.validatedURL(
          allowWebSocket: kind == .voice && config.voiceProvider == .doubao)
        let savedToken = try ServiceTokenStore.read(kind, url: tokenURL)
        let result = try await CustomServiceClient.request(kind: kind, configuration: config,
          text: text, wav: audio, pcm: pcm, token: savedToken, language: language,
          generation: generation, doubaoClient: doubaoClient)
        try Task.checkCancellation()
        guard requestID == id else { return }
        try await deliver(result, polish: polish, id: id)
      } catch is CancellationError {
        if requestID == id { status = "已取消" }
      } catch {
        if requestID == id && !Task.isCancelled { status = error.localizedDescription }
      }
      if requestID == id { busy = false }
    }
  }
  /// Shows a recognized result, polished first when the voice page asks for it.
  private func deliver(_ result: String, polish: VoicePolishSettings?, id: UUID) async throws {
    if let polish, !result.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
      transcript = result
      output = result
      status = "识别完成，正在润色…"
      do {
        output = try await Self.polish(result, settings: polish)
        status = "已完成"
      } catch is CancellationError {
        throw CancellationError()
      } catch {
        guard requestID == id else { return }
        status = "润色失败，已保留识别原文：\(error.localizedDescription)"
      }
    } else {
      output = result
      status = "已完成"
    }
  }

  /// Doubao with 边说边识别 on, the only provider that can take audio before the recording ends.
  private var streamsLive: Bool {
    kind == .voice && configuration.voiceProvider == .doubao && voiceSettings.streamLive
  }

  /// Recognizes while the user is still speaking. A failed request leaves the recording in place once it stops, so it can still be sent the ordinary way.
  private func recognizeLive(_ pcm: AsyncStream<Data>) {
    let config = configuration
    let polish = voiceSettings.polishEnabled ? voiceSettings : nil
    voiceGeneration &+= 1
    let generation = voiceGeneration
    let client = DoubaoVoiceClient(transport: DoubaoWebSocketTransport(), codec: DoubaoHostFrameCodec.make(
      enableITN: config.doubaoEnableITN, punctuation: config.doubaoEnablePunctuation,
      DDC: config.doubaoEnableDDC, boostingTable: config.doubaoBoostingTableID))
    requestID = UUID()
    let id = requestID
    output = ""
    transcript = ""
    status = ""
    liveText = ""
    recognizesLive = true
    operation = Task {
      do {
        let url = try config.validatedURL(requiresModel: false, allowWebSocket: true)
        let token = try ServiceTokenStore.read(.voice, url: url)
        let result = try await CustomServiceClient.streamDoubao(
          configuration: config, token: token, generation: generation, client: client, pcm: pcm
        ) { text in
          Task { @MainActor in if requestID == id && recognizesLive { liveText = text } }
        }
        try Task.checkCancellation()
        guard requestID == id else { return }
        recognizesLive = false
        liveText = ""
        // The recording has been recognized; keeping it would offer to send it a second time.
        recorder.discard()
        busy = true
        try await deliver(result, polish: polish, id: id)
      } catch is CancellationError {
        if requestID == id { status = "已取消" }
      } catch {
        if requestID == id && !Task.isCancelled { status = "实时识别失败，可停止录音后改为发送录音：\(error.localizedDescription)" }
      }
      if requestID == id {
        recognizesLive = false
        liveText = ""
        busy = false
      }
    }
  }

  /// 本地模型 or 系统语音识别: recognition on the phone itself, always while the user speaks.
  private var recognizesOnDevice: Bool {
    kind == .voice && configuration.voiceProvider.isOnDevice
  }

  /// Recognizes on the phone while the user is still speaking, showing the running transcript. A failure leaves the recording in place once it stops, so it can be recognized again.
  private func recognizeOnDevice(_ pcm: AsyncStream<Data>) {
    let provider = configuration.voiceProvider
    let language = voiceSettings.language
    let polish = voiceSettings.polishEnabled ? voiceSettings : nil
    let model = localModels.selectedDirectory
    requestID = UUID()
    let id = requestID
    output = ""
    transcript = ""
    status = ""
    liveText = ""
    recognizesLive = true
    operation = Task {
      let partial: (String) -> Void = { text in
        Task { @MainActor in if requestID == id && recognizesLive { liveText = text } }
      }
      do {
        let result: String
        if provider == .local {
          guard let model else { throw ServiceFailure(message: "请先在“本地模型”中下载并选用一个模型。") }
          let stateRoot = URL(fileURLWithPath: MetasequoiaInputSessionBridge.sharedStateDirectory, isDirectory: true)
          let resources = PersonalDictionaryBridge.packagedResources
          // The user's own words, read while the first audio is already buffering.
          let hotwords = await Task.detached(priority: .userInitiated) {
            resources.map { LocalSpeechModelStore.hotwords(resources: $0, stateRoot: stateRoot) } ?? []
          }.value
          result = try await LocalSpeechRecognizer.transcribe(pcm: pcm, modelDirectory: model, language: language,
                                                             hotwords: hotwords, partial: partial)
        } else {
          result = try await SystemSpeechRecognizer.transcribe(pcm: pcm, language: language, partial: partial)
        }
        try Task.checkCancellation()
        guard requestID == id else { return }
        recognizesLive = false
        liveText = ""
        // The recording has been recognized; keeping it would offer to recognize it a second time.
        recorder.discard()
        busy = true
        try await deliver(result, polish: polish, id: id)
      } catch is CancellationError {
        if requestID == id { status = "已取消" }
      } catch {
        if requestID == id && !Task.isCancelled {
          if recorder.isRecording { recorder.stop() }
          status = "识别失败：\(error.localizedDescription)"
        }
      }
      if requestID == id {
        recognizesLive = false
        liveText = ""
        busy = false
      }
    }
  }

  /// The polish pass after recognition, on its own saved service or the one under 「AI 设置」.
  private static func polish(_ transcript: String, settings: VoicePolishSettings) async throws -> String {
    var (configuration, token) = try VoicePolishService.resolved()
    configuration.prompt = settings.systemPrompt
    let polished = try await CustomServiceClient.request(
      kind: .ai, configuration: configuration, text: VoicePolishSettings.userMessage(transcript), token: token)
    let trimmed = polished.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { throw ServiceFailure(message: "服务未返回可用文字。") }
    return trimmed
  }

  private func cancelRequest() {
    requestID = UUID()
    operation?.cancel()
    // Live recognition owns the microphone until the recorder finishes its stream. Cancelling the
    // task alone does not finish an AsyncStream producer, so the send side can remain suspended
    // while the audio engine keeps recording in the background. Stop and discard it here to close
    // the stream and release the engine immediately.
    if recognizesLive { recorder.discard() }
    fetchingModels = false
    testingConnection = false
    busy = false
    recognizesLive = false
    liveText = ""
  }
  private func cancelAndClear() {
    cancelRequest()
    recorder.discard()
    // A resident model holds hundreds of megabytes; nothing on this page needs it once the page is gone.
    if kind == .voice { LocalSpeechEngine.shared.release() }
  }
}

/// 「候选栏 AI 候选 → 提示词」: the desktop's three custom prompt slots for candidate-bar AI, on a page of its own so the editor has the whole screen on a phone.
private struct AICandidatePromptView: View {
  @State private var slot = AICandidatePreference.promptID(MetasequoiaInputSessionBridge.loadSharedPreferences())
  @State private var text = ""
  @State private var savedText = ""
  @StateObject private var autosave = SettingsAutosave()

  var body: some View {
    Form {
      Section {
        Picker("使用的提示词", selection: $slot) {
          ForEach(AICandidatePreference.promptSlots, id: \.id) { Text($0.title).tag($0.id) }
        }
        .pickerStyle(.segmented)
        .accessibilityIdentifier("aiCandidatePromptSlot")
        TextEditor(text: $text)
          .frame(minHeight: 240)
          .font(.body.monospaced())
          .autocorrectionDisabled()
          .textInputAutocapitalization(.never)
          .accessibilityLabel("提示词内容").accessibilityIdentifier("aiCandidatePromptText")
          .onChange(of: text) { _, _ in
            guard text != savedText || autosave.hasPending else { return }
            let slot = slot, content = text
            autosave.schedule { try save(content, slot: slot) }
          }
      } footer: {
        VStack(alignment: .leading, spacing: 4) {
          Text("选中的提示词会随拼音和前文一起发给 AI 服务。留空时使用内置提示词，它要求服务只返回 JSON 形式的候选列表。")
          SettingsAutosaveStatus(autosave: autosave)
        }
      }
    }
    .navigationTitle("提示词")
    .onAppear { load(slot) }
    .onChange(of: slot) { _, next in
      // Keep what was typed in the slot being left (the waiting save names that slot), then put the new slot in use.
      autosave.flush()
      load(next)
      let content = text
      autosave.saveNow { try save(content, slot: next) }
    }
    .flushesAutosave(autosave)
  }

  private func load(_ slot: String) {
    text = AICandidatePreference.prompt(MetasequoiaInputSessionBridge.loadSharedPreferences(), slot: slot)
    savedText = text
  }

  private func save(_ content: String, slot: String) throws {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      preferences["ai_assistant"] = AICandidatePreference.assistant(
        preferences["ai_assistant"] as? [String: Any], promptSlot: slot, text: content)
    }
    guard written else { throw ServiceFailure(message: "提示词未能保存，请稍后重试。") }
    savedText = content
  }
}

/// The system recording cues. Haptics accompany them so the cue still registers with the ringer off.
private enum VoiceCue {
  static func playStart() async {
    UIImpactFeedbackGenerator(style: .light).impactOccurred()
    await withCheckedContinuation { continuation in
      AudioServicesPlaySystemSoundWithCompletion(1113) { continuation.resume() }
    }
  }

  static func playEnd() {
    UIImpactFeedbackGenerator(style: .light).impactOccurred()
    AudioServicesPlaySystemSound(1114)
  }
}

/// The live microphone level under the record button, as the Windows voice overlay draws it: one bar per sample, newest on the right, so the user can see the microphone is hearing them.
private struct VoiceLevelWaveform: View {
  let levels: [Float]

  var body: some View {
    HStack(alignment: .center, spacing: 3) {
      ForEach(0..<VoiceLevel.history, id: \.self) { index in
        let offset = index - (VoiceLevel.history - levels.count)
        let level = offset >= 0 ? CGFloat(levels[offset]) : 0
        Capsule().fill(Color.red.opacity(0.75))
          .frame(width: 4, height: 4 + 28 * level)
      }
    }
    .frame(maxWidth: .infinity, minHeight: 32)
    .animation(.linear(duration: 0.05), value: levels)
    .accessibilityHidden(true)
    .accessibilityIdentifier("voiceLevelWaveform")
  }
}

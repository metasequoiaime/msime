import SwiftUI
import UIKit

struct KeyboardSettingsView: View {
  var body: some View {
    Form {
        Section("键盘外观与输入") {
          NavigationLink(destination: InputSettingsView()) {
            Label("输入设置", systemImage: "slider.horizontal.3")
          }.accessibilityIdentifier("inputSettingsLink")
          NavigationLink(destination: KeyboardLayoutSettingsView()) {
            Label("键盘布局", systemImage: "rectangle.3.group")
          }.accessibilityIdentifier("keyboardLayoutLink")
          NavigationLink(destination: SkinSettingsView()) {
            Label("皮肤", systemImage: "paintpalette")
          }.accessibilityIdentifier("skinSettingsLink")
        }
        Section("词库与智能服务") {
          NavigationLink(destination: DictionarySettingsView()) {
            Label("词库", systemImage: "books.vertical")
          }.accessibilityIdentifier("dictionarySettingsLink")
          NavigationLink(destination: ServiceSettingsView(kind: .ai)) {
            Label("AI 设置", systemImage: "sparkles")
          }.accessibilityIdentifier("aiSettingsLink")
          NavigationLink(destination: ServiceSettingsView(kind: .voice)) {
            Label("语音设置", systemImage: "waveform")
          }.accessibilityIdentifier("voiceSettingsLink")
        }

        Section("系统") {
          Button {
            guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
            UIApplication.shared.open(url)
          } label: {
            Label("系统键盘设置", systemImage: "gearshape")
          }
          .accessibilityIdentifier("openKeyboardSettingsButton")
        }
    }.navigationTitle("键盘设置").navigationBarTitleDisplayMode(.inline)
  }
}

/// 输入页：语言与方案、中文选项、辅助码、候选翻译和其余输入开关，对应 Android 的 `TypingPage`。
///
/// 各项都以共享偏好文档为准，页面每次写完都重新读回，免得显示一个键盘其实没有的设置。只有「沿用上次的中英文」是例外，它由键盘从 App Group 的 `ImeModeMemoryPreference` 读取。
struct InputSettingsView: View {
  @ObservedObject private var resourcePacks = IOSResourcePackStore.shared
  /// 最近一次读到的共享文档；拼音纠错和辅助码两行直接读它。
  @State private var document: [String: Any]?
  @State private var inputScheme = InputSchemePreference.scheme
  @State private var enabledSchemes = InputSchemePreference.enabledSchemes
  @State private var wubiProfile = WubiProfilePreference.profile
  @State private var usesTraditionalOutput = ChineseOutputPreference.usesTraditional
  @State private var fuzzy = FuzzyPinyinPreference.Settings.pristine
  @State private var habits = InputHabitPreference.mirrored
  @State private var startsInEnglish = false
  @State private var remembersImeMode = false
  @State private var wubiAutoCommitUnique = true
  @State private var backspaceRevertsSelection = true
  @State private var wubiMixedPinyin = MSIMEAppEdition.wubiMixedPinyinDefault
  @State private var wubiCodeHint = true
  /// 用户导入的辅助码表（#7176），列在「辅助码方案」的内置方案之后。
  @State private var helpcodePacks: [HelpcodeChoicePolicy.Pack] = []
  @State private var addingLanguage = false
  @State private var failedGroup: PageGroup?

  private enum PageGroup { case languages, chinese, helpcode, translation }

  /// 语言选项面板里的一个选项。
  private enum LanguageChoice: Hashable {
    case scheme(ChineseInputScheme)
    case wubi(String)
    case remove
  }


  /// 设置始终提供本版本的方案；缺失词库在用户添加时下载，键盘只显示已可用的方案。
  private static let availableSchemes = ChineseInputScheme.allCases.filter(\.isOfferedByEdition)


  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "语言与方案", footer: footer(.languages)) {
          languageCard
          if ChineseInputScheme.nineKeyShuangpin.isOfferedByEdition {
            DesignDivider(leading: 16)
            NavigationLink(destination: NineKeyShuangpinSettingsView()) {
              HStack {
                Text("九键双拼方案与自定义")
                Spacer()
                Image(systemName: "chevron.right").font(.caption).foregroundStyle(.secondary)
              }.padding(16)
            }.accessibilityIdentifier("nineKeyShuangpinSettingsLink")
          }
        }
        chineseGroup
        helpcodeGroup
        translationGroup
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .designCanvasBackground()
    .navigationTitle("输入")
    .navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive({
      resourcePacks.reload()
      reloadPreferences()
      reloadHelpcodePacks()
    }, onAppear: {
      // 真机下载验收期间仅保持当前设置页面亮屏。
      if ProcessInfo.processInfo.arguments.contains("--resource-pack-ui-test") { UIApplication.shared.isIdleTimerDisabled = true }
    })
    .onDisappear {
      if ProcessInfo.processInfo.arguments.contains("--resource-pack-ui-test") { UIApplication.shared.isIdleTimerDisabled = false }
    }
    .alert("下载词库", isPresented: Binding(
      get: { resourcePacks.meteredRequest != nil },
      set: { if !$0 { resourcePacks.meteredRequest = nil } }), presenting: resourcePacks.meteredRequest) { id in
        Button("下载") { resourcePacks.start(id) }
        Button("取消", role: .cancel) { resourcePacks.meteredRequest = nil }
      } message: { id in
        Text("当前网络按流量计费或开启了低数据模式，词库大小约 \(resourcePacks.size(id))。是否继续？")
      }
  }

  /// 已导入的辅助码表只在页面出现时重读：目录会校验每个包的码表，不跟着每次写设置重读。从「更多辅助码设置」导入或删除后回到本页时会再出现一次。
  private func reloadHelpcodePacks() {
    Task { @MainActor in
      helpcodePacks = await Task.detached(priority: .userInitiated) { HelpcodeSettingsView.installedPacks() }.value
    }
  }

  private func footer(_ group: PageGroup) -> String? {
    failedGroup == group ? SettingsSaveFailure.message : nil
  }

  // MARK: - 语言与方案

  private func available(_ language: InputLanguage) -> [ChineseInputScheme] {
    language.schemes.filter(Self.availableSchemes.contains)
  }

  /// 有一行的语言：本版本有普通话就一定列出，其他语言要启用了其中某个方案才列出。
  private var shownLanguages: [InputLanguage] {
    InputLanguage.allCases.filter { language in
      let schemes = available(language)
      return !schemes.isEmpty && (!language.isRemovable || schemes.contains(where: enabledSchemes.contains))
    }
  }

  /// 添加语言列出的语言：本页提供、但一个方案都没启用的那些。
  private var addableLanguages: [InputLanguage] {
    InputLanguage.allCases.filter { language in
      let schemes = available(language)
      return language.isRemovable && !schemes.isEmpty && !schemes.contains(where: enabledSchemes.contains)
    }
  }

  /// 语言行上显示的方案：键盘当前方案属于该语言时就是它，否则是该语言第一个已启用的方案。
  private func shownScheme(_ language: InputLanguage) -> ChineseInputScheme? {
    ChineseInputScheme.represented(among: available(language), enabled: enabledSchemes, current: inputScheme)
  }

  @ViewBuilder private var languageCard: some View {
    let languages = shownLanguages
    let addable = addableLanguages
    ForEach(Array(languages.enumerated()), id: \.element) { index, language in
      if index > 0 { DesignDivider(leading: 58) }
      let shown = shownScheme(language)
      let pack = shown.flatMap(IOSResourcePacks.requiredPack)
      InputLanguageRow(language: language, value: pack.map { resourcePacks.available($0) ? (shown?.shortLabel ?? "") : resourcePacks.describe($0) } ?? shown?.shortLabel, options: options(for: language),
                       selected: selectedChoice(in: language)) { choose($0, in: language) }
      if let pack, !resourcePacks.available(pack) {
        packDownloadRow(pack, title: "\(language.name)词库")
      }
    }
    if addingLanguage {
      ForEach(addable) { language in
        DesignDivider(leading: 0)
        AddableLanguageRow(language: language, detail: available(language).first.flatMap(IOSResourcePacks.requiredPack).map {
          resourcePacks.available($0) ? "添加" : "添加并下载 \(resourcePacks.size($0))"
        } ?? "添加") { add(language) }
      }
    }
    if !addable.isEmpty {
      DesignDivider(leading: 0)
      Button {
        withAnimation(.easeInOut(duration: 0.2)) { addingLanguage.toggle() }
      } label: {
        HStack(spacing: 12) {
          Image(systemName: "plus").font(.system(size: 20, weight: .regular))
            .frame(width: 30, height: 30).accessibilityHidden(true)
          Text(addingLanguage ? "完成" : "添加方案").font(.system(size: 17))
          Spacer(minLength: 0)
        }
        .foregroundStyle(MetasequoiaTheme.accent)
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
        .contentShape(Rectangle())
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("addInputLanguageButton")
    }
  }

  /// 面板里打勾的选项就是行上显示的方案，键盘正用着别的语言时也一样。
  private func selectedChoice(in language: InputLanguage) -> LanguageChoice? {
    guard let scheme = shownScheme(language) else { return nil }
    return scheme == .wubi ? .wubi(wubiProfile) : .scheme(scheme)
  }

  /// 语言的选项面板：按顺序直接列出它的方案，双拼列小鹤、自然码等，五笔列 86 和 98，不再各开一层二级面板（那样「双拼」行、方案面板、双拼面板就成了三层）；不是仅剩的最后一个语言时再加一项移除。每个方案选项保留 `inputScheme_<rawValue>` 这个 id，五笔版本是 `inputScheme_wubi_<profile>`。
  private func options(for language: InputLanguage) -> [DesignOption<LanguageChoice>] {
    let schemes = available(language)
    var options: [DesignOption<LanguageChoice>] = []
    for scheme in schemes {
      if scheme == .wubi {
        options += WubiProfilePreference.profiles.map { profile in
          DesignOption(title: profile == "wubi98" ? "五笔 98" : "五笔 86", value: .wubi(profile),
                       identifier: "inputScheme_wubi_\(profile)")
        }
      } else {
        options.append(schemeOption(scheme, title: optionTitle(scheme)))
      }
    }
    if language.isRemovable && canRemove(language) {
      options.append(DesignOption(title: "移除\(language.name)", value: .remove, isDestructive: true,
                                  identifier: "removeInputLanguage_\(language.rawValue)"))
    }
    return options
  }

  private func schemeOption(_ scheme: ChineseInputScheme, title: String) -> DesignOption<LanguageChoice> {
    DesignOption(title: title, value: .scheme(scheme), identifier: "inputScheme_\(scheme.rawValue)")
  }

  /// 方案在所属语言面板里的名字，面板标题已经是语言名。
  private func optionTitle(_ scheme: ChineseInputScheme) -> String {
    switch scheme {
    case .shuangpin: "小鹤"
    case .ziranma: "自然码"
    case .microsoft: "微软"
    case .shoudao: "首道"
    case .zhuyin: "注音"
    case .cantonese: "粤拼"
    case .japanese: "26 键"
    case .japaneseNineKey: "9 键"
    default: scheme.title
    }
  }

  /// 移除该语言后，是否还剩下键盘能运行的已启用方案。
  private func canRemove(_ language: InputLanguage) -> Bool {
    enabledSchemes.contains { !language.schemes.contains($0) && Self.availableSchemes.contains($0) }
  }

  private func choose(_ choice: LanguageChoice, in language: InputLanguage) {
    switch choice {
    case .scheme(let scheme):
      // 方案尚未启用时，`select` 在同一次写入里顺带启用它。
      if let pack = IOSResourcePacks.requiredPack(for: scheme), !resourcePacks.available(pack) {
        let saved = InputSchemePreference.setEnabled(scheme, true)
        record(saved, .languages)
        if saved { resourcePacks.request(pack) }
      } else {
        record(InputSchemePreference.select(scheme), .languages)
      }
    case .wubi(let profile):
      record(WubiProfilePreference.save(profile) && InputSchemePreference.select(.wubi), .languages)
    case .remove:
      remove(language)
    }
    reloadPreferences()
  }

  /// 停用该语言的全部方案。键盘正用着其中之一时切到全拼 26 键，全拼没启用就切到剩下的第一个方案。如果停用后一个能运行的方案都不剩，就什么都不写。
  private func remove(_ language: InputLanguage) {
    var refused = false
    let written = InputSchemePreference.update { selection in
      let remaining = selection.enabled.filter { !language.schemes.contains($0) }
      let runnable = remaining.filter { scheme in
        Self.availableSchemes.contains(scheme) && (IOSResourcePacks.requiredPack(for: scheme).map(resourcePacks.available) ?? true)
      }
      guard let fallback = runnable.contains(.quanpin) ? ChineseInputScheme.quanpin : runnable.first else {
        refused = true
        return
      }
      selection.enabled = remaining
      if language.schemes.contains(selection.scheme) { selection.scheme = fallback }
    }
    if refused {
      ToastCenter.shared.show("至少要保留一种输入语言")
    } else {
      record(written != nil, .languages)
    }
  }

  /// 启用该语言的第一个方案，但不把键盘切过去。
  private func add(_ language: InputLanguage) {
    guard let first = available(language).first else { return }
    let saved = InputSchemePreference.setEnabled(first, true)
    record(saved, .languages)
    if saved {
      if let pack = IOSResourcePacks.requiredPack(for: first), !resourcePacks.available(pack) { resourcePacks.request(pack) }
      else { ToastCenter.shared.show("已添加\(language.name)") }
    }
    reloadPreferences()
  }

  private func packDownloadRow(_ id: String, title: String) -> some View {
    Button {
      if resourcePacks.statuses[id]?.running == true { resourcePacks.cancel(id) }
      else { resourcePacks.request(id) }
    } label: {
      HStack {
        VStack(alignment: .leading, spacing: 4) {
          Text(title).foregroundStyle(.primary)
          Text(resourcePacks.describe(id)).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
        }
        Spacer()
        Text(resourcePacks.statuses[id]?.running == true ? "取消" : "下载").foregroundStyle(MetasequoiaTheme.accent)
      }
      .padding(.horizontal, 16).frame(maxWidth: .infinity, minHeight: 52)
      .contentShape(Rectangle())
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityIdentifier("downloadResourcePack_\(id)")
  }

  // MARK: - 中文

  /// `quanpin` 的两项自动纠错都开着时拼音纠错才算开；两项默认都开。
  private var autocorrect: Bool {
    let quanpin = document?["quanpin"] as? [String: Any]
    return (quanpin?["autocorrect_transposition"] as? Bool ?? true) && (quanpin?["autocorrect_neighbor"] as? Bool ?? true)
  }

  private var chineseGroup: some View {
    DesignGroup(title: "中文", footer: footer(.chinese)) {
      DesignSelectRow(title: "中文字符集", options: [DesignOption(title: "简体", value: false), DesignOption(title: "繁体", value: true)],
                      selection: Binding(get: { usesTraditionalOutput }, set: { traditional in
                        guard traditional != usesTraditionalOutput else { return }
                        usesTraditionalOutput = traditional
                        record(ChineseOutputPreference.save(traditional), .chinese)
                        reloadPreferences()
                      }),
                      sheetTitle: "中文字符集", identifier: "chineseOutputPicker")
      DesignDivider()
      DesignToggleRow(title: "拼音纠错", info: "纠正相邻键误触和字母顺序颠倒",
                      isOn: Binding(get: { autocorrect }, set: saveAutocorrect))
        .accessibilityIdentifier("pinyinAutocorrectToggle")
      DesignDivider()
      DesignToggleRow(title: "模糊音", info: "如 z/zh、an/ang 不分", isOn: Binding(get: { fuzzy.enabled }, set: { enabled in
        let next = FuzzyPinyinSettingsView.toggled(fuzzy, enabled: enabled)
        let saved = FuzzyPinyinPreference.save(next)
        if saved { fuzzy = next }
        record(saved, .chinese)
        reloadPreferences()
      }))
      .accessibilityIdentifier("fuzzyPinyinToggle")
      if fuzzy.enabled {
        DesignDivider()
        navRow("模糊音规则", identifier: "fuzzyPinyinSettingsLink") { FuzzyPinyinSettingsView() }
      }
      additionalChineseRows
    }
  }

  /// 把两项自动纠错开关写进现有的 `quanpin` 对象，保留其中其他内容，与 Android `TypingPage.saveAutocorrect` 的做法一致。
  private func saveAutocorrect(_ enabled: Bool) {
    record(MetasequoiaInputSessionBridge.updateSharedPreferences {
      var quanpin = $0["quanpin"] as? [String: Any] ?? [:]
      quanpin["autocorrect_transposition"] = enabled
      quanpin["autocorrect_neighbor"] = enabled
      $0["quanpin"] = quanpin
    }, .chinese)
    reloadPreferences()
  }

  // MARK: - 辅助码

  private var helpcodeGroup: some View {
    let family = HelpcodeSettingsView.family(of: inputScheme)
    let schema = HelpcodeSettingsView.schema(of: family, in: document)
    let pack = HelpcodeChoicePolicy.selectedPack(of: family, in: document)
    let selected = HelpcodeChoicePolicy.selectedValue(schema: schema, selectedPack: pack)
    var options = HelpcodeSettingsView.schemas.map { DesignOption(title: $0.1, value: $0.0) }
    // 引擎认识但这份列表里没有的用户码表，仍按它的 id 显示。
    if !options.contains(where: { $0.value == schema }) { options.append(DesignOption(title: schema, value: schema)) }
    // 导入的码表排在内置方案之后，选中的包不在了时显示「未找到」，规则与「更多辅助码设置」相同。
    options += HelpcodeChoicePolicy.packOptions(helpcodePacks, selectedPack: pack)
      .map { DesignOption(title: $0.label, value: $0.value) }
    return DesignGroup(title: "辅助码", footer: footer(.helpcode)) {
      DesignSelectRow(title: "辅助码方案", subtitle: family.label, options: options,
                      selection: Binding(get: { selected }, set: { value in
                        guard value != selected else { return }
                        record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                          HelpcodeChoicePolicy.choose(value, for: family, document: &$0)
                        }, .helpcode)
                        reloadPreferences()
                      }),
                      sheetTitle: "辅助码方案", sheetMessage: family.label, identifier: "helpcodeSchemaPicker")
      DesignDivider()
      navRow("更多辅助码设置", identifier: "helpcodeSettingsLink") { HelpcodeSettingsView() }
    }
  }

  // MARK: - 翻译

  private var translationGroup: some View {
    let languages = CandidateTranslationPreference.languages
    let primary = CandidateTranslationPreference.language(at: habits.primaryLanguage)
    let glossOn = habits.glossEnabled
    // 范围只在候选词翻译开着时有意义；关着时置灰，已存的值不动。
    let scopeOn = glossOn && habits.onlineTranslations
    return DesignGroup(title: "翻译", footer: footer(.translation)) {
      // 键盘只在有释义时才显示翻译，所以离线英文释义打开之前，翻译各行不可用。
      DesignToggleRow(title: "候选词翻译",
                      subtitle: glossOn ? nil : "打开离线英文释义后可用",
                      info: glossOn ? "在候选词下方显示\(primary.title)释义" : nil,
                      isOn: habit(\.onlineTranslations))
        .disabled(!glossOn).opacity(glossOn ? 1 : 0.5)
        .accessibilityIdentifier("candidateTranslationOnline")
      DesignDivider()
      // 文案与共享设置页（`candidate-translation-options-section.tsx`）一致，放在「候选词翻译」正下方。
      DesignSelectRow(title: "翻译范围",
                      info: "仅当前高亮：只翻译高亮的候选，高亮移动时译文跟着走；离线英文释义不受影响",
                      options: [DesignOption(title: "仅当前高亮", value: CandidateTranslationScope.highlighted),
                                DesignOption(title: "全部", value: CandidateTranslationScope.all)],
                      selection: habit(\.translationScope), sheetTitle: "翻译范围",
                      identifier: "candidateTranslationScopePicker")
        .disabled(!scopeOn).opacity(scopeOn ? 1 : 0.5)
      DesignDivider()
      DesignToggleRow(title: "离线英文释义", info: "用随键盘打包的离线词库，不联网", isOn: habit(\.glossEnabled))
        .accessibilityIdentifier("candidateGlossToggle")
      DesignDivider()
      // 与 Android 的「显示读音」相同：默认关闭；英文音标表是约 5.7 MB 的资源包，法、日、西、俄、德、韩释义的读音表另一个（不到 4 MB），只有用户在下面那一行点「下载」才联网取，键盘只读本机文件。外语那一行只在主目标语言不是英语时给出。
      DesignToggleRow(title: "显示读音",
                      subtitle: resourcePacks.available(CandidatePronunciationPreference.packID)
                        && (!targetReadingsNeeded || resourcePacks.available(CandidatePronunciationPreference.targetPackID))
                        ? nil : "需先下载下面的离线读音词典",
                      info: "在英文候选和释义下面标出读音：英文给音标，日文和韩文给罗马音，其他语言给国际音标，不联网",
                      isOn: habit(\.pronunciationEnabled))
        .accessibilityIdentifier("candidatePronunciationToggle")
      if habits.pronunciationEnabled && !resourcePacks.available(CandidatePronunciationPreference.packID) {
        DesignDivider()
        packDownloadRow(CandidatePronunciationPreference.packID, title: "离线英文音标词典")
      }
      if habits.pronunciationEnabled && targetReadingsNeeded
        && !resourcePacks.available(CandidatePronunciationPreference.targetPackID) {
        DesignDivider()
        packDownloadRow(CandidatePronunciationPreference.targetPackID, title: "离线外语读音词典")
      }
      DesignDivider()
      DesignSelectRow(title: "翻译目标语言",
                      options: languages.indices.map { DesignOption(title: languages[$0].title, value: $0) },
                      selection: habit(\.primaryLanguage), sheetTitle: "翻译目标语言",
                      identifier: "candidateTranslationPrimaryPicker")
        .disabled(!glossOn).opacity(glossOn ? 1 : 0.5)
      DesignDivider()
      navRow("翻译服务", info: "联网翻译需要在这里选择服务", identifier: "translationProviderLink") {
        TranslationProviderSettingsView()
      }
    }
  }

  /// 主目标语言不是英语时，读音跟着的是那种语言的释义，要用外语读音表。
  private var targetReadingsNeeded: Bool {
    let code = CandidateTranslationPreference.language(at: habits.primaryLanguage).code.lowercased()
    return code != "en" && CandidatePronunciationModel.languages.contains(code)
  }

  /// 保存一个输入习惯字段的绑定；保存失败就重新读取，让控件显示实际存下的值。
  private func habit<Value>(_ field: WritableKeyPath<InputHabitSettings, Value>) -> Binding<Value> {
    Binding(get: { habits[keyPath: field] }, set: { value in
      let saved = InputHabitPreference.update { $0[keyPath: field] = value }
      if let saved { habits = saved }
      record(saved != nil, .translation)
    })
  }

  // MARK: - 其他输入选项

  private var additionalChineseRows: some View {
    Group {
      DesignToggleRow(title: "退格键回退已选候选词",
                      info: "关闭后，退格只删除剩余拼写，保留已选文字；拼写删空时上屏已选文字",
                      isOn: Binding(get: { backspaceRevertsSelection }, set: { enabled in
                        backspaceRevertsSelection = enabled
                        record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                          $0["backspace_reverts_selection"] = enabled
                        }, .chinese)
                        reloadPreferences()
                      }))
        .accessibilityIdentifier("backspaceRevertsSelection")
      DesignDivider()
      if enabledSchemes.contains(.wubi) {
        DesignToggleRow(title: "五笔拼音混输", info: "五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选",
                        isOn: Binding(get: { wubiMixedPinyin }, set: { enabled in
                          wubiMixedPinyin = enabled
                          record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                            $0["wubi_mixed_pinyin"] = enabled
                          }, .chinese)
                          reloadPreferences()
                        }))
          .accessibilityIdentifier("wubiMixedPinyin")
        DesignDivider()
        DesignToggleRow(title: "候选显示剩余编码", info: "在候选后标出还要输入的字母",
                        isOn: Binding(get: { wubiCodeHint }, set: { enabled in
                          wubiCodeHint = enabled
                          record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                            $0["wubi_code_hint"] = enabled
                          }, .chinese)
                          reloadPreferences()
                        }))
          .accessibilityIdentifier("wubiCodeHint")
        DesignDivider()
        DesignToggleRow(title: "四码唯一候选自动上屏",
                        info: "五笔四码且只有一个候选时自动上屏，关闭后用空格或数字键选它",
                        isOn: Binding(get: { wubiAutoCommitUnique }, set: { enabled in
                          wubiAutoCommitUnique = enabled
                          record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                            $0["wubi_auto_commit_unique"] = enabled
                          }, .chinese)
                          reloadPreferences()
                        }))
          .accessibilityIdentifier("wubiAutoCommitUnique")
        DesignDivider()
        // 五笔查码（#6789）：与三个五笔开关一样，只在启用了五笔时列出。
        navRow("五笔查码", info: "查汉字的 86、98 五笔编码", identifier: "wubiCodeLookupLink") {
          WubiCodeLookupView(profile: wubiProfile)
        }
        DesignDivider()
      }
      DesignSelectRow(title: "默认中英文", info: "新打开的键盘从这里开始",
                      options: [DesignOption(title: "中文", value: false), DesignOption(title: "英文", value: true)],
                      selection: Binding(get: { startsInEnglish }, set: { english in
                        guard english != startsInEnglish else { return }
                        startsInEnglish = english
                        record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                          $0["default_ime_mode"] = english ? "english" : "chinese"
                        }, .chinese)
                        reloadPreferences()
                      }),
                      sheetTitle: "默认中英文", identifier: "defaultImeModePicker")
      DesignDivider()
      DesignToggleRow(title: "沿用上次的中英文", info: "新打开的键盘沿用上次按中/英键选的模式",
                      isOn: Binding(get: { remembersImeMode }, set: { enabled in
                        remembersImeMode = enabled
                        ImeModeMemoryPreference.setEnabled(enabled)
                      }))
        .accessibilityIdentifier("remembersImeModeToggle")
      DesignDivider()
      navRow("快捷模式", identifier: "localModeSettingsLink") { LocalModeSettingsView() }
      DesignDivider()
      navRow("剪贴板历史", identifier: "clipboardHistorySettingsLink") { ClipboardHistorySettingsView() }
    }
  }

  private func navRow<Destination: View>(_ title: String, info: String? = nil, identifier: String,
                                         @ViewBuilder destination: () -> Destination) -> some View {
    NavigationLink(destination: destination()) {
      DesignNavRowLabel(title: title, info: info)
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityIdentifier(identifier)
  }

  // MARK: - 状态

  /// 写入被拒后在 `group` 下面显示保存失败的提示，写入成功后清掉。
  private func record(_ saved: Bool, _ group: PageGroup) {
    failedGroup = saved ? nil : group
  }

  private func reloadPreferences() {
    document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    InputSchemePreference.mirror(document)
    ChineseOutputPreference.mirror(document)
    inputScheme = InputSchemePreference.scheme
    enabledSchemes = InputSchemePreference.enabledSchemes
    usesTraditionalOutput = ChineseOutputPreference.usesTraditional
    if let document { WubiProfilePreference.mirror(document) }
    wubiProfile = WubiProfilePreference.profile
    wubiAutoCommitUnique = document?["wubi_auto_commit_unique"] as? Bool ?? true
    backspaceRevertsSelection = document?["backspace_reverts_selection"] as? Bool ?? true
    wubiMixedPinyin = WubiMixedPinyinPreference.isEnabled(in: document)
    wubiCodeHint = WubiCodeHintPreference.isEnabled(in: document)
    fuzzy = FuzzyPinyinPreference.settings(in: document) ?? .pristine
    habits = InputHabitPreference.settings(in: document)
    startsInEnglish = document?["default_ime_mode"] as? String == "english"
    remembersImeMode = ImeModeMemoryPreference.isEnabled()
    if addableLanguages.isEmpty { addingLanguage = false }
  }
}

/// 语言与方案卡片里的一行：语言图块、语言名、它所用的输入方案和一个箭头。点一下打开该语言的选项面板。
private struct InputLanguageRow<Choice: Hashable>: View {
  let language: InputLanguage
  let value: String?
  let options: [DesignOption<Choice>]
  let selected: Choice?
  let onSelect: (Choice) -> Void
  @State private var presented = false

  var body: some View {
    Button { presented = true } label: {
      HStack(spacing: 12) {
        Text(language.tile)
          .font(.system(size: 15, weight: .semibold))
          .lineLimit(1).minimumScaleFactor(0.5)
          .foregroundStyle(MetasequoiaTheme.accent)
          .frame(width: 30, height: 30)
          .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
          .accessibilityHidden(true)
        Text(language.name).font(.system(size: 17)).foregroundStyle(.primary)
        Spacer(minLength: 8)
        if let value {
          Text(value).font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
        }
        DesignChevron(fontSize: 14)
      }
      .padding(.horizontal, 16)
      .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
      .contentShape(Rectangle())
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityLabel(language.name)
    .accessibilityValue(value ?? "")
    .accessibilityIdentifier("inputLanguage_\(language.accessibilityKey)")
    .designOptionSheet(isPresented: $presented, title: language.name, message: "选择输入方案", options: options,
                       selected: selected, onSelect: onSelect)
  }
}

/// 添加语言里可添加的一个语言：描边图块、次要色的语言名和强调色的「添加」。点整行即可添加。
private struct AddableLanguageRow: View {
  let language: InputLanguage
  var detail: String = "添加"
  let add: () -> Void

  var body: some View {
    Button(action: add) {
      HStack(spacing: 12) {
        Text(language.tile)
          .font(.system(size: 15, weight: .semibold))
          .lineLimit(1).minimumScaleFactor(0.5)
          .foregroundStyle(MetasequoiaTheme.sub)
          .frame(width: 30, height: 30)
          .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(MetasequoiaTheme.hair, lineWidth: 1.5))
          .accessibilityHidden(true)
        Text(language.name).font(.system(size: 17)).foregroundStyle(MetasequoiaTheme.sub)
        Spacer(minLength: 8)
        Text(detail).font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.accent)
      }
      .padding(.horizontal, 16)
      .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
      .contentShape(Rectangle())
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityLabel("添加\(language.name)")
    .accessibilityIdentifier("addInputLanguage_\(language.rawValue)")
  }
}

struct OnboardingView: View {
  var onFinish: (() -> Void)? = nil

  private let steps = [
    ("1", "打开键盘设置", "前往“设置 → 通用 → 键盘 → 键盘”。"),
    ("2", "添加水杉输入法", "选择“添加新键盘”，再选择水杉输入法。"),
    ("3", "切换并开始输入", "在输入框长按地球键，选择水杉输入法。"),
  ]

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 28) {
        header

        VStack(spacing: 0) {
          ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
            stepRow(
              number: step.0, title: step.1, detail: step.2, drawsLine: index < steps.count - 1)
          }
        }
        .padding(.horizontal, 20)
        .background(.background, in: RoundedRectangle(cornerRadius: 24, style: .continuous))

        Button(action: openSettings) {
          Label("打开系统设置", systemImage: "gearshape.fill")
            .font(.headline)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 15)
            .foregroundStyle(MetasequoiaTheme.onAccent)
            .background(
              MetasequoiaTheme.accent, in: RoundedRectangle(cornerRadius: 16, style: .continuous)
            )
            .contentShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("openKeyboardSettingsButton")
        .accessibilityHint("打开水杉输入法的系统设置页面")

        if let onFinish {
          Button("已完成，进入设置", action: onFinish)
            .font(.headline)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
            .accessibilityIdentifier("finishOnboardingButton")
        }

        Text("键盘默认离线。打字统计需开启“允许完全访问”以保存本机字数；AI 和语音服务可在设置中单独配置。")
          .font(.footnote)
          .foregroundStyle(.secondary)
          .frame(maxWidth: .infinity, alignment: .center)
      }
      .padding(.horizontal, 22)
      .padding(.vertical, 30)
    }
    .designCanvasBackground()
    .tint(MetasequoiaTheme.accent)
    .navigationTitle("启用指南")
    .navigationBarTitleDisplayMode(.inline)
  }

  private var header: some View {
    HStack(alignment: .center, spacing: 18) {
      MetasequoiaMark()
        .stroke(
          MetasequoiaTheme.accent,
          style: StrokeStyle(lineWidth: 4, lineCap: .round, lineJoin: .round)
        )
        .frame(width: 58, height: 76)
        .padding(12)
        .background(.background, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .accessibilityHidden(true)

      VStack(alignment: .leading, spacing: 5) {
        Text("水杉输入法")
          .font(.system(.largeTitle, design: .rounded).weight(.bold))
          .foregroundStyle(.primary)
        Text("添加键盘，开始使用水杉输入法")
          .font(.subheadline.weight(.medium))
          .foregroundStyle(MetasequoiaTheme.needle)
      }
    }
  }

  private func stepRow(number: String, title: String, detail: String, drawsLine: Bool) -> some View
  {
    HStack(alignment: .top, spacing: 16) {
      VStack(spacing: 0) {
        Text(number)
          .font(.system(.headline, design: .rounded).weight(.bold))
          .foregroundStyle(.white)
          .frame(width: 34, height: 34)
          .background(MetasequoiaTheme.cone, in: Circle())
        if drawsLine {
          Rectangle()
            .fill(MetasequoiaTheme.needle.opacity(0.3))
            .frame(width: 2, height: 54)
        }
      }

      VStack(alignment: .leading, spacing: 5) {
        Text(title)
          .font(.headline)
          .foregroundStyle(.primary)
        Text(detail)
          .font(.subheadline)
          .foregroundStyle(.secondary)
          .fixedSize(horizontal: false, vertical: true)
      }
      .padding(.top, 5)

      Spacer(minLength: 0)
    }
    .padding(.top, 18)
  }

  private func openSettings() {
    guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
    UIApplication.shared.open(url)
  }
}

#Preview {
  OnboardingView()
}

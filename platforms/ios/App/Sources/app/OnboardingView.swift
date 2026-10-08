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
/// 除了五笔的几个开关和「沿用上次的中英文」由键盘从 App Group 读取，这里其余各项都以共享偏好文档为准。每个控件都写这份文档，页面每次写完都重新读回，免得显示一个键盘其实没有的设置。
struct InputSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @AppStorage(WubiMixedPinyinPreference.enabledKey, store: WubiMixedPinyinPreference.defaults)
  private var wubiMixedPinyin = MSIMEAppEdition.wubiMixedPinyinDefault
  @AppStorage(WubiCodeHintPreference.enabledKey, store: WubiCodeHintPreference.defaults)
  private var wubiCodeHint = true
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
  @State private var addingLanguage = false
  @State private var failedGroup: PageGroup?

  private enum PageGroup { case languages, chinese, helpcode, translation, more }

  /// 语言选项面板里的一个选项。
  private enum LanguageChoice: Hashable {
    case scheme(ChineseInputScheme)
    case wubi(String)
    case remove
  }

  private static let saveFailure = "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"

  /// 页面可以提供的方案：本版本的全部条目，键盘扩展没带粤拼、注音、笔画的词库时去掉这几项，因为键盘那边也会把它们藏起来。
  private static let availableSchemes = ChineseInputScheme.allCases.filter(\.isOfferedByEdition)
    .filter { !$0.needsLanguageDictionary || InputLanguage.installedDictionarySchemes.contains($0) }

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "语言与方案", footer: footer(.languages)) { languageCard }
        chineseGroup
        helpcodeGroup
        translationGroup
        moreGroup
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("输入")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reloadPreferences)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reloadPreferences() }
    }
  }

  private func footer(_ group: PageGroup) -> String? {
    failedGroup == group ? Self.saveFailure : nil
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
    let schemes = available(language)
    if schemes.contains(inputScheme) && enabledSchemes.contains(inputScheme) { return inputScheme }
    return schemes.first(where: enabledSchemes.contains)
  }

  @ViewBuilder private var languageCard: some View {
    let languages = shownLanguages
    let addable = addableLanguages
    ForEach(Array(languages.enumerated()), id: \.element) { index, language in
      if index > 0 { DesignDivider(leading: 58) }
      let shown = shownScheme(language)
      InputLanguageRow(language: language, value: shown?.shortLabel, options: options(for: language),
                       selected: selectedChoice(in: language)) { choose($0, in: language) }
    }
    if addingLanguage {
      ForEach(addable) { language in
        DesignDivider(leading: 0)
        AddableLanguageRow(language: language) { add(language) }
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
          Text(addingLanguage ? "完成" : "添加语言").font(.system(size: 17))
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

  private func selectedChoice(in language: InputLanguage) -> LanguageChoice? {
    guard language.schemes.contains(inputScheme), enabledSchemes.contains(inputScheme) else { return nil }
    return inputScheme == .wubi ? .wubi(wubiProfile) : .scheme(inputScheme)
  }

  /// 语言的选项面板：按顺序列出它的方案，双拼和五笔各开一层二级面板，不是仅剩的最后一个语言时再加一项移除。每个方案选项保留 `inputScheme_<rawValue>` 这个 id。
  private func options(for language: InputLanguage) -> [DesignOption<LanguageChoice>] {
    let schemes = available(language)
    let shuangpin = schemes.filter { $0.shuangpinProfile != nil }
    var options: [DesignOption<LanguageChoice>] = []
    for scheme in schemes {
      if scheme.shuangpinProfile != nil {
        guard scheme == shuangpin.first else { continue }
        if shuangpin.count == 1 {
          options.append(schemeOption(scheme, title: scheme.shortLabel))
        } else {
          options.append(DesignOption(title: "双拼", value: .scheme(scheme), identifier: "inputSchemeGroup_shuangpin",
                                      children: shuangpin.map { schemeOption($0, title: optionTitle($0)) }))
        }
      } else if scheme == .wubi {
        options.append(DesignOption(title: "五笔", value: .scheme(.wubi), identifier: "inputSchemeGroup_wubi",
                                    children: WubiProfilePreference.profiles.map { profile in
          DesignOption(title: profile == "wubi98" ? "五笔 98" : "五笔 86", value: .wubi(profile),
                       identifier: "inputScheme_wubi_\(profile)")
        }))
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
      record(InputSchemePreference.select(scheme), .languages)
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
      let runnable = remaining.filter(Self.availableSchemes.contains)
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
    if saved { ToastCenter.shared.show("已添加\(language.name)") }
    reloadPreferences()
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
      DesignToggleRow(title: "拼音纠错", subtitle: "纠正相邻键误触和字母顺序颠倒",
                      isOn: Binding(get: { autocorrect }, set: saveAutocorrect))
        .accessibilityIdentifier("pinyinAutocorrectToggle")
      DesignDivider()
      DesignToggleRow(title: "模糊音", subtitle: "如 z/zh、an/ang 不分", isOn: Binding(get: { fuzzy.enabled }, set: { enabled in
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
    var options = HelpcodeSettingsView.schemas.map { DesignOption(title: $0.1, value: $0.0) }
    // 引擎认识但这份列表里没有的用户码表，仍按它的 id 显示。
    if !options.contains(where: { $0.value == schema }) { options.append(DesignOption(title: schema, value: schema)) }
    return DesignGroup(title: "辅助码", footer: footer(.helpcode)) {
      DesignSelectRow(title: "辅助码方案", subtitle: family.label, options: options,
                      selection: Binding(get: { schema }, set: { value in
                        guard value != schema else { return }
                        record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                          HelpcodeSettingsView.merge("schema", value, into: family, document: &$0)
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
    return DesignGroup(title: "翻译", footer: footer(.translation)) {
      // 键盘只在有释义时才显示翻译，所以离线英文释义打开之前，翻译各行不可用。
      DesignToggleRow(title: "候选词翻译",
                      subtitle: glossOn ? "在候选词下方显示\(primary.title)释义" : "打开离线英文释义后可用",
                      isOn: habit(\.onlineTranslations))
        .disabled(!glossOn).opacity(glossOn ? 1 : 0.5)
        .accessibilityIdentifier("candidateTranslationOnline")
      DesignDivider()
      DesignToggleRow(title: "离线英文释义", subtitle: "用随键盘打包的离线词库，不联网", isOn: habit(\.glossEnabled))
        .accessibilityIdentifier("candidateGlossToggle")
      DesignDivider()
      DesignSelectRow(title: "翻译目标语言",
                      options: languages.indices.map { DesignOption(title: languages[$0].title, value: $0) },
                      selection: habit(\.primaryLanguage), sheetTitle: "翻译目标语言",
                      identifier: "candidateTranslationPrimaryPicker")
        .disabled(!glossOn).opacity(glossOn ? 1 : 0.5)
      DesignDivider()
      DesignSelectRow(title: "第二种语言",
                      options: [DesignOption(title: "不显示", value: -1)]
                        + languages.indices.filter { $0 != habits.primaryLanguage }
                          .map { DesignOption(title: languages[$0].title, value: $0) },
                      selection: habit(\.secondaryLanguage), sheetTitle: "第二种语言",
                      identifier: "candidateTranslationSecondaryPicker")
        .disabled(!glossOn).opacity(glossOn ? 1 : 0.5)
      DesignDivider()
      navRow("翻译服务", subtitle: "联网翻译需要在这里选择服务", identifier: "translationProviderLink") {
        TranslationProviderSettingsView()
      }
    }
  }

  /// 保存一个输入习惯字段的绑定；保存失败就重新读取，让控件显示实际存下的值。
  private func habit<Value>(_ field: WritableKeyPath<InputHabitSettings, Value>) -> Binding<Value> {
    Binding(get: { habits[keyPath: field] }, set: { value in
      let saved = InputHabitPreference.update { $0[keyPath: field] = value }
      if let saved { habits = saved }
      record(saved != nil, .translation)
    })
  }

  // MARK: - 更多

  private var moreGroup: some View {
    DesignGroup(title: "更多", footer: footer(.more)) {
      if enabledSchemes.contains(.wubi) {
        DesignToggleRow(title: "编码打不出时用拼音候选", subtitle: "五笔词库答不上时用同一串字母查全拼", isOn: $wubiMixedPinyin)
          .accessibilityIdentifier("wubiMixedPinyin")
        DesignDivider()
        DesignToggleRow(title: "候选显示剩余编码", subtitle: "在候选后标出还要输入的字母", isOn: $wubiCodeHint)
          .accessibilityIdentifier("wubiCodeHint")
        DesignDivider()
      }
      DesignSelectRow(title: "默认中英文", subtitle: "新打开的键盘从这里开始",
                      options: [DesignOption(title: "中文", value: false), DesignOption(title: "英文", value: true)],
                      selection: Binding(get: { startsInEnglish }, set: { english in
                        guard english != startsInEnglish else { return }
                        startsInEnglish = english
                        record(MetasequoiaInputSessionBridge.updateSharedPreferences {
                          $0["default_ime_mode"] = english ? "english" : "chinese"
                        }, .more)
                        reloadPreferences()
                      }),
                      sheetTitle: "默认中英文", identifier: "defaultImeModePicker")
      DesignDivider()
      DesignToggleRow(title: "沿用上次的中英文", subtitle: "新打开的键盘沿用上次按中/英键选的模式",
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

  private func navRow<Destination: View>(_ title: String, subtitle: String? = nil, identifier: String,
                                         @ViewBuilder destination: () -> Destination) -> some View {
    NavigationLink(destination: destination()) {
      DesignNavRowLabel(title: title, subtitle: subtitle)
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
        Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold))
          .foregroundStyle(MetasequoiaTheme.sub.opacity(0.55))
          .accessibilityHidden(true)
      }
      .padding(.horizontal, 16)
      .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
      .contentShape(Rectangle())
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityLabel(language.name)
    .accessibilityValue(value ?? "")
    .accessibilityIdentifier("inputLanguage_\(language.rawValue)")
    .designOptionSheet(isPresented: $presented, title: language.name, message: "选择输入方案", options: options,
                       selected: selected, onSelect: onSelect)
  }
}

/// 添加语言里可添加的一个语言：描边图块、次要色的语言名和强调色的「添加」。点整行即可添加。
private struct AddableLanguageRow: View {
  let language: InputLanguage
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
        Text("添加").font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.accent)
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
    .background(Color(uiColor: .systemGroupedBackground).ignoresSafeArea())
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

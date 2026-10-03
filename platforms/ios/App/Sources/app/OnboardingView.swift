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

struct InputSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @AppStorage(KeyboardFeedbackPreference.soundKey, store: KeyboardFeedbackPreference.defaults)
  private var soundEnabled = true
  @AppStorage(KeyboardFeedbackPreference.hapticsKey, store: KeyboardFeedbackPreference.defaults)
  private var hapticsEnabled = false
  @AppStorage(KeyboardFeedbackPreference.strengthKey, store: KeyboardFeedbackPreference.defaults)
  private var hapticStrength = KeyboardHapticStrength.medium.rawValue
  @AppStorage(WubiMixedPinyinPreference.enabledKey, store: WubiMixedPinyinPreference.defaults)
  private var wubiMixedPinyin = false
  @AppStorage(WubiCodeHintPreference.enabledKey, store: WubiCodeHintPreference.defaults)
  private var wubiCodeHint = true
  @State private var previewFeedback: UIImpactFeedbackGenerator?
  @State private var inputScheme = InputSchemePreference.scheme
  @State private var enabledSchemes = InputSchemePreference.enabledSchemes
  @State private var usesTraditionalOutput = ChineseOutputPreference.usesTraditional
  @State private var startsInEnglish = false
  @State private var defaultModeSaveFailed = false
  @State private var schemeSaveFailed = false
  @State private var outputSaveFailed = false
  @State private var wubiProfile = WubiProfilePreference.profile
  @State private var wubiProfileSaveFailed = false
  @State private var remembersImeMode = false
  /// The shared document as last read, for the candidate preview at the top (dc.html: 输入 leads with the same card as 主题 and 候选栏).
  @State private var document: [String: Any]?
  @Environment(\.colorScheme) private var colorScheme

  var body: some View {
    Form {
        Section {
          CandidatePreviewCard(theme: KeyboardTheme.resolve(document: document), document: document,
                               systemDark: colorScheme == .dark)
        }
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets())
        Section {
          ForEach(ChineseInputScheme.allCases, id: \.self) { scheme in
            HStack {
              Button {
                schemeSaveFailed = !InputSchemePreference.save(scheme: scheme, enabled: enabledSchemes)
                reloadPreferences()
              } label: {
                HStack {
                  Text(scheme.title).foregroundStyle(.primary)
                  Spacer()
                  if inputScheme == scheme {
                    Image(systemName: "checkmark")
                      .foregroundStyle(MetasequoiaTheme.accent)
                      .accessibilityHidden(true)
                  }
                }
              }
              .buttonStyle(.plain)
              .accessibilityIdentifier("inputScheme_\(scheme.rawValue)")
              .accessibilityValue(inputScheme == scheme ? "已选择" : "未选择")
              .accessibilityAddTraits(inputScheme == scheme ? [.isSelected] : [])
              .disabled(!enabledSchemes.contains(scheme))
              Toggle(scheme.title, isOn: Binding(get: { enabledSchemes.contains(scheme) }, set: { enabled in
                var selection = enabledSchemes
                if enabled { selection.append(scheme) } else { selection.removeAll { $0 == scheme } }
                schemeSaveFailed = !InputSchemePreference.save(scheme: inputScheme, enabled: selection)
                reloadPreferences()
              }))
              .labelsHidden()
              .disabled(enabledSchemes.count == 1 && enabledSchemes.contains(scheme))
              .accessibilityIdentifier("enabledInputScheme_\(scheme.rawValue)")
            }

          }
        } header: {
          Text("输入方案")
        } footer: {
          Text(schemeSaveFailed
            ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
            : "开启的方案会显示在键盘快捷切换中，至少保留一种。点击名称设为当前方案。粤拼、大千注音和笔画读取随安装包附带的语言词库，没有词库时键盘不会显示对应方案；越南语和藏文切换回来时仍是原来的中文方案；藏文按威利转写（EWTS）输入，空格加音节点、斜杠加垂符。左右滑动空格可移动光标；滑动前会先完成当前输入。")
        }

        Section("高情商回复") {
          Text("复制对方的话，点键盘工具栏上的回复按钮打开高情商回复面板，点“粘贴”后选择九宫格里的回复风格。支持帮你回、帮润色和换一句，点选回复插入聊天输入框。")
            .font(.footnote).foregroundStyle(.secondary)
          NavigationLink(destination: ServiceSettingsView(kind: .ai)) {
            Label("配置键盘 AI", systemImage: "sparkles")
          }
        }

        if enabledSchemes.contains(.wubi) {
          Section("五笔") {
            Picker("码表", selection: Binding(get: { wubiProfile }, set: { profile in
              wubiProfile = profile
              wubiProfileSaveFailed = !WubiProfilePreference.save(profile)
              reloadPreferences()
            })) {
              ForEach(WubiProfilePreference.profiles, id: \.self) { Text(WubiProfilePreference.title($0)).tag($0) }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("wubiProfilePicker")
            Text(wubiProfileSaveFailed
              ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
              : "86 版与 98 版的字根和编码不同，各用各的词库；个人词条和调频记录也分开保存，切换版本不会互相影响。")
              .font(.footnote).foregroundStyle(.secondary)
            Toggle("编码打不出时用拼音候选", isOn: $wubiMixedPinyin)
              .accessibilityIdentifier("wubiMixedPinyin")
            Text("五笔词库答不上当前编码时，用同一串字母查全拼。词库答得上的编码不受影响。")
              .font(.footnote).foregroundStyle(.secondary)
            Toggle("候选显示剩余编码", isOn: $wubiCodeHint)
              .accessibilityIdentifier("wubiCodeHint")
            Text("在候选后标出还要输入的字母；完整码、拼音回退和本地输入候选不标注。")
              .font(.footnote).foregroundStyle(.secondary)
          }
        }

        Section {
          NavigationLink(destination: FuzzyPinyinSettingsView()) {
            Label("模糊音", systemImage: "waveform.path")
          }.accessibilityIdentifier("fuzzyPinyinSettingsLink")
          NavigationLink(destination: PunctuationSettingsView()) {
            Label("标点", systemImage: "textformat.abc.dottedunderline")
          }.accessibilityIdentifier("punctuationSettingsLink")
          NavigationLink(destination: HelpcodeSettingsView()) {
            Label("辅助码", systemImage: "character.magnify")
          }.accessibilityIdentifier("helpcodeSettingsLink")
          NavigationLink(destination: LocalModeSettingsView()) {
            Label("快捷模式", systemImage: "textformat.123")
          }.accessibilityIdentifier("localModeSettingsLink")
          NavigationLink(destination: ClipboardHistorySettingsView()) {
            Label("剪贴板历史", systemImage: "doc.on.clipboard")
          }.accessibilityIdentifier("clipboardHistorySettingsLink")
        }

        Section {
          Picker("打开键盘时", selection: Binding(get: { startsInEnglish }, set: { english in
            startsInEnglish = english
            defaultModeSaveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
              $0["default_ime_mode"] = english ? "english" : "chinese"
            }
            if defaultModeSaveFailed { reloadPreferences() }
          })) {
            Text("中文").tag(false)
            Text("英文").tag(true)
          }
          .pickerStyle(.segmented)
          .accessibilityIdentifier("defaultImeModePicker")
          Toggle("沿用上次的中英文", isOn: Binding(get: { remembersImeMode }, set: { enabled in
            remembersImeMode = enabled
            ImeModeMemoryPreference.setEnabled(enabled)
          }))
          .accessibilityIdentifier("remembersImeModeToggle")
        } header: {
          Text("默认中英文")
        } footer: {
          Text(defaultModeSaveFailed
            ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
            : remembersImeMode
              ? "新打开的键盘沿用你上次按中/英键选的模式，还没切换过时从上面的默认开始。网址、邮箱等输入框临时切到的英文不算。iOS 不告诉键盘正在哪个应用里输入，所以只能记住一个模式，不能像桌面端那样按应用记住。"
              : "新打开的键盘从这里开始，与桌面端同步。按中/英键切换后，这次打开的键盘保持你的选择。iOS 不告诉键盘正在哪个应用里输入，所以不像桌面端那样按应用记住中英文。")
        }

        Section {
          Picker("输出字形", selection: $usesTraditionalOutput) {
            Text("简体").tag(false)
            Text("繁体").tag(true)
          }
          .pickerStyle(.segmented)
          .accessibilityIdentifier("chineseOutputPicker")
          .onChange(of: usesTraditionalOutput) { value in
            guard value != ChineseOutputPreference.usesTraditional else { return }
            outputSaveFailed = !ChineseOutputPreference.save(value)
            if outputSaveFailed { reloadPreferences() }
          }
        } header: {
          Text("简繁体")
        } footer: {
          Text(outputSaveFailed ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。" : "应用于候选词和输入的文字。")
        }

        Section {
          Toggle("按键音", isOn: $soundEnabled)
            .accessibilityIdentifier("keyboardSoundToggle")
          if KeyboardFeedbackPreference.hapticsAvailable {
            Toggle("按键振动", isOn: $hapticsEnabled)
              .accessibilityIdentifier("keyboardHapticsToggle")
              .onChange(of: hapticsEnabled) { enabled in if enabled { previewHaptics() } }
          }
          if KeyboardFeedbackPreference.hapticsAvailable && hapticsEnabled {
            Picker("振动强度", selection: $hapticStrength) {
              ForEach(KeyboardHapticStrength.allCases, id: \.rawValue) { strength in
                Text(strength.title).tag(strength.rawValue)
              }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("keyboardHapticStrengthPicker")
            .onChange(of: hapticStrength) { _ in previewHaptics() }
            Button("试一下振动", action: previewHaptics)
              .accessibilityIdentifier("previewKeyboardHaptics")
          }
        } header: {
          Text("按键反馈")
        } footer: {
          Text(KeyboardFeedbackPreference.hapticsAvailable
            ? "按键音受系统静音设置控制；振动效果取决于设备与系统支持。"
            : "按键音受系统静音设置控制。")
        }

    }
    .navigationTitle("输入")
    .navigationBarTitleDisplayMode(.inline)
      .onAppear(perform: reloadPreferences)
      .onChange(of: scenePhase) { phase in
        if phase == .active { reloadPreferences() }
      }
  }

  private func previewHaptics() {
    guard hapticsEnabled else { return }
    let strength = KeyboardHapticStrength(rawValue: hapticStrength) ?? .medium
    let generator = UIImpactFeedbackGenerator(style: strength.style)
    previewFeedback = generator
    generator.impactOccurred(intensity: strength.intensity)
    generator.prepare()
  }

  private func reloadPreferences() {
    inputScheme = InputSchemePreference.scheme
    enabledSchemes = InputSchemePreference.enabledSchemes
    usesTraditionalOutput = ChineseOutputPreference.usesTraditional
    document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    if let document { WubiProfilePreference.mirror(document) }
    wubiProfile = WubiProfilePreference.profile
    startsInEnglish = document?["default_ime_mode"] as? String == "english"
    remembersImeMode = ImeModeMemoryPreference.isEnabled()
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

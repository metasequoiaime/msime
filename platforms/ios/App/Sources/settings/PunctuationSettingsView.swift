import SwiftUI

/// Punctuation rules.
///
/// These switches have no App Group compatibility key: they live only in the shared preference document, which is what the keyboard's session reads, so this page reads and writes that document directly. The keyboard hands a change to its live session the next time it appears.
struct PunctuationSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var smart = true
  @State private var repeatToChinese = true
  @State private var spaceConvert = false
  @State private var directDigit = true
  @State private var directLetter = true
  @State private var paired = true
  @State private var lock = "follow"
  @State private var width = CharacterWidthPreference.halfwidth
  @State private var saveFailed = false

  var body: some View {
    Form {
      Section {
        Toggle("智能标点", isOn: stored("smart_punctuation", $smart))
          .accessibilityIdentifier("smartPunctuation")
      } footer: {
        Text("中文标点模式下，字母或数字后的 , . : 自动使用英文标点。")
      }
      .designRow()
      Section {
        Toggle(isOn: stored("smart_punctuation_repeat", $repeatToChinese)) {
          labelled("重复标点转中文", "输出英文标点后，2 秒内再次输入同一标点时替换为中文标点")
        }.accessibilityIdentifier("smartPunctuationRepeat")
        Toggle(isOn: stored("smart_punctuation_space_convert", $spaceConvert)) {
          labelled("中文标点后按空格转换", "刚输入中文标点后按空格，转换为对应英文标点")
        }.accessibilityIdentifier("smartPunctuationSpaceConvert")
        Toggle(isOn: stored("smart_punctuation_direct_digit", $directDigit)) {
          labelled("数字后直出", "数字后输入逗号、句点或冒号时保留英文标点")
        }.accessibilityIdentifier("smartPunctuationDirectDigit")
        Toggle(isOn: stored("smart_punctuation_direct_letter", $directLetter)) {
          labelled("字母后直出", "字母后输入逗号、句点或冒号时保留英文标点")
        }.accessibilityIdentifier("smartPunctuationDirectLetter")
      } footer: {
        Text("以上规则在智能标点开启时生效。")
      }
      .disabled(!smart)
      .designRow()
      Section {
        Toggle(isOn: stored("paired_punctuation", $paired)) {
          labelled("成对标点自动补全", "输入左侧括号或引号时同时补上右侧")
        }.accessibilityIdentifier("pairedPunctuation")
      }
      .designRow()
      Section {
        Toggle(isOn: Binding(get: { width == CharacterWidthPreference.fullwidth },
                             set: { stored(CharacterWidthPreference.key, $width).wrappedValue =
                                      $0 ? CharacterWidthPreference.fullwidth : CharacterWidthPreference.halfwidth })) {
          labelled("全角输入", "将英文字符和空格提交为全角形式")
        }.accessibilityIdentifier("characterWidth")
      } footer: {
        Text("键盘下次出现时生效；键盘功能菜单里的「全角」开关只临时切换当前键盘。")
      }
      .designRow()
      Section {
        Picker("固定标点", selection: stored("punctuation_lock", $lock)) {
          Text("跟随中英文状态").tag("follow")
          Text("始终使用中文标点").tag("chinese")
          Text("始终使用英文标点").tag("english")
        }
        .accessibilityIdentifier("punctuationLock")
      } footer: {
        Text(saveFailed ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。" : "切换中英文时的标点形态。")
      }
      .designRow()
    }
    .designPage()
    .navigationTitle("标点").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { phase in
      if phase == .active { reload() }
    }
  }

  private func labelled(_ title: String, _ detail: String) -> some View {
    VStack(alignment: .leading, spacing: 2) {
      Text(title)
      Text(detail).font(.footnote).foregroundStyle(.secondary)
    }
  }

  /// A binding that writes the shared document as it changes; a refused write puts the stored value back so the page never shows a setting the keyboard does not have.
  private func stored<Value>(_ key: String, _ state: Binding<Value>) -> Binding<Value> {
    Binding(get: { state.wrappedValue }, set: { value in
      state.wrappedValue = value
      saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = value }
      if saveFailed { reload() }
    })
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    smart = preferences["smart_punctuation"] as? Bool ?? smart
    repeatToChinese = preferences["smart_punctuation_repeat"] as? Bool ?? repeatToChinese
    spaceConvert = preferences["smart_punctuation_space_convert"] as? Bool ?? spaceConvert
    directDigit = preferences["smart_punctuation_direct_digit"] as? Bool ?? directDigit
    directLetter = preferences["smart_punctuation_direct_letter"] as? Bool ?? directLetter
    paired = preferences["paired_punctuation"] as? Bool ?? paired
    lock = preferences["punctuation_lock"] as? String ?? lock
    width = CharacterWidthPreference.value(in: preferences) ?? width
  }
}

import SwiftUI

/// Punctuation rules.
///
/// These switches have no App Group compatibility key: they live only in the shared preference document, which is what the keyboard's session reads, so this page reads and writes that document directly. The keyboard hands a change to its live session the next time it appears.
struct PunctuationSettingsView: View {
  @State private var smart = true
  @State private var repeatToChinese = true
  @State private var spaceConvert = false
  @State private var directDigit = true
  @State private var directLetter = true
  @State private var paired = true
  @State private var lock = "follow"
  @State private var width = CharacterWidthPreference.halfwidth
  @State private var spacing = false
  @State private var quoteStyle = "curly"
  @State private var bracketStyle = "solid"
  @State private var saveFailed = false

  var body: some View {
    Form {
      // 两项只在中文标点下起作用，排在本页最前，紧接「表达」页上的「中文标点」。
      Section {
        Picker("引号样式", selection: stored("quote_style", $quoteStyle)) {
          Text("弯引号 “ ” ‘ ’").tag("curly")
          Text("直角引号 「 」『 』").tag("corner")
          Text("繁体输出时用直角引号").tag("corner_when_traditional")
        }
        .accessibilityIdentifier("quoteStyle")
        Picker("方头括号样式", selection: stored("bracket_style", $bracketStyle)) {
          Text("实心 【 】").tag("solid")
          Text("空心 〖 〗").tag("hollow")
        }
        .accessibilityIdentifier("bracketStyle")
      } footer: {
        Text("中文标点下输入 ' 和 \" 时的引号、输入 [ ] 时的括号。直角引号：' 出「」，\" 出『』。")
      }
      .designRow()
      Section {
        Toggle("智能标点", isOn: stored("smart_punctuation", $smart))
          .accessibilityIdentifier("smartPunctuation")
      } footer: {
        Text("中文标点模式下，字母或数字后的 , . : 自动使用英文标点。")
      }
      .designRow()
      Section {
        Toggle(isOn: stored("smart_punctuation_repeat", $repeatToChinese)) {
          SettingsOptionLabel(title: "重复标点转中文", info: "输出英文标点后，2 秒内再次输入同一标点时替换为中文标点")
        }.accessibilityIdentifier("smartPunctuationRepeat")
        Toggle(isOn: stored("smart_punctuation_space_convert", $spaceConvert)) {
          SettingsOptionLabel(title: "中文标点后按空格转换", info: "刚输入中文标点后按空格，转换为对应英文标点")
        }.accessibilityIdentifier("smartPunctuationSpaceConvert")
        Toggle(isOn: stored("smart_punctuation_direct_digit", $directDigit)) {
          SettingsOptionLabel(title: "数字后直出", info: "数字后输入逗号、句点或冒号时保留英文标点")
        }.accessibilityIdentifier("smartPunctuationDirectDigit")
        Toggle(isOn: stored("smart_punctuation_direct_letter", $directLetter)) {
          SettingsOptionLabel(title: "字母后直出", info: "字母后输入逗号、句点或冒号时保留英文标点")
        }.accessibilityIdentifier("smartPunctuationDirectLetter")
      } footer: {
        Text("以上规则在智能标点开启时生效。")
      }
      .disabled(!smart)
      .designRow()
      Section {
        Toggle(isOn: stored("paired_punctuation", $paired)) {
          SettingsOptionLabel(title: "成对标点自动补全", info: "输入左侧括号或引号时同时补上右侧")
        }.accessibilityIdentifier("pairedPunctuation")
      }
      .designRow()
      Section {
        Toggle(isOn: Binding(get: { width == CharacterWidthPreference.fullwidth },
                             set: { stored(CharacterWidthPreference.key, $width).wrappedValue =
                                      $0 ? CharacterWidthPreference.fullwidth : CharacterWidthPreference.halfwidth })) {
          SettingsOptionLabel(title: "全角输入", info: "将英文字符和空格提交为全角形式")
        }.accessibilityIdentifier("characterWidth")
      } footer: {
        Text("键盘下次出现时生效；键盘功能菜单里的「全角」开关只临时切换当前键盘。")
      }
      .designRow()
      Section {
        Toggle(isOn: stored("chinese_english_spacing", $spacing)) {
          SettingsOptionLabel(title: "中英文之间加空格", info: "一次上屏的文字里，汉字与英文字母、数字相邻处自动加一个空格，如「用Rust写」上屏为「用 Rust 写」")
        }.accessibilityIdentifier("chineseEnglishSpacing")
      } footer: {
        Text("两次上屏之间不加；网址、日期等快捷模式和全角输入不受影响，译文和释义上屏、常用语、剪贴板和语音结果也不加。")
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
        Text(saveFailed ? SettingsSaveFailure.message : "切换中英文时的标点形态。")
      }
      .designRow()
    }
    .designPage()
    .navigationTitle("标点").navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive(reload)
  }

  /// A binding that writes the shared document as it changes; a refused write puts the stored value back so the page never shows a setting the keyboard does not have.
  private func stored<Value>(_ key: String, _ state: Binding<Value>) -> Binding<Value> {
    SharedPreferenceBinding.topLevel(key, state: state) { saved in
      saveFailed = !saved
      if !saved { reload() }
    }
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
    // 关着时文档里没有这个键，缺键就是关。
    spacing = preferences["chinese_english_spacing"] as? Bool ?? false
    // 等于缺省值时文档里没有这两个键，缺键就是弯引号、实心括号。
    quoteStyle = preferences["quote_style"] as? String ?? "curly"
    bracketStyle = preferences["bracket_style"] as? String ?? "solid"
  }
}

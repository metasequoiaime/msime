import SwiftUI

/// 表达：输入的文字如何输出，对应 Android 的 `ExpressionPage`。标点分组放三个日常标点开关，其余规则在标点页，点一行就到；智能分组放整句联想和英文联想；常用语打开键盘常用语面板发送的那些短语。
///
/// 标点开关和整句联想读写共享偏好文档，键盘下次出现时应用。英文联想沿用 App Group 开关（`EnglishSuggestionsPreference`），控制原文及唯一纠错候选和空格默认纠错；不增加第二个开关。
struct ExpressionSettingsView: View {
  @AppStorage(EnglishSuggestionsPreference.enabledKey, store: EnglishSuggestionsPreference.defaults)
  private var englishSuggestions = true
  @State private var chinesePunctuation = true
  @State private var paired = true
  @State private var smart = true
  @State private var lock = "follow"
  @State private var sentenceLevel = SentenceAssociationPreference.Level.standard
  @State private var failedGroup: PageGroup?

  private enum PageGroup { case punctuation, intelligence }

  private static let sentenceSubtitle = "更准确，但更耗电"

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "标点", footer: failedGroup == .punctuation ? SettingsSaveFailure.message : nil) {
          DesignToggleRow(title: "使用英文标点", subtitle: lockSubtitle, isOn: englishPunctuation)
            .disabled(lock != "follow")
            .accessibilityIdentifier("englishPunctuationToggle")
          DesignDivider()
          DesignToggleRow(title: "自动补全成对标点", info: "输入左侧括号或引号时同时补上右侧",
                          isOn: stored("paired_punctuation", $paired, group: .punctuation))
            .accessibilityIdentifier("pairedPunctuationToggle")
          DesignDivider()
          DesignToggleRow(title: "智能标点", info: "字母或数字后的 , . : 使用英文标点",
                          isOn: stored("smart_punctuation", $smart, group: .punctuation))
            .accessibilityIdentifier("smartPunctuationToggle")
          DesignDivider()
          NavigationLink(destination: PunctuationSettingsView()) {
            DesignNavRowLabel(title: "更多标点设置", info: "引号与括号样式、固定标点、全角输入、中英文空格和智能标点细则")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("punctuationSettingsLink")
        }

        DesignGroup(title: "智能", footer: failedGroup == .intelligence ? SettingsSaveFailure.message : nil) {
          DesignSelectRow(title: "整句联想", info: Self.sentenceSubtitle,
                          options: SentenceAssociationPreference.Level.allCases.map { DesignOption(title: $0.title, value: $0) },
                          selection: Binding(get: { sentenceLevel }, set: saveSentenceLevel),
                          sheetTitle: "整句联想", sheetMessage: Self.sentenceSubtitle,
                          identifier: "sentenceAssociationPicker")
          DesignDivider()
          DesignToggleRow(title: "英文联想", info: "保留原文并提供纠错，空格默认采用纠错结果", isOn: $englishSuggestions)
            .accessibilityIdentifier("englishSuggestionsToggle")
        }

        DesignGroup(title: "常用语", info: "在键盘的「常用语」面板中，点一下即可发送") {
          NavigationLink(destination: CommonPhrasesSettingsView()) {
            DesignNavRowLabel(title: "常用语")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier(SettingsPage.phrases.linkIdentifier)
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .designCanvasBackground()
    .navigationTitle("表达")
    .navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive(reload)
  }

  /// 使用英文标点是 `chinese_punctuation` 取反。`punctuation_lock` 不是跟随时，键盘里以它为准，此时开关显示锁定后的形式，不能在这里改。
  private var englishPunctuation: Binding<Bool> {
    Binding(get: {
      switch lock {
      case "chinese": false
      case "english": true
      default: !chinesePunctuation
      }
    }, set: { english in
      stored("chinese_punctuation", $chinesePunctuation, group: .punctuation).wrappedValue = !english
    })
  }

  private var lockSubtitle: String? {
    switch lock {
    case "chinese": "已锁定为中文标点"
    case "english": "已锁定为英文标点"
    default: nil
    }
  }

  /// 随改动写入共享文档某个顶层字段的绑定；写入被拒绝时恢复已存的值，让页面永远不会显示键盘实际没有的设置。
  private func stored(_ key: String, _ state: Binding<Bool>, group: PageGroup) -> Binding<Bool> {
    SharedPreferenceBinding.topLevel(key, state: state) { saved in record(saved, group: group) }
  }

  private func saveSentenceLevel(_ level: SentenceAssociationPreference.Level) {
    guard level != sentenceLevel else { return }
    sentenceLevel = level
    record(SentenceAssociationPreference.save(level), group: .intelligence)
  }

  private func record(_ saved: Bool, group: PageGroup) {
    failedGroup = saved ? nil : group
    if !saved { reload() }
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    chinesePunctuation = preferences["chinese_punctuation"] as? Bool ?? true
    paired = preferences["paired_punctuation"] as? Bool ?? true
    smart = preferences["smart_punctuation"] as? Bool ?? true
    lock = preferences["punctuation_lock"] as? String ?? "follow"
    sentenceLevel = SentenceAssociationPreference.level(in: preferences)
  }
}

import SwiftUI

/// 表达：输入的文字如何输出，对应 Android 的 `ExpressionPage`。标点分组放三个日常标点开关，其余规则在标点页，点一行就到；智能分组放整句联想和英文联想；常用语打开键盘常用语面板发送的那些短语。
///
/// 标点开关和整句联想只存在于共享偏好文档中，键盘会话读的也是它，所以本页直接读写这份文档；键盘下次出现时会把改动交给它正在运行的会话。英文联想是键盘每次按键都会读的 App Group 开关（`EnglishSuggestionsPreference`）。设计稿里的英文自动纠正没有共享键，所以这一行和 Android 一样是英文联想；设计稿里的发现短语没有放进来，因为 iOS 还不能安装社区短语包。
struct ExpressionSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @AppStorage(EnglishSuggestionsPreference.enabledKey, store: EnglishSuggestionsPreference.defaults)
  private var englishSuggestions = true
  @State private var chinesePunctuation = true
  @State private var paired = true
  @State private var smart = true
  @State private var lock = "follow"
  @State private var sentenceLevel = SentenceAssociationPreference.Level.standard
  @State private var failedGroup: PageGroup?

  private enum PageGroup { case punctuation, intelligence }

  private static let saveFailure = "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"
  private static let sentenceSubtitle = "更准确，但更耗电"

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "标点", footer: failedGroup == .punctuation ? Self.saveFailure : nil) {
          DesignToggleRow(title: "使用英文标点", subtitle: lockSubtitle, isOn: englishPunctuation)
            .disabled(lock != "follow")
            .accessibilityIdentifier("englishPunctuationToggle")
          DesignDivider()
          DesignToggleRow(title: "自动补全成对标点", subtitle: "输入左侧括号或引号时同时补上右侧",
                          isOn: stored("paired_punctuation", $paired, group: .punctuation))
            .accessibilityIdentifier("pairedPunctuationToggle")
          DesignDivider()
          DesignToggleRow(title: "智能标点", subtitle: "字母或数字后的 , . : 使用英文标点",
                          isOn: stored("smart_punctuation", $smart, group: .punctuation))
            .accessibilityIdentifier("smartPunctuationToggle")
          DesignDivider()
          NavigationLink(destination: PunctuationSettingsView()) {
            DesignNavRowLabel(title: "更多标点设置", subtitle: "固定标点、全角输入和智能标点细则")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("punctuationSettingsLink")
        }

        DesignGroup(title: "智能", footer: failedGroup == .intelligence ? Self.saveFailure : nil) {
          DesignSelectRow(title: "整句联想", subtitle: Self.sentenceSubtitle,
                          options: SentenceAssociationPreference.Level.allCases.map { DesignOption(title: $0.title, value: $0) },
                          selection: Binding(get: { sentenceLevel }, set: saveSentenceLevel),
                          sheetTitle: "整句联想", sheetMessage: Self.sentenceSubtitle,
                          identifier: "sentenceAssociationPicker")
          DesignDivider()
          DesignToggleRow(title: "英文联想", subtitle: "输入英文时在候选栏给出单词", isOn: $englishSuggestions)
            .accessibilityIdentifier("englishSuggestionsToggle")
        }

        DesignGroup(title: "常用语", footer: "在键盘的「常用语」面板中，点一下即可发送") {
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
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("表达")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reload() }
    }
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
    Binding(get: { state.wrappedValue }, set: { value in
      state.wrappedValue = value
      record(MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = value }, group: group)
    })
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

import SwiftUI

struct StatisticsKeyLanguagePicker: View {
  let languages: TypingKeyLanguages
  @Binding var selection: TypingKeyLanguages.Language

  var body: some View {
    HStack {
      Text("语言").font(.system(size: 15))
      Spacer()
      Picker("按键语言", selection: Binding(
        get: { languages.resolved(selection) },
        set: { selection = $0 }
      )) {
        ForEach(languages.available) { language in
          Text(language.title).tag(language)
        }
      }
      .pickerStyle(.menu)
      .labelsHidden()
      .tint(MetasequoiaTheme.accent)
      .accessibilityLabel("按键语言")
      .accessibilityIdentifier("statisticsKeyLanguage")
    }
    .frame(minHeight: 44)
    .statisticsCard(top: 0, horizontal: 16, bottom: 0)
  }
}

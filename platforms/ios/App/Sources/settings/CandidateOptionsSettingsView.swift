import SwiftUI
import UIKit

/// The candidate preview, text size and font, what the strip shows while spelling, 以词定字, cloud candidates, and what gets mixed into the Chinese candidates.
///
/// Like the punctuation page, these live only in the shared preference document, nested under `quanpin`, `word_character` and `mixed_input`, so each write merges one field into its object and leaves the rest of the object as stored. Cloud candidates are the exception: an iOS-only switch in the App Group (see CloudCandidatePreference). The keyboard hands a change to its live session the next time it appears. The candidate package, mode and colours are part of the custom theme and live on the 主题 page (CustomThemeCandidateSection).
struct CandidateOptionsSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var english = true
  @State private var minimumPrefix = 5
  @State private var emoji = false
  @State private var kaomoji = false
  @State private var wordCharacter = true
  @AppStorage(CloudCandidatePreference.key, store: CloudCandidatePreference.defaults)
  private var cloudCandidates = false
  @AppStorage(CandidatePageSizePreference.key, store: CandidatePageSizePreference.defaults)
  private var pageSize = CandidatePageSizePreference.defaultSize
  @AppStorage(EnglishSuggestionsPreference.enabledKey, store: EnglishSuggestionsPreference.defaults)
  private var englishSuggestions = true
  @State private var candidateSize = CandidateFontPreference.defaultCandidateSize
  @State private var preeditSize = CandidateFontPreference.defaultPreeditSize
  @State private var fontFamily = CandidateFontPreference.defaultFamily
  @State private var englishFamily: String?
  @State private var fontFamilies: [String] = []
  @State private var preeditStyle = CandidatePreeditStyle.pinyin.rawValue
  @State private var shuangpinRaw = true
  @State private var inlinePreedit = InlinePreeditPreference.style
  /// The shared document as last read, for the preview card at the top.
  @State private var document: [String: Any]?
  @Environment(\.colorScheme) private var colorScheme
  @State private var saveFailed = false
  /// A docked iPad keyboard has room for larger candidates than a phone strip; the keyboard applies the same limits when it draws.
  private let tablet = UIDevice.current.userInterfaceIdiom == .pad

  var body: some View {
    Form {
      Section {
        CandidatePreviewCard(theme: KeyboardTheme.resolve(document: document), document: document,
                             systemDark: colorScheme == .dark, fontSize: candidateSize)
      }
      .listRowBackground(Color.clear)
      .listRowInsets(EdgeInsets())
      Section {
        VStack(alignment: .leading, spacing: 6) {
          LabeledContent("候选字号", value: "\(candidateSize)px")
          Slider(value: fontSizeSlider, in: sliderRange, step: 1) {
            Text("候选字号")
          } minimumValueLabel: {
            Text("\(CandidateFontPreference.candidateRange(tablet: tablet).lowerBound)").font(.caption)
          } maximumValueLabel: {
            Text("\(CandidateFontPreference.candidateRange(tablet: tablet).upperBound)").font(.caption)
          }
          .accessibilityValue("\(candidateSize)px")
          .accessibilityIdentifier("candidateFontSize")
        }
        Stepper(value: storedTop(CandidateFontPreference.preeditKey, $preeditSize),
                in: CandidateFontPreference.preeditRange(tablet: tablet)) {
          VStack(alignment: .leading, spacing: 2) {
            Text("编码字号：\(preeditSize)")
            Text("shui'shan").font(Font(CandidateFontPreference.font(
              .subheadline, scale: CGFloat(preeditSize) / CGFloat(CandidateFontPreference.defaultPreeditSize))))
          }
        }.accessibilityIdentifier("candidatePreeditFontSize")
        NavigationLink {
          CandidateFontFamilyList(title: "中文字体", selection: fontFamily, none: nil) { family in
            write { $0[CandidateFontPreference.familyKey] = family ?? CandidateFontPreference.defaultFamily }
          }
        } label: {
          LabeledContent("中文字体", value: familyTitle(fontFamily))
        }.accessibilityIdentifier("candidateFontFamily")
        NavigationLink {
          CandidateFontFamilyList(title: "英文字体", selection: englishFamily, none: "跟随中文字体") { family in
            write { $0[CandidateFontPreference.englishFamilyKey] = family }
          }
        } label: {
          LabeledContent("英文字体", value: englishFamily.map(familyTitle) ?? "跟随中文字体")
        }.accessibilityIdentifier("candidateEnglishFont")
      } header: {
        Text("字号与字体")
      } footer: {
        Text((tablet
          ? "默认 18 和 15，与桌面端同步。候选栏会随字号变高；浮动的小键盘按手机的上限显示。"
          : "默认 18 和 15，与桌面端同步。候选栏会随字号变高；桌面端设得更大时，手机上最多显示到 24 和 20。")
          + "字体与桌面端同步；此设备没有的字体或缺的字使用系统字体。")
      }
      Section {
        Stepper(value: Binding(get: { CandidatePageSizePreference.clamped(pageSize) }, set: { pageSize = CandidatePageSizePreference.clamped($0) }),
                in: CandidatePageSizePreference.range) {
          labelled("每页候选数：\(CandidatePageSizePreference.clamped(pageSize))", "候选栏编号的候选个数，其余的展开候选面板查看")
        }.accessibilityIdentifier("candidatePageSize")
      } footer: {
        Text("默认 9 个，与符号键盘和 iPad 数字行的 1–9 对应；组字时按数字键选对应编号的候选。只在本机生效，不影响电脑上的候选窗口。")
      }
      Section {
        Picker(selection: Binding(get: { inlinePreedit }, set: { inlinePreedit = $0; InlinePreeditPreference.style = $0 })) {
          ForEach(InlinePreeditPreference.Style.allCases, id: \.self) { Text($0.title).tag($0) }
        } label: {
          labelled("行内预编辑", "把正在拼写的编码也写进输入框，像系统键盘那样带下划线显示")
        }.accessibilityIdentifier("inlinePreedit")
        Picker("候选栏预编辑", selection: storedTop(CandidatePreeditStyle.key, $preeditStyle)) {
          ForEach(CandidatePreeditStyle.allCases, id: \.self) { Text($0.title).tag($0.rawValue) }
        }.accessibilityIdentifier("candidatePreeditStyle")
        Toggle(isOn: storedTop("shuangpin_preedit_uses_raw", $shuangpinRaw)) {
          labelled("双拼显示原始按键", "关闭后显示按键对应的完整拼音，只对双拼生效")
        }.accessibilityIdentifier("shuangpinPreeditUsesRaw")
      } header: {
        Text("预编辑")
      } footer: {
        Text("行内预编辑默认关闭；个别 App 显示输入框里的组字不完整时可以关掉。韩语正在拼的音节、注音正在转换的文字、越南语正在拼的词和藏文正在拼的音节总是写在输入框里，不受这项影响。候选栏预编辑选「不显示」时，候选栏不再显示正在拼写的编码，把位置留给候选；已选定的半个词和快捷模式的名称仍会显示。")
      }
      Section {
        Toggle(isOn: stored("word_character", "enabled", $wordCharacter)) {
          labelled("以词定字", "长按两个字以上的候选，可以只上屏它的首字或末字")
        }.accessibilityIdentifier("wordCharacter")
      }
      Section {
        Toggle(isOn: $cloudCandidates) {
          labelled("云候选", "输入停顿时向 Google 输入法服务查询候选，排进候选栏")
        }.accessibilityIdentifier("cloudCandidates")
      } footer: {
        Text("默认关闭。开启后，正在输入的编码会发送到 Google 输入法服务；还需要在系统设置中允许键盘完全访问。")
      }
      Section {
        Toggle(isOn: $englishSuggestions) {
          labelled("英文单词提示", "英文输入时在候选栏提示常用单词，点选补全当前单词")
        }.accessibilityIdentifier("englishSuggestions")
      }
      Section {
        Toggle(isOn: stored("mixed_input", "english", $english)) {
          labelled("中英混输", "中文输入时在候选中补充英文单词")
        }.accessibilityIdentifier("mixedEnglish")
        Stepper(value: stored("mixed_input", "minimum_prefix", $minimumPrefix), in: 1...8) {
          labelled("触发字母数：\(minimumPrefix)", "输入的字母达到这个长度后才出现英文候选")
        }
        .disabled(!english)
        .accessibilityIdentifier("mixedEnglishMinimumPrefix")
      }
      Section {
        Toggle(isOn: stored("mixed_input", "emoji", $emoji)) {
          labelled("emoji 混输", "在候选中加入匹配的 emoji，排在英文候选之后")
        }.accessibilityIdentifier("mixedEmoji")
        Toggle(isOn: stored("mixed_input", "kaomoji", $kaomoji)) {
          labelled("颜文字混输", "在候选中加入匹配的颜文字，排在 emoji 之后")
        }.accessibilityIdentifier("mixedKaomoji")
      } footer: {
        if saveFailed { Text("设置没有保存，键盘可能正在写入同一份设置，请再试一次。") }
      }
    }
    .navigationTitle("候选栏").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
  }

  private var sliderRange: ClosedRange<Double> {
    let range = CandidateFontPreference.candidateRange(tablet: tablet)
    return Double(range.lowerBound)...Double(range.upperBound)
  }

  /// The slider of dc.html L1991 over the same field and limits the stepper used; a drag writes the document once per whole step, not per frame.
  private var fontSizeSlider: Binding<Double> {
    let size = storedTop(CandidateFontPreference.candidateKey, $candidateSize)
    return Binding(get: { Double(candidateSize) }, set: { value in
      let rounded = Int(value.rounded())
      if rounded != candidateSize { size.wrappedValue = rounded }
    })
  }

  private func labelled(_ title: String, _ detail: String) -> some View {
    VStack(alignment: .leading, spacing: 2) {
      Text(title)
      Text(detail).font(.footnote).foregroundStyle(.secondary)
    }
  }

  /// A binding that merges one field into a nested object of the shared document; a refused write puts the stored value back.
  private func stored<Value>(_ object: String, _ key: String, _ state: Binding<Value>) -> Binding<Value> {
    Binding(get: { state.wrappedValue }, set: { value in
      state.wrappedValue = value
      saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences {
        var nested = $0[object] as? [String: Any] ?? [:]
        nested[key] = value
        $0[object] = nested
      }
      if saveFailed { reload() }
    })
  }

  /// A family name, marked when this device does not have it and the keyboard therefore skips it.
  private func familyTitle(_ family: String) -> String {
    CandidateFontPreference.isInstalled(family) ? family : "\(family)（未安装）"
  }

  private func write(_ mutate: (inout [String: Any]) -> Void) {
    saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences(mutate)
    reload()
  }

  /// A binding that writes one top-level field of the shared document; a refused write puts the stored value back.
  private func storedTop<Value>(_ key: String, _ state: Binding<Value>) -> Binding<Value> {
    Binding(get: { state.wrappedValue }, set: { value in
      state.wrappedValue = value
      saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = value }
      if saveFailed { reload() }
    })
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    let mixed = preferences["mixed_input"] as? [String: Any] ?? [:]
    english = mixed["english"] as? Bool ?? english
    minimumPrefix = (mixed["minimum_prefix"] as? NSNumber)?.intValue ?? minimumPrefix
    emoji = mixed["emoji"] as? Bool ?? emoji
    kaomoji = mixed["kaomoji"] as? Bool ?? kaomoji
    candidateSize = CandidateFontPreference.candidateSize(in: preferences, tablet: tablet)
    preeditSize = CandidateFontPreference.preeditSize(in: preferences, tablet: tablet)
    fontFamily = preferences[CandidateFontPreference.familyKey] as? String ?? CandidateFontPreference.defaultFamily
    englishFamily = (preferences[CandidateFontPreference.englishFamilyKey] as? String).flatMap { $0.isEmpty ? nil : $0 }
    fontFamilies = CandidateFontPreference.families(in: preferences)
    preeditStyle = CandidatePreeditStyle(in: preferences).rawValue
    document = preferences
    shuangpinRaw = preferences["shuangpin_preedit_uses_raw"] as? Bool ?? true
    wordCharacter = (preferences["word_character"] as? [String: Any])?["enabled"] as? Bool ?? wordCharacter
  }
}

/// The font families this device has, each drawn in itself, for one of the candidate font fields. A name synced from another device that this one lacks stays listed at the top, so the page shows what the document says rather than silently picking something else.
private struct CandidateFontFamilyList: View {
  let title: String
  let selection: String?
  /// The title of the "no family" row, or nil when the field always names one; choosing it passes nil.
  let none: String?
  let choose: (String?) -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var query = ""
  private let installed = UIFont.familyNames.sorted()

  var body: some View {
    List {
      if let none, query.isEmpty { row(none, family: nil, font: .body) }
      if let selection, !installed.contains(selection), query.isEmpty {
        Section {
          row(selection, family: selection, font: .body)
        } footer: {
          Text("此设备没有这个字体，键盘会跳过它。")
        }
      }
      Section {
        ForEach(installed.filter { query.isEmpty || $0.localizedCaseInsensitiveContains(query) }, id: \.self) { family in
          row(family, family: family, font: .custom(family, size: UIFont.preferredFont(forTextStyle: .body).pointSize,
                                                  relativeTo: .body))
        }
      }
    }
    .searchable(text: $query)
    .navigationTitle(title).navigationBarTitleDisplayMode(.inline)
  }

  private func row(_ title: String, family: String?, font: Font) -> some View {
    Button {
      choose(family)
      dismiss()
    } label: {
      HStack {
        Text(title).font(font).foregroundStyle(.primary)
        Spacer()
        if family == selection { Image(systemName: "checkmark").foregroundStyle(.tint) }
      }
    }
    .accessibilityAddTraits(family == selection ? .isSelected : [])
  }
}

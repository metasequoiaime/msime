import SwiftUI
import UIKit
import CoreFoundation

/// 候选栏：候选字号、字体、每页候选数、拼写时候选栏显示什么，以及中文候选里混入哪些内容（云候选、英文、emoji、颜文字、以词定字）。
///
/// Like the punctuation page, these live only in the shared preference document, nested under `quanpin`, `word_character` and `mixed_input`, so each write merges one field into its object and leaves the rest of the object as stored. Cloud candidates are the exception: an iOS-only switch in the App Group (see CloudCandidatePreference). The keyboard hands a change to its live session the next time it appears. The candidate package, mode and colours are part of the custom theme and live on the 主题 page (CustomThemeCandidateSection).
///
/// 设计稿里的候选栏高度和翻页两行没有做：键盘没有候选栏高度设置和翻页按钮，扩展也拿不到可用来翻页的硬件按键。
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
  @State private var saveFailed = false
  /// A docked iPad keyboard has room for larger candidates than a phone strip; the keyboard applies the same limits when it draws.
  private let tablet = UIDevice.current.userInterfaceIdiom == .pad

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "候选栏") {
          DesignSliderRow(title: "候选字号", value: fontSizeSlider, range: sliderRange, step: 1,
                          identifier: "candidateFontSize") { "\(Int($0))px" }
        }

        DesignGroup(title: "字体") {
          stepperRow("编码字号", value: "\(preeditSize)", binding: storedTop(CandidateFontPreference.preeditKey, $preeditSize),
                     in: CandidateFontPreference.preeditRange(tablet: tablet), identifier: "candidatePreeditFontSize") {
            Text("shui'shan").font(Font(CandidateFontPreference.font(
              .subheadline, scale: CGFloat(preeditSize) / CGFloat(CandidateFontPreference.defaultPreeditSize))))
              .foregroundStyle(MetasequoiaTheme.sub)
          }
          DesignDivider()
          NavigationLink {
            CandidateFontFamilyList(title: "中文字体", selection: fontFamily, none: nil) { family in
              write { $0[CandidateFontPreference.familyKey] = family ?? CandidateFontPreference.defaultFamily }
            }
          } label: {
            DesignNavRowLabel(title: "中文字体", value: familyTitle(fontFamily))
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("candidateFontFamily")
          DesignDivider()
          NavigationLink {
            CandidateFontFamilyList(title: "英文字体", selection: englishFamily, none: "跟随中文字体") { family in
              write { $0[CandidateFontPreference.englishFamilyKey] = family }
            }
          } label: {
            DesignNavRowLabel(title: "英文字体", value: englishFamily.map(familyTitle) ?? "跟随中文字体")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("candidateEnglishFont")
        }

        DesignGroup(title: "每页候选数") {
          stepperRow("每页候选数", value: "\(CandidatePageSizePreference.clamped(pageSize))",
                     binding: Binding(get: { CandidatePageSizePreference.clamped(pageSize) },
                                      set: { pageSize = CandidatePageSizePreference.clamped($0) }),
                     in: CandidatePageSizePreference.range, identifier: "candidatePageSize") {
            detail("编号的候选个数，其余的展开候选面板查看")
          }
        }

        DesignGroup(title: "预编辑") {
          DesignSelectRow(title: "行内预编辑", subtitle: "把正在拼写的编码也写进输入框",
                          options: InlinePreeditPreference.Style.allCases.map { DesignOption(title: $0.title, value: $0) },
                          selection: Binding(get: { inlinePreedit }, set: { inlinePreedit = $0; InlinePreeditPreference.style = $0 }),
                          sheetTitle: "行内预编辑", identifier: "inlinePreedit")
          DesignDivider()
          DesignSelectRow(title: "候选栏预编辑", subtitle: "候选栏上是否显示正在拼写的编码",
                          options: CandidatePreeditStyle.allCases.map { DesignOption(title: $0.title, value: $0.rawValue) },
                          selection: storedTop(CandidatePreeditStyle.key, $preeditStyle),
                          sheetTitle: "候选栏预编辑", identifier: "candidatePreeditStyle")
          DesignDivider()
          DesignToggleRow(title: "双拼显示原始按键", subtitle: "关闭后显示按键对应的完整拼音",
                          isOn: storedTop("shuangpin_preedit_uses_raw", $shuangpinRaw))
            .accessibilityIdentifier("shuangpinPreeditUsesRaw")
        }

        DesignGroup(title: "候选内容", footer: saveFailed ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。" : nil) {
          // 副标题就是隐私说明：只有打开这个开关，输入的编码才会离开设备。
          DesignToggleRow(title: "云候选", subtitle: "开启后正在输入的编码会发送到 Google 输入法服务获取候选，需允许完全访问",
                          isOn: $cloudCandidates)
            .accessibilityIdentifier("cloudCandidates")
          DesignDivider()
          DesignToggleRow(title: "英文单词提示", subtitle: "英文输入时提示常用单词，点选补全", isOn: $englishSuggestions)
            .accessibilityIdentifier("englishSuggestions")
          DesignDivider()
          DesignToggleRow(title: "中英混输", subtitle: "中文输入时在候选中补充英文单词",
                          isOn: stored("mixed_input", "english", $english))
            .accessibilityIdentifier("mixedEnglish")
          DesignDivider()
          stepperRow("触发字母数", value: "\(minimumPrefix)", binding: stored("mixed_input", "minimum_prefix", $minimumPrefix),
                     in: 1...8, identifier: "mixedEnglishMinimumPrefix") {
            detail("字母达到这个长度才出现英文候选")
          }
          .disabled(!english)
          .opacity(english ? 1 : 0.45)
          DesignDivider()
          DesignToggleRow(title: "emoji 混输", subtitle: "在候选中加入匹配的 emoji，紧跟在它描绘的那个词后面", isOn: stored("mixed_input", "emoji", $emoji))
            .accessibilityIdentifier("mixedEmoji")
          DesignDivider()
          DesignToggleRow(title: "颜文字混输", subtitle: "在候选中加入匹配的颜文字，排在同一个词的 emoji 之后", isOn: stored("mixed_input", "kaomoji", $kaomoji))
            .accessibilityIdentifier("mixedKaomoji")
          DesignDivider()
          DesignToggleRow(title: "以词定字", subtitle: "长按词语候选，只上屏首字或末字",
                          isOn: stored("word_character", "enabled", $wordCharacter))
            .accessibilityIdentifier("wordCharacter")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("候选栏").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { reload() }
    }
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

  private func detail(_ text: String) -> some View {
    Text(text).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
  }

  /// 带标题、说明行、当前值和步进器的一行，布局同 DesignKit 的行。
  private func stepperRow<Value: Strideable, Detail: View>(
    _ title: String, value: String, binding: Binding<Value>, in range: ClosedRange<Value>, identifier: String,
    @ViewBuilder detail: () -> Detail
  ) -> some View {
    HStack(spacing: 12) {
      VStack(alignment: .leading, spacing: 2) {
        Text(title).font(.system(size: 17)).foregroundStyle(.primary)
        detail().fixedSize(horizontal: false, vertical: true)
      }
      Spacer(minLength: 8)
      Text(value).font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub).monospacedDigit()
        .accessibilityHidden(true)
      Stepper(title, value: binding, in: range)
        .labelsHidden()
        .accessibilityValue(value)
        .accessibilityIdentifier(identifier)
    }
    .padding(.vertical, 8)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
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
    minimumPrefix = Self.minimumPrefix(mixed["minimum_prefix"]) ?? minimumPrefix
    emoji = mixed["emoji"] as? Bool ?? emoji
    kaomoji = mixed["kaomoji"] as? Bool ?? kaomoji
    candidateSize = CandidateFontPreference.candidateSize(in: preferences, tablet: tablet)
    preeditSize = CandidateFontPreference.preeditSize(in: preferences, tablet: tablet)
    fontFamily = preferences[CandidateFontPreference.familyKey] as? String ?? CandidateFontPreference.defaultFamily
    englishFamily = (preferences[CandidateFontPreference.englishFamilyKey] as? String).flatMap { $0.isEmpty ? nil : $0 }
    fontFamilies = CandidateFontPreference.families(in: preferences)
    preeditStyle = CandidatePreeditStyle(in: preferences).rawValue
    shuangpinRaw = preferences["shuangpin_preedit_uses_raw"] as? Bool ?? true
    wordCharacter = (preferences["word_character"] as? [String: Any])?["enabled"] as? Bool ?? wordCharacter
  }

  static func minimumPrefix(_ value: Any?) -> Int? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          let integer = Int(number.stringValue),
          NSNumber(value: integer).compare(number) == .orderedSame,
          (1...8).contains(integer) else { return nil }
    return integer
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

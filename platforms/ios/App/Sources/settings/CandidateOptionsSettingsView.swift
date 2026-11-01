import SwiftUI
import UIKit
import CoreFoundation

/// 候选栏：候选字号、字体、每页候选数、拼写时候选栏显示什么，以及中文候选里混入哪些内容（云候选、英文、符号、emoji、颜文字、以词定字）。
///
/// Like the punctuation page, these live only in the shared preference document, nested under `quanpin`, `word_character` and `mixed_input`, so each write merges one field into its object and leaves the rest of the object as stored. Cloud candidates are the exception: an iOS-only switch in the App Group (see CloudCandidatePreference). The keyboard hands a change to its live session the next time it appears. The candidate package, mode and colours are part of the custom theme and live on the 主题 page (CustomThemeCandidateSection).
///
/// 设计稿里的候选栏高度由「候选栏大小」（`candidate_scale_percent`）承担：候选栏连同字号、空闲时占同一行的工具栏一起缩放（#10137）。翻页一行没有做：键盘没有翻页按钮，扩展也拿不到可用来翻页的硬件按键。
struct CandidateOptionsSettingsView: View {
  @State private var multilingualEnabled = false
  @State private var multilingualLanguages = ["chinese", "english", "japanese"]
  @State private var mixedEnabled = true
  @State private var firstPageCount = 1
  @State private var english = true
  @State private var minimumPrefix = 5
  @State private var emoji = false
  @State private var kaomoji = false
  @State private var symbols = true
  @State private var wordCharacter = true
  @AppStorage(CloudCandidatePreference.key, store: CloudCandidatePreference.defaults)
  private var cloudCandidates = false
  @AppStorage(CandidatePageSizePreference.key, store: CandidatePageSizePreference.defaults)
  private var pageSize = CandidatePageSizePreference.defaultSize
  @AppStorage(EnglishSuggestionsPreference.enabledKey, store: EnglishSuggestionsPreference.defaults)
  private var englishSuggestions = true
  @State private var candidateSize = CandidateFontPreference.defaultCandidateSize
  @State private var preeditSize = CandidateFontPreference.defaultPreeditSize
  @State private var scalePercent = CandidateFontPreference.defaultScalePercent
  @State private var fontFamily = CandidateFontPreference.defaultFamily
  @State private var englishFamily: String?
  @State private var fontFamilies: [String] = []
  @State private var shuangpinRaw = true
  @State private var saveFailed = false
  @State private var cloudRecipient = "Google input-tools（inputtools.google.com）"
  /// A docked iPad keyboard has room for larger candidates than a phone strip; the keyboard applies the same limits when it draws.
  private let tablet = UIDevice.current.userInterfaceIdiom == .pad

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        DesignGroup(title: "候选栏") {
          DesignSliderRow(title: "候选字号", value: fontSizeSlider, range: sliderRange, step: 1,
                          identifier: "candidateFontSize") { "\(Int($0))px" }
          DesignDivider()
          // 与 Android、鸿蒙同一范围和说法（#10137）：候选栏和空闲时占同一行的工具栏一起缩放，按键不动。
          DesignSliderRow(title: "候选栏大小", info: "候选字和工具栏一起缩放，按键大小不变", value: scaleSlider,
                          range: Double(CandidateFontPreference.scaleRange.lowerBound)...Double(CandidateFontPreference.scaleRange.upperBound),
                          step: Double(CandidateFontPreference.scaleStep), identifier: "candidateScale") { "\(Int($0))%" }
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
                     in: CandidatePageSizePreference.range, identifier: "candidatePageSize",
                     info: "编号的候选个数，其余的展开候选面板查看")
        }

        DesignGroup(title: "预编辑") {
          DesignToggleRow(title: "双拼显示原始按键", info: "关闭后显示按键对应的完整拼音",
                          isOn: storedTop("shuangpin_preedit_uses_raw", $shuangpinRaw))
            .accessibilityIdentifier("shuangpinPreeditUsesRaw")
        }

        if MSIMEAppEdition.offers("quanpin") {
          DesignGroup(title: "连续多语言输入", info: "至少选择两种可用语言。日文使用已有词典，未安装或准备中时暂不参与。") {
            DesignToggleRow(title: "连续多语言输入（实验）", info: "开启候选混输后，在全拼 26 键中连续输入所选语言的编码；完整中文优先，混合结果作为备选",
                            isOn: stored("multilingual_input", "enabled", $multilingualEnabled))
              .accessibilityIdentifier("multilingualInputEnabled")
            ForEach([("chinese", "中文"), ("english", "英文"), ("japanese", "日文")], id: \.0) { language, title in
              DesignDivider()
              DesignToggleRow(title: "连续输入：\(title)", isOn: multilingualLanguage(language))
                .accessibilityIdentifier("multilingualInput.\(language)")
                .disabled(!multilingualEnabled)
            }
          }
        }

        DesignGroup(title: "候选内容", footer: saveFailed ? SettingsSaveFailure.message : nil) {
          // 副标题就是隐私说明：只有打开这个开关，输入的编码才会离开设备。
          DesignToggleRow(title: "云候选", info: "仅将当前拼写发送给 \(cloudRecipient)，不发送已上屏内容、词库或历史；需允许完全访问",
                          isOn: $cloudCandidates)
            .accessibilityIdentifier("cloudCandidates")
          DesignDivider()
          NavigationLink {
            CloudCandidateServiceSettingsView()
          } label: {
            DesignNavRowLabel(title: "云候选服务", value: cloudRecipient)
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("cloudCandidateService")
          DesignDivider()
          DesignToggleRow(title: "英文单词提示", info: "英文输入时提示常用单词，点选补全", isOn: $englishSuggestions)
            .accessibilityIdentifier("englishSuggestions")
          DesignDivider()
          DesignToggleRow(title: "候选混输", info: "关闭后保留各项设置，再开启时恢复",
                          isOn: stored("mixed_input", "enabled", $mixedEnabled))
            .accessibilityIdentifier("mixedInputEnabled")
          DesignDivider()
          DesignToggleRow(title: "中英混输", info: "中文输入时在候选中补充英文单词",
                          isOn: stored("mixed_input", "english", $english))
            .accessibilityIdentifier("mixedEnglish")
            .disabled(!mixedEnabled)
          DesignDivider()
          stepperRow("触发字母数", value: "\(minimumPrefix)", binding: stored("mixed_input", "minimum_prefix", $minimumPrefix),
                     in: 1...8, identifier: "mixedEnglishMinimumPrefix", info: "达到此长度后补全英文；26 键中更短的完整词仍可选")
          .disabled(!mixedEnabled || !english)
          .opacity(mixedEnabled && english ? 1 : 0.45)
          DesignDivider()
          stepperRow("首页英文候选数", value: "\(firstPageCount)",
                     binding: stored("mixed_input", "first_page_count", $firstPageCount),
                     in: 1...3, identifier: "mixedEnglishFirstPageCount", info: "首页有空间时优先排列的英文候选数")
          .disabled(!mixedEnabled || !english)
          .opacity(mixedEnabled && english ? 1 : 0.45)
          DesignDivider()
          DesignToggleRow(title: "符号混输", info: "打出符号的名称时在候选中加入对应符号，如「正负」后面是 ±", isOn: stored("mixed_input", "symbols", $symbols))
            .accessibilityIdentifier("mixedSymbols")
            .disabled(!mixedEnabled)
            .opacity(mixedEnabled ? 1 : 0.45)
          DesignDivider()
          DesignToggleRow(title: "emoji 混输", info: "在候选中加入匹配的 emoji，紧跟在它描绘的那个词后面", isOn: stored("mixed_input", "emoji", $emoji))
            .accessibilityIdentifier("mixedEmoji")
            .disabled(!mixedEnabled)
            .opacity(mixedEnabled ? 1 : 0.45)
          DesignDivider()
          DesignToggleRow(title: "颜文字混输", info: "在候选中加入匹配的颜文字，排在同一个词的 emoji 之后", isOn: stored("mixed_input", "kaomoji", $kaomoji))
            .accessibilityIdentifier("mixedKaomoji")
            .disabled(!mixedEnabled)
            .opacity(mixedEnabled ? 1 : 0.45)
          DesignDivider()
          DesignToggleRow(title: "以词定字", info: "长按词语候选，只上屏首字或末字",
                          isOn: stored("word_character", "enabled", $wordCharacter))
            .accessibilityIdentifier("wordCharacter")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 32)
    }
    .designCanvasBackground()
    .navigationTitle("候选栏").navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive(reload)
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

  /// 「候选栏大小」滑块：拖动时每跨过一个步长写一次文档；回到 100% 时删掉这一项，与共享设置页和 Android 写缺省值的方式相同。
  private var scaleSlider: Binding<Double> {
    Binding(get: { Double(scalePercent) }, set: { value in
      let step = CandidateFontPreference.scaleStep
      let rounded = Int((value / Double(step)).rounded()) * step
      guard rounded != scalePercent, CandidateFontPreference.scaleRange.contains(rounded) else { return }
      scalePercent = rounded
      write {
        if rounded == CandidateFontPreference.defaultScalePercent {
          $0.removeValue(forKey: CandidateFontPreference.scaleKey)
        } else {
          $0[CandidateFontPreference.scaleKey] = rounded
        }
      }
    })
  }

  /// 带标题、说明「?」、当前值和步进器的一行，布局同 DesignKit 的行。
  private func stepperRow<Value: Strideable>(
    _ title: String, value: String, binding: Binding<Value>, in range: ClosedRange<Value>, identifier: String,
    info: String
  ) -> some View {
    stepperRow(title, value: value, binding: binding, in: range, identifier: identifier, info: info) { EmptyView() }
  }

  /// 同上，标题下面另画一段内容（例如编码字号的预览）；`info` 仍收进「?」。
  private func stepperRow<Value: Strideable, Detail: View>(
    _ title: String, value: String, binding: Binding<Value>, in range: ClosedRange<Value>, identifier: String,
    info: String? = nil, @ViewBuilder detail: () -> Detail
  ) -> some View {
    HStack(spacing: 12) {
      VStack(alignment: .leading, spacing: 2) {
        DesignInfoTitle(title: title, info: info)
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
    SharedPreferenceBinding.nested(object, key: key, state: state) { saved in
      saveFailed = !saved
      if !saved { reload() }
    }
  }

  /// A family name, marked when this device does not have it and the keyboard therefore skips it.
  private func familyTitle(_ family: String) -> String {
    CandidateFontPreference.isInstalled(family) ? family : "\(family)（未安装）"
  }

  /// 修改一个语言时保留其它语言和未来字段；显式空列表不回退到默认值。
  private func multilingualLanguage(_ language: String) -> Binding<Bool> {
    Binding(get: { multilingualLanguages.contains(language) }, set: { selected in
      write {
        var multilingual = $0["multilingual_input"] as? [String: Any] ?? [:]
        var languages = multilingual["languages"] as? [String] ?? ["chinese", "english", "japanese"]
        languages.removeAll { $0 == language }
        if selected { languages.append(language) }
        multilingual["languages"] = languages
        $0["multilingual_input"] = multilingual
      }
    })
  }

  private func write(_ mutate: (inout [String: Any]) -> Void) {
    saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences(mutate)
    reload()
  }

  /// A binding that writes one top-level field of the shared document; a refused write puts the stored value back.
  private func storedTop<Value>(_ key: String, _ state: Binding<Value>) -> Binding<Value> {
    SharedPreferenceBinding.topLevel(key, state: state) { saved in
      saveFailed = !saved
      if !saved { reload() }
    }
  }

  private func reload() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    let provider = preferences["cloud_candidate_provider"] as? [String: Any] ?? [:]
    if provider["source"] as? String == "custom", let endpoint = provider["endpoint"] as? String {
      cloudRecipient = URL(string: endpoint.replacingOccurrences(of: "{query}", with: "query"))?.host ?? "所选自定义服务"
    } else { cloudRecipient = "Google input-tools（inputtools.google.com）" }
    let multilingual = preferences["multilingual_input"] as? [String: Any] ?? [:]
    multilingualEnabled = multilingual["enabled"] as? Bool ?? false
    multilingualLanguages = multilingual["languages"] as? [String] ?? ["chinese", "english", "japanese"]
    let mixed = preferences["mixed_input"] as? [String: Any] ?? [:]
    mixedEnabled = mixed["enabled"] as? Bool ?? true
    firstPageCount = mixed["first_page_count"] as? Int ?? 1
    english = mixed["english"] as? Bool ?? english
    minimumPrefix = Self.minimumPrefix(mixed["minimum_prefix"]) ?? minimumPrefix
    emoji = mixed["emoji"] as? Bool ?? emoji
    kaomoji = mixed["kaomoji"] as? Bool ?? kaomoji
    symbols = Self.symbolsEnabled(mixed)
    candidateSize = CandidateFontPreference.candidateSize(in: preferences, tablet: tablet)
    preeditSize = CandidateFontPreference.preeditSize(in: preferences, tablet: tablet)
    scalePercent = CandidateFontPreference.scalePercent(in: preferences)
    fontFamily = preferences[CandidateFontPreference.familyKey] as? String ?? CandidateFontPreference.defaultFamily
    englishFamily = (preferences[CandidateFontPreference.englishFamilyKey] as? String).flatMap { $0.isEmpty ? nil : $0 }
    fontFamilies = CandidateFontPreference.families(in: preferences)
    shuangpinRaw = preferences["shuangpin_preedit_uses_raw"] as? Bool ?? true
    wordCharacter = (preferences["word_character"] as? [String: Any])?["enabled"] as? Bool ?? wordCharacter
  }

  /// 符号混输（`mixed_input.symbols`）默认开，共享文档只在关掉时才写这个键，所以缺键读作开，不沿用页面上的旧值。
  static func symbolsEnabled(_ mixed: [String: Any]) -> Bool {
    mixed["symbols"] as? Bool ?? true
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


/// 所有服务字段一次交给共享偏好保存，避免编辑尚未完成时覆盖有效配置。
private struct CloudCandidateServiceSettingsView: View {
  @State private var source = "google"
  @State private var endpoint = ""
  @State private var method = "GET"
  @State private var headers = "{}"
  @State private var bodyTemplate = ""
  @State private var responseFormat = "candidates"
  @State private var responsePath = ""
  @State private var timeout = ""
  @State private var cacheEnabled = true
  @State private var saveFailed = false
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    Form {
      Section {
        Picker("云候选来源", selection: $source) {
          Text("Google input-tools").tag("google")
          Text("自定义 HTTP 接口").tag("custom")
        }
        .accessibilityIdentifier("cloudCandidateSource")
      } footer: {
        Text("只发送当前正在组的拼写，不发送已上屏内容、词库或历史。配置和凭据只保存在本机。")
      }
      if source == "custom" {
        Section("HTTP 请求") {
          TextField("接口地址（GET 用 {query}）", text: $endpoint)
            .accessibilityIdentifier("cloudCandidateEndpoint")
          Picker("请求方法", selection: $method) {
            Text("GET").tag("GET"); Text("POST").tag("POST")
          }
          Text("请求头（JSON，例如 Authorization）").font(.caption)
          TextEditor(text: $headers).frame(minHeight: 80)
            .accessibilityIdentifier("cloudCandidateHeaders")
          if method == "POST" {
            Text("请求体模板（JSON，字符串中的 {query} 自动转义替换）").font(.caption)
            TextEditor(text: $bodyTemplate).frame(minHeight: 100)
              .accessibilityIdentifier("cloudCandidateBodyTemplate")
          }
        }
        Section("响应") {
          Picker("响应格式", selection: $responseFormat) {
            Text("标准 candidates 数组").tag("candidates")
            Text("Google input-tools").tag("google")
            Text("JSON 路径").tag("json_path")
          }
          if responseFormat == "json_path" {
            TextField("JSON 路径，例如 data.0", text: $responsePath)
              .accessibilityIdentifier("cloudCandidateResponsePath")
          }
        }
      }
      Section {
        TextField("超时（毫秒）", text: $timeout).keyboardType(.numberPad)
          .accessibilityIdentifier("cloudCandidateTimeout")
        Toggle("缓存云候选", isOn: $cacheEnabled)
      } footer: {
        Text("超时范围由共享层校验（100–10000 毫秒）。缓存仅留在本机内存，各来源独立；无结果缓存 60 秒。公网地址使用 HTTPS，本机服务可用 HTTP。")
      }
      if saveFailed { Text("保存失败，请检查接口地址、请求头 JSON、请求体模板、响应路径和超时。").foregroundStyle(.red) }
    }
    .textInputAutocapitalization(.never).autocorrectionDisabled()
    .navigationTitle("云候选服务").navigationBarTitleDisplayMode(.inline)
    .toolbar {
      ToolbarItem(placement: .topBarTrailing) { Button("保存", action: save) }
    }
    .onAppear(perform: load)
  }

  private func load() {
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences(),
          let provider = preferences["cloud_candidate_provider"] as? [String: Any] else { return }
    source = provider["source"] as? String ?? source
    endpoint = provider["endpoint"] as? String ?? endpoint
    method = provider["method"] as? String ?? method
    headers = provider["headers_json"] as? String ?? headers
    bodyTemplate = provider["body_template"] as? String ?? bodyTemplate
    responseFormat = provider["response_format"] as? String ?? responseFormat
    responsePath = provider["response_path"] as? String ?? responsePath
    timeout = (provider["timeout_ms"] as? NSNumber)?.stringValue ?? timeout
    cacheEnabled = provider["cache_enabled"] as? Bool ?? cacheEnabled
  }

  private func save() {
    guard let milliseconds = Int(timeout) else { saveFailed = true; return }
    let provider: [String: Any] = ["source": source, "endpoint": endpoint, "method": method,
      "headers_json": headers, "body_template": bodyTemplate, "response_format": responseFormat,
      "response_path": responsePath, "timeout_ms": milliseconds, "cache_enabled": cacheEnabled]
    saveFailed = !MetasequoiaInputSessionBridge.updateSharedPreferences { $0["cloud_candidate_provider"] = provider }
    if !saveFailed { dismiss() }
  }
}

import SwiftUI

/// 词库页：已安装的词库、导入入口、可添加的社区词库、学习开关，以及进入「背单词」和「词库信息」的链接。「学习」保存在共享偏好文档里（见 InputHabitPreference），键盘下次出现时重新读取；手机和 iPad 显示同一个页面。
struct DictionarySettingsView: View {
  /// 设计稿最多列出这么多社区词库；完整列表在「社区」标签页。
  private static let discoverLimit = 8

  @Environment(\.scenePhase) private var scenePhase
  @State private var habits = InputHabitPreference.mirrored
  @State private var saveFailed = false
  @State private var dictionary: PersonalDictionaryState?
  @State private var importing = false
  @State private var discover: [CommunityResource]?
  @State private var discoverFailed = false
  /// 本机排队导入过的社区词库（`CommunityDictionaryImports`），胶囊按钮据此显示「已添加」；服务器上的收藏只表示关注更新，不代表本机装过。
  @State private var added = CommunityDictionaryImports.ids()
  @State private var adding: Set<String> = []
  @State private var confirming: CommunityResource?
  @State private var message: String?
  private let store = PersonalDictionaryStore.forSettingsPages()

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 28) {
        DesignGroup(title: "已安装", footer: "个人词库收着你添加的和输入时学到的词条，拼音、五笔、快捷短语和英文都在里面。点进去可以查看、搜索、添加和编辑，滑动一条可删除。") {
          NavigationLink(destination: PersonalDictionaryView()) {
            DictionaryPackRowLabel(initial: "个", title: "个人词库", meta: wordCountLabel)
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("personalDictionaryLink")
        }
        DesignCard {
          Button { importing = true } label: {
            DictionaryActionRowLabel(symbol: "square.and.arrow.up", title: "导入词库")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("importDictionaryAction")
        }
        DesignGroup(title: "发现词库") { discoverRows }
        DesignGroup(title: "学习", footer: learningFooter) { learningRows }
        DesignGroup(title: "更多") {
          NavigationLink(destination: VocabularyReviewSettingsView()) {
            DesignNavRowLabel(title: "背单词")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("vocabularyReviewSettingsLink")
          DesignDivider(leading: 20)
          NavigationLink(destination: DictionaryInfoView()) {
            DesignNavRowLabel(title: "词库信息")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("dictionaryInfoLink")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 12)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("词库")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .onChange(of: scenePhase) { _, phase in if phase == .active { reload() } }
    .task { await loadDiscover() }
    .refreshable { await loadDiscover() }
    .sheet(isPresented: $importing, onDismiss: reload) {
      PersonalDictionaryImportView { words in
        try store.enqueueImport(words)
        ToastCenter.shared.show("已加入导入队列")
      }
    }
    .alert("导入这版词库到本机？", isPresented: Binding(get: { confirming != nil }, set: { if !$0 { confirming = nil } }),
           presenting: confirming) { item in
      Button("取消", role: .cancel) {}
      Button("导入") { add(item) }
    } message: { item in
      Text("「\(item.name)」的 \(item.content.entries?.count ?? 0) 条词条会加入本机个人词库，开启完全访问后由水杉键盘在空闲时导入。")
    }
    .alert("词库", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }

  // MARK: - 已安装

  /// 用户自己的词条数，取自键盘最近一次列出的那一页。App 自己从不打开 Engine 词库（只有键盘可以），所以这个数就是那份快照显示的数量，而且只在快照是整份列表的第一页时才算：同时也是最后一页时是准确值，后面还有页时是下限，快照是搜索结果或后面的页时则未知。用户改过权重的内置词条会和用户词条一起列出，但不是用户自己的，所以不计入，与「统计」徽标的 `TypingSummary.userWords` 一样把它们排除在外；不同于只数拼音的那个徽标，这里各种类型都计入，因为这一行会打开全部类型。
  private var wordCountLabel: String? {
    guard let dictionary else { return nil }
    guard dictionary.snapshotDate != nil else { return "打开键盘后显示自造词数量" }
    guard dictionary.pageKind == nil, dictionary.pageQuery.isEmpty, dictionary.pageOffset == 0 else { return nil }
    let own = dictionary.entries.filter { !$0.isBundled }.count
    return dictionary.hasMore ? "\(own)+ 条自造词" : "\(own) 条自造词"
  }

  // MARK: - 发现词库

  @ViewBuilder private var discoverRows: some View {
    if let discover {
      if discover.isEmpty {
        DictionaryNoteRow(text: "社区里还没有词库。")
      } else {
        ForEach(Array(discover.enumerated()), id: \.element.id) { index, item in
          if index > 0 { DesignDivider(leading: 0) }
          discoverRow(item)
        }
      }
    } else {
      DictionaryNoteRow(text: discoverFailed ? "暂时无法加载社区词库" : "正在读取社区词库…")
    }
  }

  private func discoverRow(_ item: CommunityResource) -> some View {
    let isAdded = added.contains(item.id)
    let isAdding = adding.contains(item.id)
    var meta = item.author.isEmpty ? [] : ["@\(item.author)"]
    if let count = item.content.entries?.count { meta.append("\(count) 条") }
    return HStack(spacing: 12) {
      Text(item.name.first.map(String.init) ?? "词")
        .font(.system(size: 15, weight: .bold))
        .foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 36, height: 36)
        .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 2) {
        Text(item.name).font(.system(size: 15, weight: .semibold)).foregroundStyle(.primary).lineLimit(1)
        if !meta.isEmpty {
          Text(meta.joined(separator: " · ")).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
        }
      }
      Spacer(minLength: 8)
      Button { confirming = item } label: {
        Text(isAdded ? "已添加" : isAdding ? "添加中" : "添加")
          .font(.system(size: 13, weight: .semibold))
          .foregroundStyle(isAdded ? MetasequoiaTheme.sub : MetasequoiaTheme.accent)
          .padding(.vertical, 5)
          .padding(.horizontal, 14)
          .background(isAdded ? MetasequoiaTheme.segBg : MetasequoiaTheme.accentSoft, in: Capsule())
          .contentShape(Capsule())
      }
      .buttonStyle(.plain)
      .disabled(isAdded || isAdding)
      .accessibilityLabel("\(isAdded ? "已添加" : isAdding ? "添加中" : "添加")「\(item.name)」")
      .accessibilityIdentifier("discoverDictionary-\(item.id)")
    }
    .padding(.vertical, 12)
    .padding(.horizontal, 16)
    .frame(maxWidth: .infinity, alignment: .leading)
  }

  @MainActor private func loadDiscover() async {
    do {
      let page = try await SkinCommunityAPI.shared.resources(.dictionary)
      discover = Array(page.items.prefix(Self.discoverLimit))
      discoverFailed = false
    } catch is CancellationError {
      return
    } catch {
      // 已经加载过的列表保留下来；刷新失败只替换加载提示。
      if discover == nil { discoverFailed = true }
    }
  }

  /// 与 CommunityResourceDetail 提供的导入相同：已登录时把词库标记为已收藏，让它出现在「收藏」里并跟进更新，然后把词条排进键盘的导入队列。「已添加」表示已排队、尚未生效；键盘稍后才导入。
  private func add(_ item: CommunityResource) {
    guard !adding.contains(item.id) else { return }
    adding.insert(item.id)
    Task { @MainActor in
      defer { adding.remove(item.id) }
      do {
        var latest = item
        if try await SkinCommunityAPI.shared.signedIn() {
          try await SkinCommunityAPI.shared.saveResource(item.id, saved: true)
          latest = try await SkinCommunityAPI.shared.resource(item.id)
        }
        try store.enqueueImport((latest.content.entries ?? []).map { try $0.localWord() })
        CommunityDictionaryImports.record(item.id)
        added.insert(item.id)
        if let index = discover?.firstIndex(where: { $0.id == item.id }) { discover?[index] = latest }
        ToastCenter.shared.show("已添加「\(item.name)」")
        reload()
      } catch {
        message = error.localizedDescription
      }
    }
  }

  // MARK: - 学习

  @ViewBuilder private var learningRows: some View {
    DesignToggleRow(title: "学习常用词", isOn: habit(\.learning))
      .accessibilityIdentifier("dictionaryLearningToggle")
    DesignDivider(leading: 20)
    DesignSelectRow(title: "调频方式", options: FrequencyAdjustmentMode.allCases.map { DesignOption(title: $0.title, value: $0) },
                    selection: habit(\.frequencyMode), sheetTitle: "调频方式", identifier: "frequencyAdjustmentModePicker")
      .disabled(!habits.learning)
      .opacity(habits.learning ? 1 : 0.45)
    DesignDivider(leading: 20)
    let triggerEnabled = habits.learning && habits.frequencyMode != .disabled
    DesignSelectRow(title: "触发频次", options: Self.countOptions, selection: habit(\.triggerCount),
                    sheetTitle: "触发频次", identifier: "frequencyAdjustmentTriggerPicker")
      .disabled(!triggerEnabled)
      .opacity(triggerEnabled ? 1 : 0.45)
    DesignDivider(leading: 20)
    let stepEnabled = habits.learning && habits.frequencyMode == .linear
    DesignSelectRow(title: "线性调频步长", options: Self.countOptions, selection: habit(\.linearStep),
                    sheetTitle: "线性调频步长", identifier: "frequencyAdjustmentLinearStepPicker")
      .disabled(!stepEnabled)
      .opacity(stepEnabled ? 1 : 0.45)
    if saveFailed {
      DesignDivider(leading: 20)
      Text("设置没有保存，键盘可能正在写入同一份设置，请再试一次。")
        .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.danger)
        .fixedSize(horizontal: false, vertical: true)
        .padding(.vertical, 12)
        .padding(.horizontal, 20)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  private static let countOptions = FrequencyAdjustmentPreference.countRange.map { DesignOption(title: "\($0)", value: $0) }

  private var learningFooter: String {
    "开启后，引擎按所选调频方式调整候选排序，并学习支持的拼音组词。不调频保留词库原有顺序，只学习新词；一次置顶移到首位；折半移到当前名次与首位之间；线性按固定步数前移；一次置前把前五名前进一位、更靠后的提到第五名。触发频次是同一候选累计选中多少次后才调整一次。学习记录仅保存在设备上。关闭后停止新增学习，不清除已有记录；正在输入的内容结束后生效。"
  }

  private func reload() {
    habits = InputHabitPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
    // 队列文件读不出来只是在这里不显示数量；错误由「拼音词库」页报告。
    dictionary = try? store.read()
    added = CommunityDictionaryImports.ids()
  }

  /// 只保存一个字段的 binding；保存失败时重新加载，让控件显示实际存下的值。
  private func habit<Value>(_ field: WritableKeyPath<InputHabitSettings, Value>) -> Binding<Value> {
    Binding(get: { habits[keyPath: field] }, set: { value in
      if let saved = InputHabitPreference.update({ $0[keyPath: field] = value }) {
        habits = saved
        saveFailed = false
      } else {
        saveFailed = true
        reload()
      }
    })
  }
}

// MARK: - 行

/// 已安装的词库行：30pt 着色图块显示首字，然后是名称和大小，以及右箭头。高 60pt，与设计稿的词库包行一致。iOS 只有一个由用户管理的词库，且无法关闭，所以这一行不带「已启用」状态。
private struct DictionaryPackRowLabel: View {
  let initial: String
  let title: String
  let meta: String?

  var body: some View {
    HStack(spacing: 12) {
      Text(initial)
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 30, height: 30)
        .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 1) {
        Text(title).font(.system(size: 17)).foregroundStyle(.primary)
        if let meta {
          Text(meta).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
        }
      }
      Spacer(minLength: 8)
      Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold))
        .foregroundStyle(MetasequoiaTheme.sub.opacity(0.6))
        .accessibilityHidden(true)
    }
    .padding(.leading, 16)
    .padding(.trailing, 12)
    .padding(.vertical, 6)
    .frame(maxWidth: .infinity, minHeight: 60, alignment: .leading)
    .contentShape(Rectangle())
  }
}

/// 强调色的操作行：30pt 图标列和 17pt 标题，高 52pt。
private struct DictionaryActionRowLabel: View {
  let symbol: String
  let title: String

  var body: some View {
    HStack(spacing: 12) {
      Image(systemName: symbol).font(.system(size: 18, weight: .medium))
        .frame(width: 30)
        .accessibilityHidden(true)
      Text(title).font(.system(size: 17))
      Spacer(minLength: 0)
    }
    .foregroundStyle(MetasequoiaTheme.accent)
    .padding(.horizontal, 16)
    .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
    .contentShape(Rectangle())
  }
}

/// 卡片里占据行位置的单行状态，例如加载中或社区无法访问。
private struct DictionaryNoteRow: View {
  let text: String

  var body: some View {
    Text(text).font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
      .fixedSize(horizontal: false, vertical: true)
      .padding(.vertical, 16)
      .padding(.horizontal, 16)
      .frame(maxWidth: .infinity, alignment: .leading)
  }
}

// MARK: - 词库信息

/// App 自带的内容：内置方案、清单里的 profile 和版本，以及候选的管理方式。
private struct DictionaryInfoView: View {
  private var manifest: [String: Any] {
    guard let url = Bundle.main.url(forResource: "msime-dictionary-manifest", withExtension: "json"),
          let data = try? Data(contentsOf: url),
          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
    return object
  }

  var body: some View {
    let manifest = manifest
    ScrollView {
      VStack(alignment: .leading, spacing: 28) {
        DesignGroup(title: "已安装词库") {
          DictionaryInfoText("内置离线多方案词库", detail: "支持全拼 26 键、全拼 9 键、小鹤／自然码／微软／首道双拼、86／98 五笔、日语罗马字、韩语两套式、粤拼、大千注音（繁体输出）、越南语 Telex／VNI、藏文威利转写（EWTS）和笔画（横竖撇点折五键加通配）；粤拼、注音和笔画需要安装包里带有对应的语言词库，默认不启用，可在方案设置里打开；提供英文补全、快捷短语、表情及颜文字。")
          DesignDivider(leading: 20)
          DesignValueRow(title: "更新方式", value: "随 App 更新")
        }
        DesignGroup(title: "词库信息", footer: "词库保存在设备上，日常输入不需要联网。已启用日语整句转换，支持罗马字输入、假名及汉字混合候选。") {
          let profile = manifest["profile"] as? String
          let commit = (manifest["source"] as? [String: Any])?["commit"] as? String
          if let profile {
            DesignValueRow(title: "规格", value: profile)
          }
          if let commit {
            if profile != nil { DesignDivider(leading: 20) }
            VStack(alignment: .leading, spacing: 4) {
              Text("词库版本").font(.system(size: 17))
              Text(String(commit.prefix(12))).font(.system(size: 13, design: .monospaced))
                .foregroundStyle(MetasequoiaTheme.sub)
                .textSelection(.enabled)
            }
            .padding(.vertical, 10)
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
            .accessibilityElement(children: .combine)
          }
          if profile == nil && commit == nil {
            DictionaryInfoText(nil, detail: "安装包里没有词库清单。")
          }
        }
        DesignGroup(title: "候选词管理") {
          DictionaryInfoText("长按候选词", detail: "全拼 26 键、九键、双拼和五笔支持长按候选词：优先显示、固定到前五位中的某一位、取消固定或删除词条。删除需要再次确认，单个汉字由引擎保护。")
          DesignDivider(leading: 20)
          DictionaryInfoText(nil, detail: "日语、韩语、粤拼、注音、越南语、藏文、笔画和本地工具暂不支持候选词管理。第三方词库文件（词在前、编码在前或 Rime 格式）在词库页的「导入词库」里导入。")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 12)
      .padding(.bottom, 32)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("词库信息")
    .navigationBarTitleDisplayMode(.inline)
  }
}

/// 「词库信息」里的说明行：可选的 17pt 标题，下面是 13pt 说明文字。
private struct DictionaryInfoText: View {
  let title: String?
  let detail: String

  init(_ title: String?, detail: String) {
    self.title = title
    self.detail = detail
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      if let title { Text(title).font(.system(size: 17)).foregroundStyle(.primary) }
      Text(detail).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
        .fixedSize(horizontal: false, vertical: true)
    }
    .padding(.vertical, 12)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, alignment: .leading)
    .accessibilityElement(children: .combine)
  }
}

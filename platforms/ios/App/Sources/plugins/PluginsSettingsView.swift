import SwiftUI
import UniformTypeIdentifiers

/// 设置里的「插件」页，照共享设置页插件页的结构画成原生 SwiftUI：标题下固定「我的插件 / 社区插件」两段。
///
/// 「我的插件」按类型分组列出这台设备用得上的包（`PluginKind.phoneOrder`），指令表和短语表在行上直接启用，点进详情启用、选用、编辑或删除；右上角的「排序」进入排序状态，在一组里拖动调整先后（`plugins.plugin_order`，只影响列表）；下面是「制作插件」（新建短语表、指令表、符号集或单词本）和「导入插件」（系统文件选择器选 `.zip`，手机上不导入文件夹）。特效包、音乐包和旋律音效包装着也不列，只用一行说明有几个。包装在 App Group 的 `<共享状态目录>/plugins`，键盘下次获得焦点时读到。
struct PluginsSettingsView: View {
  enum Segment: Hashable, CaseIterable {
    case mine, community

    var title: String {
      switch self {
      case .mine: "我的插件"
      case .community: "社区插件"
      }
    }
  }

  @StateObject private var library = PluginLibrary()
  @State private var segment: Segment
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  /// `searchAnchor` 是设置搜索命中的关键词（`SettingsSearchAnchor`）：它是某一段的标题时从那一段开始，例如搜「社区插件」直接停在「社区插件」；其余情况从「我的插件」开始。
  init(searchAnchor: String? = nil) {
    let title = SettingsSearchAnchor.section(Segment.allCases.map(\.title), anchor: searchAnchor)
    _segment = State(initialValue: Segment.allCases.first { $0.title == title } ?? .mine)
  }

  var body: some View {
    Group {
      switch segment {
      case .mine: PluginListView(library: library)
      case .community: CommunityPluginsView(library: library)
      }
    }
    .designPinnedSegments {
      DesignSegmentedControl(items: Segment.allCases.map { ($0.title, $0) },
                             selection: $segment, style: .rounded, identifierPrefix: "pluginsTab")
        .frame(maxWidth: horizontalSizeClass == .regular ? CommunityLayout.regularMaxWidth : .infinity)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 16).padding(.vertical, 8)
    }
    .designCanvasBackground()
    .navigationTitle("插件")
    .navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive { library.reload() }
  }
}

/// 列表推入的页面。
private enum PluginRoute: Hashable, Identifiable {
  case pack(String)
  case missing(String)
  var id: Self { self }
}

/// 「我的插件」：分组列表、制作和导入。
struct PluginListView: View {
  @ObservedObject var library: PluginLibrary
  @State private var route: PluginRoute?
  @State private var creating = false
  /// 刚在编辑器里保存的包，编辑器收起后打开它的详情。
  @State private var savedKey: String?
  @State private var importing = false
  @State private var message: String?
  /// 「排序」状态：行尾换成拖动手柄，行不再推入详情，也不显示开关。
  @State private var editMode = EditMode.inactive

  /// 有哪一组装着两个以上的包时才有东西可排。
  private var reorderable: Bool {
    library.state == .loaded && library.catalog.groups(missing: []).contains { $0.packs.count > 1 }
  }

  var body: some View {
    Form {
      let missing = library.missing
      switch library.state {
      case .loading:
        Section { Text("正在读取插件…").foregroundStyle(.secondary) }.designRow()
      case .failed:
        Section { Text("插件列表没有读取成功，重新打开这个页面会再试一次。").foregroundStyle(.secondary) }.designRow()
      case .loaded:
        let groups = library.catalog.groups(missing: missing, order: library.preferences.pluginOrder)
        ForEach(groups) { group in
          Section {
            // 拖动用系统 List 的拖拽，只在排序状态下打开；一组的 ForEach 各管各的，包不会被拖到别的类型里。
            ForEach(group.packs) { pack in packRow(pack) }
              .onMove(perform: editMode.isEditing ? { source, destination in
                if !library.movePacks(in: group, from: source, to: destination) { message = SettingsSaveFailure.message }
              } : nil)
            ForEach(group.missing) { entry in missingRow(entry) }
            if group.kind == .commandTable && group.packs.isEmpty && group.missing.isEmpty {
              Text("还没有导入指令表。").foregroundStyle(.secondary)
            }
          } header: {
            DesignSectionHeader(title: group.kind.label, info: group.kind == .commandTable
              ? "在「输入 → 快捷模式」打开 / 指令后，按 / 再输入指令字母即可使用。启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用 \(PluginPreferences.maximumTables) 个。"
              : nil)
          }
          .designRow()
        }
        if library.catalog.unusedCount > 0 {
          Section {
            Text("本机另外装着 \(library.catalog.unusedCount) 个这台设备用不上的插件（特效包、音乐包或按键旋律），这里不列出。")
              .font(.footnote).foregroundStyle(.secondary)
              .accessibilityIdentifier("pluginsUnusedCount")
          }
          .designRow()
        }
      }
      Section {
        Button { creating = true } label: {
          SettingsOptionLabel(title: "新建插件", info: PluginEditorModel.createDescription)
        }
        .disabled(library.working)
        .accessibilityIdentifier("pluginsCreate")
      } header: {
        Text("制作插件")
      }
      .designRow()
      Section {
        Button { importing = true } label: {
          SettingsOptionLabel(title: "导入 .zip", info: "只含音频、指令模板等数据，不含可执行内容，每个包须声明许可证")
        }
        .disabled(library.working)
        .accessibilityIdentifier("pluginsImportArchive")
        if library.working { ProgressView().frame(maxWidth: .infinity) }
      } header: {
        Text("导入插件")
      } footer: {
        if !library.notice.isEmpty { Text(library.notice).accessibilityIdentifier("pluginsNotice") }
      }
      .designRow()
      if !library.catalog.issues.isEmpty {
        Section("无法载入的插件") {
          ForEach(Array(library.catalog.issues.enumerated()), id: \.offset) { _, issue in
            Text("\(issue.kindLabel) \(issue.folder.isEmpty ? "目录" : issue.folder) 无法载入：\(issue.reason)")
              .font(.footnote).foregroundStyle(.secondary)
          }
        }
        .designRow()
      }
    }
    .designPage()
    .environment(\.editMode, $editMode)
    .toolbar {
      if reorderable || editMode.isEditing {
        ToolbarItem(placement: .topBarTrailing) {
          Button(editMode.isEditing ? "完成" : "排序") {
            withAnimation { editMode = editMode.isEditing ? .inactive : .active }
          }
          .accessibilityIdentifier("pluginsReorder")
        }
      }
    }
    .navigationDestination(item: $route) { route in
      switch route {
      case .pack(let key): PluginDetailView(library: library, key: key)
      case .missing(let key): MissingPluginView(library: library, key: key)
      }
    }
    // 保存后先关掉编辑器，等它真的收起再推入详情：弹出页还在收起时推入的页面会被丢掉。
    .sheet(isPresented: $creating, onDismiss: {
      if let savedKey { route = .pack(savedKey) }
      savedKey = nil
    }) {
      PluginEditorView(library: library, target: nil) { saved in
        savedKey = saved.key
        creating = false
      }
    }
    .fileImporter(isPresented: $importing, allowedContentTypes: [.zip]) { result in
      guard case .success(let url) = result else { return }
      Task { message = await library.importArchive(url) }
    }
    .alert("插件", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }

  private func packRow(_ pack: PluginPackage) -> some View {
    // 内置插件的版本和作者都一样，所以这一行只留能把它们区分开的内容；详情页仍然显示全部信息。
    let description = [pack.builtin ? nil : pack.version, !pack.builtin ? pack.author.map { "作者 \($0)" } : nil]
      .compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
    let table = pack.kind == .commandTable || pack.kind == .phraseTable
    return Group {
      if editMode.isEditing {
        PluginRowLabel(title: pack.label, detail: description, marker: library.marker(pack), showsChevron: false)
          .accessibilityElement(children: .combine)
      } else if table {
        // 行上有两个控件：名字那一块和箭头推入详情，开关只管启用。按钮用 plain 样式，各自只响应自己那一块，点开关不会连带推入详情。
        HStack(spacing: 8) {
          Button { route = .pack(pack.key) } label: {
            PluginRowLabel(title: pack.label, detail: description, marker: library.marker(pack), showsChevron: false)
          }
          .buttonStyle(.plain)
          .accessibilityIdentifier("pluginPack.\(pack.key)")
          tableToggle(pack)
          Button { route = .pack(pack.key) } label: { DesignChevron(fontSize: 13).contentShape(Rectangle()) }
            .buttonStyle(.plain)
            .accessibilityHidden(true)
        }
      } else {
        Button { route = .pack(pack.key) } label: {
          PluginRowLabel(title: pack.label, detail: description, marker: library.marker(pack))
        }
        .accessibilityIdentifier("pluginPack.\(pack.key)")
      }
    }
  }

  /// 指令表、短语表行上的「启用」开关：与详情里的开关同一条规则、同一条写入路径，启用满了而它没启用时不能点。
  private func tableToggle(_ pack: PluginPackage) -> some View {
    let state = PluginCatalogPolicy.tableSwitch(pack, preferences: library.preferences)
    return Toggle("启用\(pack.name)", isOn: Binding(get: { state.enabled }, set: { on in
      if !library.setTable(pack, enabled: on) { message = SettingsSaveFailure.message }
    }))
    .labelsHidden()
    .disabled(state.full)
    .accessibilityLabel("启用\(pack.name)")
    .accessibilityIdentifier("pluginRowEnabled.\(pack.key)")
  }

  private func missingRow(_ entry: PluginMissingSelection) -> some View {
    Button { route = .missing(entry.key) } label: {
      PluginRowLabel(title: entry.title, detail: "\(entry.uses.joined(separator: "、"))，\(entry.reason)", marker: nil)
    }
    .accessibilityIdentifier("pluginMissing.\(entry.key)")
  }
}

/// 列表里的一行：名字、版本和作者，行尾是使用状态和箭头；行上另有开关或拖动手柄时不画箭头。
struct PluginRowLabel: View {
  let title: String
  let detail: String
  let marker: String?
  var showsChevron = true

  var body: some View {
    HStack(spacing: 8) {
      VStack(alignment: .leading, spacing: 2) {
        Text(title).foregroundStyle(.primary)
        if !detail.isEmpty { Text(detail).font(.footnote).foregroundStyle(.secondary) }
      }
      Spacer(minLength: 8)
      if let marker {
        Text(marker).font(.footnote).foregroundStyle(MetasequoiaTheme.accent).multilineTextAlignment(.trailing)
      }
      if showsChevron { DesignChevron(fontSize: 13) }
    }
    .contentShape(Rectangle())
  }
}

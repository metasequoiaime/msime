import SwiftUI
import UniformTypeIdentifiers

/// 设置里的「插件」页，照共享设置页插件页的结构画成原生 SwiftUI：标题下固定「我的插件 / 社区插件」两段。
///
/// 「我的插件」按类型分组列出这台设备用得上的包（`PluginKind.phoneOrder`），点进详情启用、选用、编辑或删除；下面是「制作插件」（新建短语表、指令表、符号集或单词本）和「导入插件」（系统文件选择器选 `.zip`，手机上不导入文件夹）。特效包、音乐包和旋律音效包装着也不列，只用一行说明有几个。包装在 App Group 的 `<共享状态目录>/plugins`，键盘下次获得焦点时读到。
struct PluginsSettingsView: View {
  enum Segment: Hashable { case mine, community }

  @StateObject private var library = PluginLibrary()
  @State private var segment = Segment.mine
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  var body: some View {
    Group {
      switch segment {
      case .mine: PluginListView(library: library)
      case .community: CommunityPluginsView(library: library)
      }
    }
    .designPinnedSegments {
      DesignSegmentedControl(items: [("我的插件", Segment.mine), ("社区插件", .community)],
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

  var body: some View {
    Form {
      let missing = library.missing
      switch library.state {
      case .loading:
        Section { Text("正在读取插件…").foregroundStyle(.secondary) }.designRow()
      case .failed:
        Section { Text("插件列表没有读取成功，重新打开这个页面会再试一次。").foregroundStyle(.secondary) }.designRow()
      case .loaded:
        let groups = library.catalog.groups(missing: missing)
        ForEach(groups) { group in
          Section {
            ForEach(group.packs) { pack in packRow(pack) }
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
    return Button { route = .pack(pack.key) } label: {
      PluginRowLabel(title: pack.label, detail: description, marker: library.marker(pack))
    }
    .accessibilityIdentifier("pluginPack.\(pack.key)")
  }

  private func missingRow(_ entry: PluginMissingSelection) -> some View {
    Button { route = .missing(entry.key) } label: {
      PluginRowLabel(title: entry.title, detail: "\(entry.uses.joined(separator: "、"))，\(entry.reason)", marker: nil)
    }
    .accessibilityIdentifier("pluginMissing.\(entry.key)")
  }
}

/// 列表里的一行：名字、版本和作者，行尾是使用状态和箭头。
struct PluginRowLabel: View {
  let title: String
  let detail: String
  let marker: String?

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
      DesignChevron(fontSize: 13)
    }
    .contentShape(Rectangle())
  }
}

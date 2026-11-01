import SwiftUI

/// 「插件」页的「社区插件」分段：按类型浏览、搜索、安装、评分、举报、下架和发布，文字与共享设置页的社区插件图库（`community-plugins.tsx`）相同。只列在手机上起作用的类型；装好的包进 App Group 的插件目录，与导入的一样出现在「我的插件」里。
struct CommunityPluginsView: View {
  @ObservedObject var library: PluginLibrary
  @State private var search = ""
  @State private var activeSearch = ""
  @State private var kind: PluginKind?
  @State private var mineOnly = false
  @State private var paging = CommunityPluginPaging()
  @State private var busy = false
  @State private var loaded = false
  /// 详情页压在列表上面时屏蔽或在别处解除屏蔽，列表已不可见、读取会被跳过，回到列表时再补读。
  @State private var blocksChanged = false
  @State private var message: String?
  @State private var notice: String?
  @State private var signedIn = false
  @State private var showAccount = false
  @State private var publishing = false
  @State private var selected: CommunityPlugin?
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 14) {
        CommunitySearchField(text: $search, placeholder: "搜索插件") { startLoad() }
        Text("指令表、短语表、辅助码表、单词本、符号集与按键音效包，安装后在「我的插件」中选用")
          .font(.footnote).foregroundStyle(MetasequoiaTheme.sub)
        HStack(spacing: 8) {
          DesignSegmentedControl(items: [("全部插件", false), ("我的作品", true)], selection: mineBinding,
                                 style: .capsule, identifierPrefix: "communityPluginScope")
          Button { if signedIn { publishing = true } else { showAccount = true } } label: {
            Text("发布我的插件").font(.subheadline.weight(.semibold))
          }
          .buttonStyle(.borderedProminent)
          .accessibilityIdentifier("publishCommunityPlugin")
        }
        kindFilter
        feedback
        if !paging.items.isEmpty {
          DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
            ForEach(Array(paging.items.enumerated()), id: \.element.id) { index, item in
              if index > 0 { DesignDivider(leading: 0) }
              row(item)
            }
          }
        }
        if busy { ProgressView("正在读取插件…").frame(maxWidth: .infinity) }
        if paging.hasMore && !busy {
          Button("加载更多") { startLoad(append: true) }.frame(maxWidth: .infinity)
            .accessibilityIdentifier("communityPluginsMore")
        }
      }
      .communityPageContent(regular: horizontalSizeClass == .regular)
    }
    .navigationDestination(item: $selected) { item in
      CommunityPluginDetail(library: library, initial: item) { changed in
        if let changed { paging.replace(changed) } else { paging.remove(item.id) }
      }
    }
    .sheet(isPresented: $publishing) {
      CommunityPluginPublishView(library: library) { published in
        publishing = false
        notice = "已发布到社区。"
        _ = published
        startLoad()
      }
    }
    .sheet(isPresented: $showAccount, onDismiss: {
      Task {
        let state = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
        guard request.isVisible else { return }
        signedIn = state
        if signedIn { publishing = true }
      }
    }) { AccountLoginSheet() }
    .refreshable { await load() }
    .onReceive(NotificationCenter.default.publisher(for: CommunityBlocks.didChange)) { _ in
      if request.isVisible { startLoad() } else { blocksChanged = true }
    }
    .trackViewRequest(request: $request, pending: $pending, onAppear: {
      if !loaded || blocksChanged {
        blocksChanged = false
        startLoad()
      }
      Task {
        let state = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
        if request.isVisible { signedIn = state }
      }
    }, onDisappear: { busy = false })
    .alert("社区插件", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }

  private var mineBinding: Binding<Bool> {
    Binding(get: { mineOnly }, set: { value in
      guard value != mineOnly else { return }
      if value && !signedIn { showAccount = true; return }
      mineOnly = value
      startLoad()
    })
  }

  private var kindFilter: some View {
    ScrollView(.horizontal, showsIndicators: false) {
      HStack(spacing: 8) {
        chip("全部", nil)
        ForEach(CommunityPluginAPI.kinds, id: \.self) { chip($0.label, $0) }
      }
    }
    .accessibilityLabel("插件类型")
  }

  private func chip(_ title: String, _ value: PluginKind?) -> some View {
    Button {
      guard kind != value else { return }
      kind = value
      startLoad()
    } label: {
      DesignPillLabel(title: title, style: kind == value ? .filled : .neutral, size: .regular)
    }
    .buttonStyle(.plain)
    .accessibilityAddTraits(kind == value ? .isSelected : [])
    .accessibilityIdentifier("communityPluginKind-\(value?.rawValue ?? "all")")
  }

  @ViewBuilder private var feedback: some View {
    if let notice { Text(notice).font(.footnote).foregroundStyle(MetasequoiaTheme.accent) }
    if loaded && !busy && paging.items.isEmpty {
      let searching = !activeSearch.isEmpty || kind != nil
      Text(paging.hasMore
        ? (searching ? "前面的插件这台设备都不能安装，点「加载更多」继续查找。" : "前面的插件这台设备都不能安装，点「加载更多」查看更早发布的插件。")
        : (mineOnly ? "你还没有发布过插件。" : searching ? "没有匹配的插件。" : "社区里还没有插件，安装或制作插件后可以点「发布我的插件」分享出来。"))
        .font(.subheadline).foregroundStyle(MetasequoiaTheme.sub)
        .frame(maxWidth: .infinity).padding(.vertical, 24)
    }
  }

  private func row(_ item: CommunityPlugin) -> some View {
    Button { selected = item } label: {
      HStack(spacing: 12) {
        CommunityGlyphTile(name: item.name)
        VStack(alignment: .leading, spacing: 2) {
          HStack(spacing: 6) {
            Text(item.name).font(.system(size: 15, weight: .semibold)).foregroundStyle(.primary).lineLimit(1)
            if item.removed { CommunityRemovedBadge() }
          }
          Text([item.kindLabel, item.author].filter { !$0.isEmpty }.joined(separator: " · "))
            .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
          if !item.description.isEmpty {
            Text(item.description).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(2)
          }
          Text(CommunityPluginText.metrics(item)).font(.system(size: 11)).foregroundStyle(MetasequoiaTheme.sub)
        }
        Spacer(minLength: 0)
        DesignChevron(fontSize: 13)
      }
      .padding(.vertical, 12).padding(.horizontal, 14)
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .accessibilityLabel("查看插件 \(item.name)")
    .accessibilityIdentifier("communityPlugin-\(item.id)")
  }

  @MainActor private func load(append: Bool = false, id scheduledID: UUID? = nil) async {
    guard request.isVisible else { return }
    let id = scheduledID ?? request.begin()
    guard request.isCurrent(id) else { return }
    busy = true
    if !append { activeSearch = search.trimmingCharacters(in: .whitespacesAndNewlines) }
    defer { if request.isCurrent(id) { busy = false; loaded = true } }
    do {
      let page = try await CommunityPluginAPI.list(offset: append ? paging.offset : 0, search: activeSearch,
                                                   kind: kind, mine: mineOnly)
      try request.checkCurrent(id)
      paging.accept(page, append: append)
    } catch is CancellationError {
    } catch {
      if request.isCurrent(id) {
        if !append { paging = CommunityPluginPaging() }
        message = error.localizedDescription
      }
    }
  }

  @MainActor private func startLoad(append: Bool = false) {
    if !append { notice = nil }
    ViewRequestTask.startLoad(request: $request, pending: $pending) { id in
      await load(append: append, id: id)
    }
  }
}

enum CommunityPluginText {
  /// 下载数和评分，与社区卡片相同。
  static func metrics(_ item: CommunityPlugin) -> String {
    let rating = item.rating_count == 0 ? "暂无评分" : String(format: "%.1f 分 · %d 人评分", item.rating_average, item.rating_count)
    return "\(item.downloads) 次下载 · \(rating)"
  }

  /// 包的大小，一位小数的 MB，与 `candidateSkinMegabytes` 相同。
  static func megabytes(_ bytes: Int) -> String {
    String(format: "%.1f MB", Double(bytes) / (1024 * 1024))
  }
}

/// 社区插件的详情：摘要、安装（同 id 的包已装着时先确认整包替换）、评分、下架自己的作品和举报别人的作品。
struct CommunityPluginDetail: View {
  @ObservedObject var library: PluginLibrary
  let initial: CommunityPlugin
  /// 回到列表时更新那一行；下架后为 nil，列表去掉这一行。
  let onChange: (CommunityPlugin?) -> Void
  @State private var updated: CommunityPlugin?
  @State private var busy = false
  @State private var detailBusy = true
  @State private var message: String?
  @State private var installedNow = false
  @State private var confirmingReplace = false
  @State private var confirmingUnpublish = false
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()
  @Environment(\.dismiss) private var dismiss

  private var plugin: CommunityPlugin { updated ?? initial }

  var body: some View {
    Form {
      Section {
        HStack(spacing: 6) {
          Text(plugin.name).font(.title3.bold())
          if plugin.removed { CommunityRemovedBadge() }
        }
        Text([plugin.kindLabel, plugin.author, plugin.version.isEmpty ? "" : "v\(plugin.version)"]
          .filter { !$0.isEmpty }.joined(separator: " · "))
          .font(.subheadline).foregroundStyle(.secondary)
        if !plugin.description.isEmpty { Text(plugin.description) }
        Text([plugin.plugin_id, plugin.license.isEmpty ? "" : "授权 \(plugin.license)", CommunityPluginText.megabytes(plugin.size)]
          .filter { !$0.isEmpty }.joined(separator: " · "))
          .font(.footnote).foregroundStyle(.secondary)
        Text(CommunityPluginText.metrics(plugin)).font(.footnote).foregroundStyle(.secondary)
        if detailBusy { ProgressView("正在读取插件详情…") }
      }
      .designRow()
      Section {
        if installedNow {
          Text("已安装到插件目录，可在「我的插件」中选用。").foregroundStyle(MetasequoiaTheme.accent)
        }
        Button { install(replace: false) } label: {
          Label(installedNow ? "重新安装" : "安装", systemImage: "arrow.down.circle")
        }
        .disabled(busy || detailBusy)
        .accessibilityIdentifier("installCommunityPlugin")
      } footer: {
        if plugin.kind == PluginKind.sound.rawValue {
          Text("这台设备只播放按键音效包；按键旋律装上也不会播放。")
        }
      }
      .designRow()
      if plugin.owned {
        Section {
          Button("下架这个插件", role: .destructive) { confirmingUnpublish = true }
            .disabled(busy)
            .accessibilityIdentifier("unpublishCommunityPlugin")
        }
        .designRow()
      } else {
        Section("我的评分（安装后可评，可重新选择）") {
          HStack {
            ForEach(1...5, id: \.self) { stars in
              Button { rate(stars) } label: {
                Image(systemName: stars <= plugin.my_rating ? "star.fill" : "star").frame(width: 44, height: 44)
              }
              .buttonStyle(.plain)
              .accessibilityLabel("评 \(stars) 星")
              .disabled(busy)
            }
          }
          CommunityReportButton(kind: "plugins", itemID: plugin.id)
          CommunityBlockAuthorButton(kind: "plugins", itemID: plugin.id, author: plugin.author)
        }
        .designRow()
      }
      if busy { Section { ProgressView().frame(maxWidth: .infinity) }.designRow() }
    }
    .designPage()
    .navigationTitle("插件详情")
    .navigationBarTitleDisplayMode(.inline)
    .task { await refresh() }
    .confirmationDialog("确认替换插件", isPresented: $confirmingReplace, titleVisibility: .visible) {
      Button("替换", role: .destructive) { install(replace: true) }
    } message: {
      Text("已安装同 id 的\(plugin.kindLabel)“\(plugin.plugin_id)”，安装会整体替换它。")
    }
    .confirmationDialog("确认下架插件", isPresented: $confirmingUnpublish, titleVisibility: .visible) {
      Button("下架", role: .destructive) { unpublish() }
    } message: {
      Text("下架后其他用户无法再下载，下载数和评分会清空；已安装的插件会保留。确定下架“\(plugin.name)”吗？")
    }
    .alert("社区插件", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
    .trackViewRequest(request: $request, pending: $pending, onDisappear: { busy = false })
  }

  @MainActor private func refresh() async {
    detailBusy = true
    defer { detailBusy = false }
    if let latest = try? await CommunityPluginAPI.detail(initial.id) {
      updated = latest
      onChange(latest)
    }
  }

  private func run(_ action: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending, action: action)
  }

  /// 安装会整体替换同类型同 id 的包，所以先看本机目录，有就先问。
  private func install(replace: Bool) {
    let target = plugin
    if !replace, library.catalog.package(kind: PluginKind(rawValue: target.kind) ?? .sound, id: target.plugin_id) != nil,
       PluginKind(rawValue: target.kind) != nil {
      confirmingReplace = true
      return
    }
    run { id in
      let pack = try await CommunityPluginAPI.install(target)
      try request.checkCurrent(id)
      installedNow = true
      library.installed(pack)
      if let latest = try? await CommunityPluginAPI.detail(target.id) {
        try request.checkCurrent(id)
        updated = latest
        onChange(latest)
      }
    }
  }

  private func rate(_ stars: Int) {
    run { id in
      try await CommunityPluginAPI.rate(plugin.id, stars: stars)
      try request.checkCurrent(id)
      let latest = try await CommunityPluginAPI.detail(plugin.id)
      try request.checkCurrent(id)
      updated = latest
      onChange(latest)
    }
  }

  private func unpublish() {
    run { id in
      try await CommunityPluginAPI.delete(plugin.id)
      try request.checkCurrent(id)
      onChange(nil)
      ToastCenter.shared.show("已下架这个插件；其他用户将无法再下载。已安装的插件会保留。")
      dismiss()
    }
  }
}

/// 「发布我的插件」：选一个自己装的包（内置的不能发布），本机打包预检，填名称、说明并确认有权发布。失败重试沿用同一个发布 id。只列这台设备用得上的包。
struct CommunityPluginPublishView: View {
  @ObservedObject var library: PluginLibrary
  let onPublished: (CommunityPlugin) -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var selection = ""
  @State private var preview: CommunityPluginPackPreview?
  @State private var previewError: String?
  @State private var previewLoading = false
  @State private var name = ""
  @State private var description = ""
  @State private var agreed = false
  @State private var publicationID = UUID().uuidString.lowercased()
  @State private var busy = false
  @State private var message: String?

  private var options: [PluginPackage] {
    library.catalog.packages.filter { !$0.builtin && $0.usedOnPhone }
  }
  private var chosen: PluginPackage? { options.first { $0.key == selection } }
  private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
  private var trimmedDescription: String { description.trimmingCharacters(in: .whitespacesAndNewlines) }
  /// 名称 1 到 32 个字，说明不超过 280 个字，与 `communityPublishFields` 相同。
  private var ready: Bool {
    preview != nil && !previewLoading && !trimmedName.isEmpty && trimmedName.count <= 32
      && trimmedName.unicodeScalars.count <= 32 && trimmedDescription.unicodeScalars.count <= 280 && agreed
  }

  var body: some View {
    NavigationStack {
      Form {
        if options.isEmpty {
          Section {
            Text("还没有可发布的插件。内置插件不能发布，请先在「我的插件」中导入或制作自己的指令表、短语表、辅助码表、单词本、符号集或按键音效包。")
              .foregroundStyle(.secondary)
          }
          .designRow()
        } else {
          Section {
            Picker("发布插件", selection: $selection) {
              ForEach(options) { pack in
                Text("\(pack.kindLabel) · \(pack.name == pack.id ? pack.id : "\(pack.name)（\(pack.id)）")").tag(pack.key)
              }
            }
            .accessibilityIdentifier("publishPluginPicker")
            if previewLoading { ProgressView("正在检查插件…") }
            if let previewError { Text(previewError).foregroundStyle(.red) }
            if let preview {
              Text(["v\(preview.version)", preview.license.isEmpty ? "" : "授权 \(preview.license)", "\(preview.fileCount) 个文件",
                    "\(CommunityPluginText.megabytes(preview.size)) / 8 MB"].filter { !$0.isEmpty }.joined(separator: " · "))
                .font(.footnote).foregroundStyle(.secondary)
            }
          }
          .designRow()
          if preview != nil {
            Section {
              TextField("名称", text: $name).accessibilityLabel("发布插件名称")
              TextField("说明", text: $description, axis: .vertical).lineLimit(2...6).accessibilityLabel("发布插件说明")
              Toggle("我拥有插件中音频与文字的发布权利，并同意其他用户按包内授权免费下载使用", isOn: $agreed)
                .accessibilityIdentifier("publishPluginAgreement")
            } footer: {
              Text("上传的是插件目录中的全部文件（不含隐藏文件），压缩后不超过 8 MB；每个账号最多发布 20 个、合计 32 MB，每小时最多发布 10 次。其他用户下载后会按原样安装。")
            }
            .designRow()
            Section {
              Button(busy ? "正在发布…" : "公开发布") { publish() }
                .disabled(busy || !ready)
                .accessibilityIdentifier("publishPluginSubmit")
            }
            .designRow()
          }
        }
      }
      .designPage()
      .fittedSheet()
      .navigationTitle("发布插件")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() }.disabled(busy) }
      }
      .alert("发布插件", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
        Button("好", role: .cancel) {}
      } message: { Text(message ?? "") }
    }
    .interactiveDismissDisabled(busy)
    .onAppear { if selection.isEmpty { selection = options.first?.key ?? "" } }
    .task(id: selection) { await loadPreview() }
  }

  @MainActor private func loadPreview() async {
    preview = nil
    previewError = nil
    agreed = false
    publicationID = UUID().uuidString.lowercased()
    guard let chosen else { return }
    previewLoading = true
    defer { previewLoading = false }
    do {
      let value = try await CommunityPluginAPI.packPreview(kind: chosen.kind, pluginID: chosen.id)
      guard !Task.isCancelled else { return }
      preview = value
      name = String(value.suggestedName.prefix(32))
      description = value.suggestedDescription
    } catch is CancellationError {
    } catch {
      previewError = error.localizedDescription
    }
  }

  private func publish() {
    guard let chosen, ready, !busy else { return }
    busy = true
    Task {
      defer { busy = false }
      do {
        let published = try await CommunityPluginAPI.publish(kind: chosen.kind, pluginID: chosen.id, id: publicationID,
                                                             name: trimmedName, description: trimmedDescription)
        onPublished(published)
      } catch is CancellationError {
      } catch {
        message = error.localizedDescription
      }
    }
  }
}

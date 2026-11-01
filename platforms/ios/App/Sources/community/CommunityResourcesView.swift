import SwiftUI
import UniformTypeIdentifiers

/// 「社区」标签页的根页面：标题下固定一排「皮肤 / 词库」分段，固定方式与皮肤页的分类相同。两段分别嵌入完整的皮肤社区和词库社区列表，搜索、分类、详情、获取或添加、举报和发布都在各自的列表里，右上角的发布按钮跟着当前分段走。所选分段存在 `AppNavigation` 里，推入详情页再返回、切换标签页再回来都保留，词库页的「浏览更多词库」也靠它直接落在「词库」。回复模板不在这里，仍在「AI 润色与回复」页里浏览。
struct CommunityHomeView: View {
  @EnvironmentObject private var navigation: AppNavigation
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  var body: some View {
    Group {
      switch navigation.communitySegment {
      case .skin: SkinCommunityView(embedded: true)
      case .dictionary: CommunityResourcesView(kind: .dictionary, embedded: true)
      }
    }
    .designPinnedSegments {
      DesignSegmentedControl(items: [("皮肤", AppNavigation.CommunitySegment.skin), ("词库", .dictionary)],
                             selection: $navigation.communitySegment, style: .rounded, identifierPrefix: "communityTab")
        .frame(maxWidth: horizontalSizeClass == .regular ? CommunityLayout.regularMaxWidth : .infinity)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 16).padding(.vertical, 8)
    }
    .designCanvasBackground()
    .navigationTitle("社区")
    .navigationBarTitleDisplayMode(.large)
  }
}

struct CommunityResourcesView: View {
  let kind: CommunityResourceKind
  var initialScope = ""
  /// 嵌在「社区」标签页的「词库」分段里时为 true：不设自己的导航标题，沿用「社区」；发布按钮照常放在导航栏右侧。
  var embedded = false
  private var scope: String { initialScope }
  @State private var search = ""
  @State private var items: [CommunityResource] = []
  @State private var more = false
  @State private var busy = false
  @State private var message: String?
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()
  @State private var publishing = false
  @State private var signedIn = false
  @State private var showAccount = false
  /// 「添加」正在进行中的 id，对应的胶囊按钮因此显示「添加中」，并忽略再次点按。
  @State private var adding: Set<String> = []
  /// 本机已经装上的作品 id：排队导入过的社区词库，或已复制进回复键盘模板库的回复模板。胶囊按钮据此显示「已添加」，与 Android 的 `taken()` 一致；服务器上的收藏只决定详情页的收藏按钮。
  @State private var installed: Set<String> = []
  /// 等待导入确认的词库：导入会改动个人词库，所以绝不在单次点按时直接发生。
  @State private var confirmingImport: CommunityResource?
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  /// 全部作品和「我发布的」都可以发布新作品；「收藏的」只是别人作品的书签，不放发布按钮。
  private var offersPublishing: Bool { initialScope != "saved" }
  /// 登录后才能发布。模拟器上的社区预览没有账号，直接打开发布表单，好让 UI 测试走到它。
  private var opensPublisher: Bool {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled { return true }
    #endif
    return signedIn
  }
  var body: some View {
    if embedded { page } else {
      page.navigationTitle(initialScope.isEmpty ? "\(kind.title)社区" : "\(initialScope == "saved" ? "收藏的" : "我发布的")\(kind.title)")
        .navigationBarTitleDisplayMode(.inline)
    }
  }

  private var page: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 16) {
        CommunitySearchField(text: $search, placeholder: "搜索\(kind.title)") { startLoad() }
        if items.isEmpty && !busy {
          VStack(spacing: 12) {
            Image(systemName: kind.icon).font(.largeTitle).foregroundStyle(MetasequoiaTheme.accent)
            Text(scope == "" ? "期待第一份\(kind == .dictionary ? "词库" : "回复模板")" : "这里还没有作品")
            Text(scope == "saved" ? "在「\(kind == .dictionary ? "词库" : "AI 润色与回复")」页浏览更多\(kind.title)，收藏喜欢的作品。" : "点右上角 + 发布你的第一份作品。")
              .font(.caption).foregroundStyle(MetasequoiaTheme.sub)
          }.frame(maxWidth: .infinity).padding(.vertical, 35)
        }
        if !items.isEmpty {
          DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
              if index > 0 { DesignDivider(leading: 0) }
              row(item)
            }
          }
        }
        if busy { ProgressView().frame(maxWidth: .infinity) }
        if more { Button("加载更多") { startLoad(append: true) }.disabled(busy).frame(maxWidth: .infinity) }
      }.communityPageContent(regular: horizontalSizeClass == .regular)
    }
    .background(MetasequoiaTheme.canvas)
    .toolbar {
      if offersPublishing {
        ToolbarItem(placement: .navigationBarTrailing) {
          Button { if opensPublisher { publishing = true } else { showAccount = true } } label: {
            Label("发布", systemImage: "plus")
          }
          .accessibilityLabel("发布\(kind.title)")
          .accessibilityIdentifier("publishCommunityResource")
        }
      }
    }
    .sheet(isPresented: $publishing) {
      CommunityResourceEditor(kind: kind, onPublished: { startLoad() })
    }
    .sheet(isPresented: $showAccount, onDismiss: {
      Task {
        let state = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
        guard request.isVisible, !Task.isCancelled else { return }
        signedIn = state
        if signedIn { publishing = true }
      }
    }) { AccountLoginSheet() }
    .task {
      let state = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
      guard request.isVisible, !Task.isCancelled else { return }
      signedIn = state
      await load()
    }
    .refreshable { await refreshInstalled(); await load() }
    .onReceive(NotificationCenter.default.publisher(for: CommunityBlocks.didChange)) { _ in startLoad() }
    // 每次回到列表都重读：详情页里导入词库、添加或移除回复模板后，返回时胶囊按钮跟着变。
    .trackViewRequest(request: $request, pending: $pending, onAppear: {
      Task { await refreshInstalled() }
    }, onDisappear: {
      busy = false
    })
    .confirmationDialog("导入这版词库到本机？", isPresented: Binding(get: { confirmingImport != nil }, set: { if !$0 { confirmingImport = nil } }),
                        titleVisibility: .visible, presenting: confirmingImport) { item in
      Button("导入 \(item.content.entries?.count ?? 0) 条词条") { add(item) }
    } message: { _ in
      Text("词条会添加到本机个人词库，开启完全访问后在键盘空闲时处理。")
    }
    .alert("社区", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }

  /// 分组卡片里的一行：字形图块、名称和元信息打开详情页；旁边的胶囊按钮不离开列表就能添加该作品。胶囊按钮放在链接外面，这样两者在点按和 VoiceOver 中各自是独立的按钮。
  private func row(_ item: CommunityResource) -> some View {
    HStack(spacing: 12) {
      NavigationLink { CommunityResourceDetail(initial: item) } label: {
        HStack(spacing: 12) {
          CommunityGlyphTile(name: item.name)
          VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
              Text(item.name).font(.system(size: 15, weight: .semibold)).foregroundStyle(.primary).lineLimit(1)
              if item.removed { CommunityRemovedBadge() }
            }
            let meta = CommunityListingText.resourceMeta(item)
            if !meta.isEmpty { Text(meta).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1) }
            // 回复模板没有条目数，所以描述里写明它是什么，与 Android 一致。
            if item.kind == .reply && !item.description.isEmpty {
              Text(item.description).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
            }
          }
          Spacer(minLength: 0)
        }
        .padding(.vertical, 12).padding(.leading, 14)
        .contentShape(Rectangle())
      }
      .buttonStyle(.plain).accessibilityIdentifier("communityResource-\(item.id)")
      pill(item).padding(.trailing, 14)
    }
  }

  @ViewBuilder private func pill(_ item: CommunityResource) -> some View {
    let pending = adding.contains(item.id)
    if installed.contains(item.id) {
      DesignPillLabel(title: "已添加", style: .neutral, size: .regular)
        .accessibilityLabel("已添加，\(item.name)")
    } else {
      Button {
        if item.kind == .dictionary { confirmingImport = item } else { add(item) }
      } label: {
        DesignPillLabel(title: pending ? "添加中" : "添加", size: .regular)
          .opacity(pending ? 0.6 : 1)
      }
      .buttonStyle(.plain).disabled(pending)
      .accessibilityLabel("\(pending ? "添加中" : "添加")，\(item.name)")
      .accessibilityIdentifier("communityResourceAdd-\(item.id)")
    }
  }

  /// 列表上的添加。词库先把确认时统计的那个版本排队导入个人词库，再只在已登录时收藏它（与词库页的添加一致），所以未登录也能添加，导入失败也不会留下收藏；回复模板先收藏，再把最新版本复制进回复键盘的模板库。
  private func add(_ item: CommunityResource) {
    guard request.isVisible, !adding.contains(item.id) else { return }
    adding.insert(item.id)
    Task { @MainActor in
      defer { if request.isVisible { adding.remove(item.id) } }
      do {
        var latest = item
        if item.kind == .dictionary {
          // 用确认时统计的那个版本，就像详情页导入的是它所显示的版本。
          try PersonalDictionaryStore().enqueueImport((item.content.entries ?? []).map { try $0.localWord() })
          try ensureVisible()
          CommunityDictionaryImports.record(item.id)
          installed.insert(item.id)
          // 收藏只是为了出现在「收藏」里并跟进更新，需要登录。
          if try await SkinCommunityAPI.shared.signedIn() {
            try await SkinCommunityAPI.shared.saveResource(item.id, saved: true)
            latest = try await SkinCommunityAPI.shared.resource(item.id)
          }
        } else {
          latest = try await CommunityReplyInstall.add(item)
          try ensureVisible()
          installed.insert(latest.id)
        }
        try ensureVisible()
        if let index = items.firstIndex(where: { $0.id == latest.id }) { items[index] = latest }
        ToastCenter.shared.show("已添加「\(latest.name)」")
      } catch is CancellationError { }
      catch { if request.isVisible { message = error.localizedDescription } }
    }
  }

  /// 重读本机已经装上的作品。回复模板库正被别处写入而读不到时保留上一次的结果。
  @MainActor private func refreshInstalled() async {
    if kind == .dictionary {
      guard request.isVisible else { return }
      installed = CommunityDictionaryImports.ids()
    } else if let ids = await CommunityReplyInstall.installedIDs() {
      guard request.isVisible, !Task.isCancelled else { return }
      installed = ids
    }
  }

  @MainActor private func load(append: Bool = false, id scheduledID: UUID? = nil) async {
    guard request.isVisible else { return }
    let id = scheduledID ?? request.begin()
    guard request.isCurrent(id) else { return }
    busy = true
    defer {
      if request.isCurrent(id) {
        busy = false
        if scheduledID == nil { pending = nil }
      }
    }
    do {
      let page = try await SkinCommunityAPI.shared.resources(kind, scope: scope, search: search, offset: append ? items.count : 0)
      try request.checkCurrent(id)
      if append { items += CommunityUnseenItems.filter(existing: items, incoming: page.items) }
      else { items = page.items }
      more = page.has_more
    } catch is CancellationError { }
    catch { if request.isCurrent(id) { if !append { items = []; more = false }; message = error.localizedDescription } }
  }

  @MainActor private func startLoad(append: Bool = false) {
    guard !busy else { return }
    ViewRequestTask.startLoad(request: $request, pending: $pending) { id in
      await load(append: append, id: id)
    }
  }

  @MainActor private func ensureVisible() throws {
    try Task.checkCancellation()
    guard request.isVisible else { throw CancellationError() }
  }

}

/// 把社区回复模板装进本机高情商回复键盘的模板库，由完整列表和「AI 润色与回复」页的社区回复模板共用。
enum CommunityReplyInstall {
  /// 先收藏（需要登录），再把最新版本复制进回复键盘的模板库；返回最新版本。
  @MainActor static func add(_ item: CommunityResource) async throws -> CommunityResource {
    try Task.checkCancellation()
    try await SkinCommunityAPI.shared.saveResource(item.id, saved: true)
    try Task.checkCancellation()
    let latest = try await SkinCommunityAPI.shared.resource(item.id)
    try Task.checkCancellation()
    try await Task.detached(priority: .userInitiated) {
      try CommunityLibrary.save(latest)
    }.value
    return latest
  }

  /// 模板库里已有的回复模板 id；模板库正被别处写入而读不到时为 nil，调用方保留上一次的结果。
  static func installedIDs() async -> Set<String>? {
    guard let library = try? await Task.detached(priority: .userInitiated, operation: { try CommunityLibrary.read() }).value else {
      return nil
    }
    return Set(library.filter { $0.kind == .reply }.map(\.id))
  }
}

/// 「AI 润色与回复」页里的社区回复模板：直接列出社区的前几份，每份带「添加」；最后一行「浏览更多回复模板」推入完整的回复模板社区，搜索、详情、收藏、评分、举报和发布都在那里。回复模板只给键盘的高情商回复用，所以放在配置它所用 AI 服务的这一页。
struct CommunityReplyTemplatesSection: View {
  /// 这一页直接列出的回复模板数。
  private static let limit = 3

  @State private var items: [CommunityResource]?
  @State private var failed = false
  @State private var installed: Set<String> = []
  @State private var adding: Set<String> = []
  @State private var message: String?
  @State private var pending: Task<Void, Never>?
  @State private var addTasks: [String: Task<Void, Never>] = [:]
  @State private var addGeneration = OperationGeneration()
  @State private var request = ViewRequestGeneration()

  var body: some View {
    Section {
      if let items {
        if items.isEmpty {
          Text("社区里还没有回复模板。").foregroundStyle(.secondary)
        }
        ForEach(items) { row($0) }
      } else {
        Text(failed ? "暂时无法加载社区回复模板" : "正在读取社区回复模板…").foregroundStyle(.secondary)
      }
      // 加载和提示挂在这一行上：它总在，而挂在 Section 上的修饰符会分给每一行。
      NavigationLink { CommunityResourcesView(kind: .reply) } label: { Text("浏览更多回复模板") }
        .accessibilityIdentifier("replyCommunityLink")
        .trackViewRequest(request: $request, pending: $pending, onAppear: {
          startLoad()
        }, onDisappear: {
          addGeneration.invalidate()
          addTasks.values.forEach { $0.cancel() }
          addTasks.removeAll()
          adding.removeAll()
        })
        .alert("社区回复模板", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
          Button("好", role: .cancel) {}
        } message: { Text(message ?? "") }
    } header: {
      DesignSectionHeader(subject: "社区回复模板", info: "添加后，在键盘的高情商回复里点「模板」即可选用；只有点击生成时才会发送文字。") { Text("社区回复模板") }
    }
  }

  private func row(_ item: CommunityResource) -> some View {
    let isAdded = installed.contains(item.id)
    let isAdding = adding.contains(item.id)
    let title = isAdded ? "已添加" : isAdding ? "添加中" : "添加"
    return HStack(spacing: 12) {
      VStack(alignment: .leading, spacing: 2) {
        Text(item.name).lineLimit(1)
        let meta = item.description.isEmpty ? CommunityListingText.resourceMeta(item) : item.description
        if !meta.isEmpty { Text(meta).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
      }
      Spacer(minLength: 8)
      Button { add(item) } label: {
        DesignPillLabel(title: title, style: isAdded ? .neutral : .tonal, size: .regular)
          .opacity(isAdding ? 0.6 : 1)
      }
      .buttonStyle(.borderless)
      .disabled(isAdded || isAdding)
      .accessibilityLabel("\(title)，\(item.name)")
      .accessibilityIdentifier("replyCommunityAdd-\(item.id)")
    }
  }

  private func add(_ item: CommunityResource) {
    guard request.isVisible, !adding.contains(item.id) else { return }
    let generation = addGeneration.current()
    adding.insert(item.id)
    addTasks[item.id] = Task { @MainActor in
      defer {
        if addGeneration.isCurrent(generation) {
          adding.remove(item.id)
          addTasks[item.id] = nil
        }
      }
      do {
        let latest = try await CommunityReplyInstall.add(item)
        try ensureVisible(generation)
        installed.insert(latest.id)
        if let index = items?.firstIndex(where: { $0.id == latest.id }) { items?[index] = latest }
        ToastCenter.shared.show("已添加「\(latest.name)」")
      } catch is CancellationError { }
      catch { if request.isVisible, addGeneration.isCurrent(generation) { message = error.localizedDescription } }
    }
  }

  @MainActor private func refreshInstalled() async {
    if let ids = await CommunityReplyInstall.installedIDs() {
      guard request.isVisible, !Task.isCancelled else { return }
      installed = ids
    }
  }

  @MainActor private func load(_ id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let page = try await SkinCommunityAPI.shared.resources(.reply)
      try request.checkCurrent(id)
      items = Array(page.items.prefix(Self.limit))
      failed = false
    } catch is CancellationError {
      return
    } catch {
      // 已经加载过的列表保留下来；刷新失败只替换加载提示。
      if request.isCurrent(id), items == nil { failed = true }
    }
  }

  @MainActor private func startLoad() {
    ViewRequestTask.startLoad(request: $request, pending: $pending) { id in
      await refreshInstalled()
      await load(id)
    }
  }

  @MainActor private func ensureVisible(_ generation: UUID) throws {
    try Task.checkCancellation()
    guard request.isVisible, addGeneration.isCurrent(generation) else { throw CancellationError() }
  }

}

struct CommunityResourceDetail: View {
  let initial: CommunityResource
  @State private var updated: CommunityResource?
  @State private var busy = false
  @State private var message: String?
  @State private var confirmImport = false
  @State private var confirmDelete = false
  @State private var editing = false
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()
  @Environment(\.dismiss) private var dismiss
  private var item: CommunityResource { updated ?? initial }
  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 18) {
        HStack {
          Label(item.name, systemImage: item.kind.icon).font(.title2.bold())
          if item.removed { CommunityRemovedBadge() }
        }
        Text("\(item.author) · v\(item.revision)").foregroundStyle(.secondary)
        Text(item.description)
        HStack {
          Label("\(item.saves) 人收藏", systemImage: "bookmark")
          Text(item.rating_count == 0 ? "暂无评分" : String(format: "%.1f 分 · %d 人", item.rating_average, item.rating_count))
        }.font(.caption).foregroundStyle(.secondary)
        if item.kind == .dictionary {
          VStack(alignment: .leading, spacing: 8) {
            Text("词条预览 · \(item.content.entries?.count ?? 0) 条").font(.headline)
            ForEach(Array((item.content.entries ?? []).enumerated()), id: \.offset) { _, word in
              HStack { Text(word.word); Spacer(); Text(word.code).foregroundStyle(.secondary) }.font(.subheadline)
              Divider()
            }
          }
          Button("导入这版词库到本机") { confirmImport = true }.buttonStyle(.borderedProminent).disabled(busy)
            .accessibilityIdentifier("communityImportLocal")
          Text("导入会添加到本机个人词库。开启完全访问后，在键盘空闲时由引擎处理；收藏和查看更新不会自动修改词条。")
            .font(.caption).foregroundStyle(.secondary)
          NavigationLink("查看导入状态", destination: PersonalDictionaryView())
          if let resourceID = UUID(uuidString: item.id) {
            CommunityCloudImportButton(resourceID: resourceID, resourceRevision: item.revision)
          }
        } else {
          Text("提示词预览").font(.headline)
          Text(item.content.prompt ?? "").font(.body).textSelection(.enabled)
            .padding().frame(maxWidth: .infinity, alignment: .leading)
            .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: 12))
          Button("添加到高情商回复键盘") { run { id in
            try await SkinCommunityAPI.shared.saveResource(item.id, saved: true)
            try request.checkCurrent(id)
            let latest = try await SkinCommunityAPI.shared.resource(item.id)
            try request.checkCurrent(id)
            try await Task.detached(priority: .userInitiated) {
              try CommunityLibrary.save(latest)
            }.value
            try request.checkCurrent(id)
            updated = latest
            message = "已添加。在高情商回复键盘点击「模板」即可选择；只有点击生成时才会发送文字。"
          }}.buttonStyle(.borderedProminent).disabled(busy)
        }
        Button(item.saved ? "取消收藏" : "收藏，关注后续更新") { run { id in
          try await SkinCommunityAPI.shared.saveResource(item.id, saved: !item.saved)
          try request.checkCurrent(id)
          let latest = try await SkinCommunityAPI.shared.resource(item.id)
          try request.checkCurrent(id)
          updated = latest
        }}.disabled(busy)
        if item.kind == .reply {
          Button("从本机高情商回复键盘移除") { run { id in
            let resourceID = item.id
            try await Task.detached(priority: .userInitiated) {
              try CommunityLibrary.remove(resourceID)
            }.value
            try request.checkCurrent(id)
            message = "已从本机移除，社区收藏保留。"
          }}.disabled(busy)
        }
        if !item.owned {
          HStack {
            Text("评分")
            ForEach(1...5, id: \.self) { stars in
              Button { run { id in
                try await SkinCommunityAPI.shared.rateResource(item.id, stars: stars)
                try request.checkCurrent(id)
                let latest = try await SkinCommunityAPI.shared.resource(item.id)
                try request.checkCurrent(id)
                updated = latest
              }} label: { Image(systemName: stars <= item.my_rating ? "star.fill" : "star") }
                .accessibilityLabel("\(stars) 星").disabled(busy || !item.saved)
            }
          }
          HStack(spacing: 20) {
            CommunityReportButton(kind: item.kind == .dictionary ? "dictionaries" : "replies", itemID: item.id)
            CommunityBlockAuthorButton(kind: item.kind == .dictionary ? "dictionaries" : "replies", itemID: item.id, author: item.author)
          }
        } else {
          Button("编辑并发布新版本") { editing = true }.disabled(busy)
          Button("下架作品", role: .destructive) { confirmDelete = true }.disabled(busy)
        }
        if busy { ProgressView() }
      }.padding(20)
    }.designCanvasBackground().navigationTitle(item.kind == .dictionary ? "词库详情" : "回复模板")
      .task {
        run { id in
        let latest = try await SkinCommunityAPI.shared.resource(initial.id)
        try request.checkCurrent(id)
        updated = latest
        }
      }
      .sheet(isPresented: $editing, onDismiss: { run { id in
        let latest = try await SkinCommunityAPI.shared.resource(initial.id)
        try request.checkCurrent(id)
        updated = latest
      } }) {
        CommunityResourceEditor(kind: item.kind, existing: item)
      }
      .confirmationDialog("确认导入预览中的 \(item.content.entries?.count ?? 0) 条词条？", isPresented: $confirmImport, titleVisibility: .visible) {
        Button("确认导入") { let selected = item; run { id in
          try PersonalDictionaryStore().enqueueImport((selected.content.entries ?? []).map { try $0.localWord() })
          try request.checkCurrent(id)
          CommunityDictionaryImports.record(selected.id)
          message = "已加入本机导入队列，打开水杉键盘后处理。可在「查看导入状态」检查每条结果。"
        }}
      }
      .confirmationDialog("下架后其他用户将无法获取此作品，已有本地副本保留。", isPresented: $confirmDelete, titleVisibility: .visible) {
        Button("下架", role: .destructive) { run { id in
          try await SkinCommunityAPI.shared.unpublishResource(item.id)
          try request.checkCurrent(id)
          dismiss()
        } }
      }
      .alert("社区", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
        Button("好", role: .cancel) {}
      } message: { Text(message ?? "") }
      .trackViewRequest(request: $request, pending: $pending, onDisappear: {
        busy = false
      })
  }
  private func run(_ action: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending, action: action)
  }
}

private extension PersonalWordKind {
  /// 社区词库收的种类（`pinyin`、`wubi`、`quick`、`english`，见 `SkinCommunityAPI.validResource`），不含 98 五笔。
  static let communityCases = allCases.filter { $0 != .wubi98 }
}

struct CommunityResourceEditor: View {
  let kind: CommunityResourceKind
  var existing: CommunityResource?
  var onPublished: () -> Void = {}
  @State private var publicationID = UUID().uuidString.lowercased()
  @State private var name = ""
  @State private var description = ""
  @State private var prompt = ""
  @State private var words: [PersonalWord] = []
  @State private var wordKind: PersonalWordKind = .pinyin
  @State private var code = ""
  @State private var word = ""
  @State private var importing = false
  @State private var busy = false
  @State private var message: String?
  @State private var confirming = false
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()
  @Environment(\.dismiss) private var dismiss
  var body: some View {
    NavigationStack {
      Form {
        Section("作品信息") {
          TextField("名称（最多 32 字）", text: $name)
          TextField("介绍（最多 280 字）", text: $description)
        }
        if kind == .reply {
          Section("回复提示词") {
            TextEditor(text: $prompt).frame(minHeight: 180).accessibilityIdentifier("communityPromptEditor")
            Text("描述语气、适用场景和输出要求。不要填写 API Key 或私人聊天内容，最多 2000 字。")
              .font(.caption).foregroundStyle(.secondary)
          }
        } else {
          Section("添加词条") {
            Picker("类型", selection: $wordKind) { ForEach(PersonalWordKind.communityCases) { Text($0.title).tag($0) } }
            TextField("编码，例如 ni hao", text: $code).textInputAutocapitalization(.never).autocorrectionDisabled()
            TextField("词语或短语", text: $word)
            Button("添加到待发布词库") {
              do {
                let entry = try PersonalWord(kind: wordKind, key: code, value: word).validated()
                guard words.count < 128, !words.contains(where: { $0.id == entry.id }) else { throw CommunityFailure(message: "词条重复或已达到 128 条。") }
                words.append(entry); code = ""; word = ""
              } catch { message = error.localizedDescription }
            }.disabled(code.isEmpty || word.isEmpty)
            Button("从词库 JSON 文件选择") { importing = true }
            // iOS 把细胞词库导入个人词库时不记来源，没法像共享设置页那样逐条核对，只能写明规则。
            Text("只发布你自己整理的词条。从搜狗细胞词库等第三方词库导入的词不能发布：它们的整理成果属于原作者，公开分享可能侵犯原作者的权利。")
              .font(.caption).foregroundStyle(.secondary)
          }
          Section("待发布预览 · \(words.count)/128") {
            ForEach(words) { entry in
              HStack { VStack(alignment: .leading) { Text(entry.value); Text(entry.key).font(.caption).foregroundStyle(.secondary) }
                Spacer(); Button(role: .destructive) { words.removeAll { $0.id == entry.id } } label: { Image(systemName: "minus.circle") }
              }
            }
          }
        }
        Section {
          Text("发布后所有用户均可查看这些内容。请确认你有分享权限，且不包含个人信息。只有预览中的内容会被发布。")
            .font(.caption).foregroundStyle(.secondary)
        }
      }.disabled(busy)
        .designPage()
        .fittedSheet()
        .navigationTitle(existing == nil ? "发布\(kind.title)" : "更新作品")
        .toolbar {
          ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() }.disabled(busy) }
          ToolbarItem(placement: .confirmationAction) {
            Button("发布") { confirming = true }
              .disabled(busy || name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || name.count > 32 || description.count > 280 || (kind == .reply ? prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || prompt.count > 2000 : words.isEmpty))
          }
        }
        .task {
          guard let existing else { return }
          publicationID = existing.id; name = existing.name; description = existing.description; prompt = existing.content.prompt ?? ""
          do { words = try (existing.content.entries ?? []).map { try $0.localWord() } } catch { message = error.localizedDescription }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
          guard request.isVisible else { return }
          do {
            let url = try result.get()
            run { id in
              let imported = try await Task.detached { try PersonalDictionaryImport.read(from: url) }.value
              try request.checkCurrent(id)
              guard imported.entries.allSatisfy({ PersonalWordKind.communityCases.contains($0.kind) }) else { throw CommunityFailure(message: "社区词库暂不支持 98 五笔词条，请从文件中去掉后再选择。") }
              let additions = CommunityUnseenItems.filter(existing: words, incoming: imported.entries)
              guard words.count + additions.count <= 128 else { throw CommunityFailure(message: "每份社区词库最多 128 条，请先精简文件。") }
              words += additions
            }
          } catch { message = error.localizedDescription }
        }
        .confirmationDialog("确认公开发布「\(name)」？", isPresented: $confirming, titleVisibility: .visible) {
          Button("公开发布") {
            run { id in
              let content = CommunityResourceContent(entries: kind == .dictionary ? words.map(CommunityWord.init) : nil,
                prompt: kind == .reply ? prompt : nil)
              try await SkinCommunityAPI.shared.publishResource(id: publicationID, kind: kind, name: name,
                description: description, content: content, revision: existing?.revision ?? 0)
              try request.checkCurrent(id)
              onPublished(); dismiss()
            }
          }
        }
        .alert("发布作品", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
          Button("好", role: .cancel) {}
        } message: { Text(message ?? "") }
    }
    .trackViewRequest(request: $request, pending: $pending, onDisappear: {
      busy = false
    })
  }


  @MainActor private func run(_ action: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending, action: action)
  }
}

import SwiftUI
import UniformTypeIdentifiers

struct CommunityHomeView: View {
  @EnvironmentObject private var navigation: AppNavigation
  private var category: Int { navigation.communityCategory }
  @State private var pendingPublish: Int?
  @State private var publishing: Int?
  @State private var account = false
  @State private var refresh = UUID()
  @State private var didPublish = false
  var body: some View {
    Group {
      if category == 0 { SkinCommunityView(embedded: true) }
      else { CommunityResourcesView(kind: category == 1 ? .dictionary : .reply).id(category) }
    }
    .id(refresh)
    // An inset rather than a row above the list, so the list stays the page's scroll view and the large title still folds into the bar as it scrolls.
    .safeAreaInset(edge: .top, spacing: 0) {
      HStack(spacing: 0) {
        categoryButton(0, title: "皮肤")
        categoryButton(1, title: "词库")
        categoryButton(2, title: "回复模板")
      }
      .padding(2)
      .background(Color(uiColor: .tertiarySystemFill), in: Capsule())
      .padding(.horizontal, 16).padding(.top, 4).padding(.bottom, 8)
      // Kept inside its own frame: a colour background reaches into the safe area by default, and here it would paint over the large title.
      .background(Color(uiColor: .systemGroupedBackground), ignoresSafeAreaEdges: [])
    }
    .background(Color(uiColor: .systemGroupedBackground))
    .navigationTitle("社区")
    .navigationBarTitleDisplayMode(.large)
    .toolbar {
      ToolbarItem(placement: .navigationBarTrailing) {
        Button { publish(category) } label: { Label("发布", systemImage: "plus").labelStyle(.titleAndIcon).font(.subheadline.weight(.semibold)) }
          .accessibilityLabel("发布作品").accessibilityIdentifier("publishCommunityWork")
      }
    }
    .sheet(isPresented: Binding(get: { publishing != nil }, set: { if !$0 { publishing = nil } }), onDismiss: {
      if didPublish { refresh = UUID(); didPublish = false }
    }) {
      if publishing == 0 { CommunityPublishView { didPublish = true } }
      else { CommunityResourceEditor(kind: publishing == 1 ? .dictionary : .reply, onPublished: { didPublish = true }) }
    }
    .sheet(isPresented: $account, onDismiss: {
      let pending = pendingPublish; pendingPublish = nil
      Task {
        if let pending, (try? await SkinCommunityAPI.shared.signedIn()) == true { publishing = pending }
      }
    }) { AccountLoginSheet() }
  }
  /// One segment of the design's capsule switch. Buttons rather than a segmented Picker: each keeps its own identifier and selected trait, which a Picker's segments do not carry.
  private func categoryButton(_ value: Int, title: String) -> some View {
    Button { navigation.communityCategory = value } label: {
      Text(title).font(.system(size: 13, weight: category == value ? .semibold : .regular))
        .frame(maxWidth: .infinity).frame(height: 30)
        .foregroundStyle(category == value ? Color.primary : Color.secondary)
        .background {
          if category == value {
            Capsule().fill(Color(uiColor: .systemBackground)).shadow(color: .black.opacity(0.08), radius: 2, y: 1)
          }
        }
        .contentShape(Capsule())
    }.buttonStyle(.plain).accessibilityIdentifier("communityCategory-\(value)")
      .accessibilityAddTraits(category == value ? [.isSelected] : [])
  }
  private func publish(_ kind: Int) {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled { publishing = kind; return }
    #endif
    Task { if (try? await SkinCommunityAPI.shared.signedIn()) == true { publishing = kind } else { pendingPublish = kind; account = true } }
  }
}

struct CommunityResourcesView: View {
  let kind: CommunityResourceKind
  var initialScope = ""
  private var scope: String { initialScope }
  @State private var search = ""
  @State private var items: [CommunityResource] = []
  @State private var more = false
  @State private var busy = false
  @State private var message: String?
  @State private var requestID = UUID()
  @State private var publishing = false
  @State private var signedIn = false
  @State private var showAccount = false
  /// The community home already owns the publish button; only the account-scoped list needs one.
  private var offersPublishing: Bool { !initialScope.isEmpty && initialScope != "saved" }
  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 16) {
        CommunitySearchField(text: $search, placeholder: "搜索\(kind.title)") { Task { await load() } }
        VStack(alignment: .leading, spacing: 4) {
          Text(scope.isEmpty ? (kind == .dictionary ? "好词，随手可得" : "找到舒服的表达") : (scope == "saved" ? "你的灵感收藏" : "你的公开作品"))
            .font(.system(size: 20, weight: .bold))
          Text(kind == .dictionary ? "把常用词带进键盘，让输入更顺手" : "收藏喜欢的语气，给每次回应一点灵感")
            .font(.caption).foregroundStyle(.secondary)
        }.padding(.vertical, 2)
        if items.isEmpty && !busy {
          VStack(spacing: 12) {
            Image(systemName: kind.icon).font(.largeTitle).foregroundStyle(MetasequoiaTheme.accent)
            Text(scope == "" ? "期待第一份\(kind == .dictionary ? "词库" : "回复模板")" : "这里还没有作品")
            Text(scope == "saved" ? "去社区逛逛，收藏喜欢的作品。" : "点右上角 + 发布你的第一份作品。")
              .font(.caption).foregroundStyle(.secondary)
          }.frame(maxWidth: .infinity).padding(.vertical, 35)
        }
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 14) {
          ForEach(items) { item in
            NavigationLink { CommunityResourceDetail(initial: item) } label: {
              CommunityResourceCard(item: item)
            }.buttonStyle(.plain).accessibilityIdentifier("communityResource-\(item.id)")
          }
        }
        if busy { ProgressView().frame(maxWidth: .infinity) }
        if more { Button("加载更多") { Task { await load(append: true) } }.disabled(busy) }
      }.padding(16)
    }
    .navigationTitle(initialScope.isEmpty ? "社区" : "\(initialScope == "saved" ? "收藏的" : "我发布的")\(kind.title)")
    // Without a scope this is the 社区 tab's own list, which carries the tab's large title.
    .navigationBarTitleDisplayMode(initialScope.isEmpty ? .large : .inline)
    .toolbar {
      if offersPublishing {
        ToolbarItem(placement: .navigationBarTrailing) {
          Button { if signedIn { publishing = true } else { showAccount = true } } label: {
            Label("发布", systemImage: "plus")
          }
          .accessibilityLabel("发布\(kind.title)")
          .accessibilityIdentifier("publishCommunityResource")
        }
      }
    }
    .sheet(isPresented: $publishing) {
      CommunityResourceEditor(kind: kind, onPublished: { Task { await load() } })
    }
    .sheet(isPresented: $showAccount, onDismiss: {
      Task {
        signedIn = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
        if signedIn { publishing = true }
      }
    }) { AccountLoginSheet() }
    .task {
      signedIn = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
      await load()
    }
    .refreshable { await load() }
    .alert("社区", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }
  @MainActor private func load(append: Bool = false) async {
    let id = UUID(); requestID = id; busy = true
    defer { if requestID == id { busy = false } }
    do {
      let page = try await SkinCommunityAPI.shared.resources(kind, scope: scope, search: search, offset: append ? items.count : 0)
      guard requestID == id else { return }
      if append { let ids = Set(items.map(\.id)); items += page.items.filter { !ids.contains($0.id) } }
      else { items = page.items }
      more = page.has_more
    } catch { if requestID == id { if !append { items = []; more = false }; message = error.localizedDescription } }
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
            .background(Color(uiColor: .secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 12))
          Button("添加到高情商回复键盘") { run {
            try await SkinCommunityAPI.shared.saveResource(item.id, saved: true)
            let latest = try await SkinCommunityAPI.shared.resource(item.id)
            try CommunityLibrary.save(latest); updated = latest
            message = "已添加。在高情商回复键盘点击「模板」即可选择；只有点击生成时才会发送文字。"
          }}.buttonStyle(.borderedProminent).disabled(busy)
        }
        Button(item.saved ? "取消收藏" : "收藏，关注后续更新") { run {
          try await SkinCommunityAPI.shared.saveResource(item.id, saved: !item.saved)
          updated = try await SkinCommunityAPI.shared.resource(item.id)
        }}.disabled(busy)
        if item.kind == .reply {
          Button("从本机高情商回复键盘移除") { run { try CommunityLibrary.remove(item.id); message = "已从本机移除，社区收藏保留。" } }.disabled(busy)
        }
        if !item.owned {
          HStack {
            Text("评分")
            ForEach(1...5, id: \.self) { stars in
              Button { run {
                try await SkinCommunityAPI.shared.rateResource(item.id, stars: stars)
                updated = try await SkinCommunityAPI.shared.resource(item.id)
              }} label: { Image(systemName: stars <= item.my_rating ? "star.fill" : "star") }
                .accessibilityLabel("\(stars) 星").disabled(busy || !item.saved)
            }
          }
          CommunityReportButton(kind: item.kind == .dictionary ? "dictionaries" : "replies", itemID: item.id)
        } else {
          Button("编辑并发布新版本") { editing = true }.disabled(busy)
          Button("下架作品", role: .destructive) { confirmDelete = true }.disabled(busy)
        }
        if busy { ProgressView() }
      }.padding(20)
    }.background(Color(uiColor: .systemGroupedBackground)).navigationTitle(item.kind == .dictionary ? "词库详情" : "回复模板")
      .task { run { updated = try await SkinCommunityAPI.shared.resource(initial.id) } }
      .sheet(isPresented: $editing, onDismiss: { run { updated = try await SkinCommunityAPI.shared.resource(initial.id) } }) {
        CommunityResourceEditor(kind: item.kind, existing: item)
      }
      .confirmationDialog("确认导入预览中的 \(item.content.entries?.count ?? 0) 条词条？", isPresented: $confirmImport, titleVisibility: .visible) {
        Button("确认导入") { let selected = item; run {
          try PersonalDictionaryStore().enqueueImport((selected.content.entries ?? []).map { try $0.localWord() })
          message = "已加入本机导入队列，打开水杉键盘后处理。可在「查看导入状态」检查每条结果。"
        }}
      }
      .confirmationDialog("下架后其他用户将无法获取此作品，已有本地副本保留。", isPresented: $confirmDelete, titleVisibility: .visible) {
        Button("下架", role: .destructive) { run { try await SkinCommunityAPI.shared.unpublishResource(item.id); dismiss() } }
      }
      .alert("社区", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
        Button("好", role: .cancel) {}
      } message: { Text(message ?? "") }
  }
  private func run(_ action: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }; busy = true
    Task { defer { busy = false }; do { try await action() } catch { message = error.localizedDescription } }
  }
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
  @Environment(\.dismiss) private var dismiss
  var body: some View {
    NavigationView {
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
            Picker("类型", selection: $wordKind) { ForEach(PersonalWordKind.allCases) { Text($0.title).tag($0) } }
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
          busy = true
          Task {
            defer { busy = false }
            do {
              let url = try result.get()
              let imported = try await Task.detached { try PersonalDictionaryImport.read(from: url) }.value
              let ids = Set(words.map(\.id)); let additions = imported.entries.filter { !ids.contains($0.id) }
              guard words.count + additions.count <= 128 else { throw CommunityFailure(message: "每份社区词库最多 128 条，请先精简文件。") }
              words += additions
            } catch { message = error.localizedDescription }
          }
        }
        .confirmationDialog("确认公开发布「\(name)」？", isPresented: $confirming, titleVisibility: .visible) {
          Button("公开发布") {
            busy = true
            Task {
              defer { busy = false }
              do {
                let content = CommunityResourceContent(entries: kind == .dictionary ? words.map(CommunityWord.init) : nil,
                  prompt: kind == .reply ? prompt : nil)
                try await SkinCommunityAPI.shared.publishResource(id: publicationID, kind: kind, name: name,
                  description: description, content: content, revision: existing?.revision ?? 0)
                onPublished(); dismiss()
              } catch { message = error.localizedDescription }
            }
          }
        }
        .alert("发布作品", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
          Button("好", role: .cancel) {}
        } message: { Text(message ?? "") }
    }.navigationViewStyle(.stack)
  }
}

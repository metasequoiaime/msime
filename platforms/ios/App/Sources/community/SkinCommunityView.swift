import SwiftUI

struct SkinCommunityView: View {
  var onlyMine = false
  var embedded = false
  private var visibleSkins: [CommunitySkin] { onlyMine ? skins.filter(\.owned) : skins }
  @State private var skins: [CommunitySkin] = []
  @State private var more = false
  @State private var busy = false
  @State private var signedIn = false
  @State private var message: String?
  @State private var search = ""
  /// `nil` 表示「全部」。
  @State private var category: CommunitySkinCategory?
  @State private var showPublish = false
  @State private var showAccount = false
  @State private var requestID = UUID()
  /// 本地皮肤库（12 个槽位），每次画廊显示时重新读取，这样在详情页领取的皮肤在卡片上会显示 使用。
  @State private var library: [SavedKeyboardSkin] = []
  /// 正在下载 获取 的 id。
  @State private var taking: Set<String> = []
  // 两个键都只用来重绘胶囊按钮：主题决定键盘上是否显示自定义设计，设计决定显示哪一个。
  @AppStorage(GlobalThemePreference.key, store: KeyboardFeedbackPreference.defaults) private var globalTheme = GlobalThemeCatalog.systemId
  @AppStorage(CustomKeyboardSkinStore.key, store: KeyboardFeedbackPreference.defaults) private var storedDesign: Data?
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  private let api = SkinCommunityAPI.shared

  /// 键盘当前绘制的设计；键盘绘制内置主题时为 nil。
  private var activeDesign: CustomKeyboardSkin? {
    guard globalTheme == GlobalThemeCatalog.customId, storedDesign != nil else { return nil }
    return CustomKeyboardSkinStore.stored
  }

  private var gallery: some View {
    let regular = horizontalSizeClass == .regular
    let columns = Array(repeating: GridItem(.flexible(), spacing: 12, alignment: .top), count: regular ? 3 : 2)
    let active = activeDesign
    return ScrollView {
      VStack(alignment: .leading, spacing: 16) {
        CommunitySearchField(text: $search, placeholder: "搜索皮肤设计") { run { try await load() } }
        categoryChips
        if visibleSkins.isEmpty && !busy {
          Text(onlyMine ? (more ? "当前页没有你的作品，继续加载查看更多。" : "还没有已发布的作品，分享你的第一款设计吧。")
               : (category == nil ? "暂时没有皮肤，发布你的第一款设计吧。" : "这个分类下暂时没有皮肤。"))
            .foregroundStyle(MetasequoiaTheme.sub).frame(maxWidth: .infinity).padding(.vertical, 40)
        }
        LazyVGrid(columns: columns, spacing: 12) {
          ForEach(visibleSkins) { skin in
            CommunitySkinCard(skin: skin, state: state(of: skin, active: active)) { pill(skin) }
          }
        }
        if more { Button("加载更多") { run { try await load(append: true) } }.disabled(busy).frame(maxWidth: .infinity) }
        if busy { ProgressView().frame(maxWidth: .infinity) }
      }.communityPageContent(regular: regular)
    }
    .background(MetasequoiaTheme.canvas)
    .onAppear { library = CustomSkinLibrary.designs }
  }

  private func state(of skin: CommunitySkin, active: CustomKeyboardSkin?) -> CommunitySkinCard.PillState {
    if taking.contains(skin.id) { return .taking }
    guard let saved = CommunitySkinInstall.saved(skin, in: library) else { return .available }
    return saved.design == active ? .inUse : .taken
  }

  /// 获取 把皮肤下载到本地皮肤库；使用 把皮肤库里的副本放到键盘上；使用中 不做任何事。
  private func pill(_ skin: CommunitySkin) {
    if let saved = CommunitySkinInstall.saved(skin, in: library) {
      guard saved.design != activeDesign else { return }
      if CommunitySkinInstall.apply(saved.design) {
        TypingStatisticsExtras.recordSkin(saved.id.uuidString)
        ToastCenter.shared.show("已换上「\(skin.name)」")
      }
      else { message = "没能换上「\(skin.name)」，请稍后重试。" }
      return
    }
    guard !taking.contains(skin.id) else { return }
    taking.insert(skin.id)
    Task { @MainActor in
      defer { taking.remove(skin.id) }
      do {
        _ = try await CommunitySkinInstall.take(skin)
        library = CustomSkinLibrary.designs
        ToastCenter.shared.show("已获取「\(skin.name)」，点「使用」换上")
      } catch { message = error.localizedDescription }
    }
  }
  /// 横向滚动的分类筛选，「全部」在最前。切换后从第一页重新读取；加载中禁用，避免切换被 `run` 的忙碌保护吞掉。
  private var categoryChips: some View {
    ScrollView(.horizontal, showsIndicators: false) {
      HStack(spacing: 8) {
        categoryChip(nil, title: "全部")
        ForEach(CommunitySkinCategory.allCases) { categoryChip($0, title: $0.label) }
      }.padding(.horizontal, 1)
    }.disabled(busy)
  }
  private func categoryChip(_ value: CommunitySkinCategory?, title: String) -> some View {
    let selected = category == value
    return Button {
      guard !selected else { return }
      let previous = category
      category = value
      run {
        do { try await load() }
        catch {
          category = CommunitySkinCategorySelectionPolicy.afterFailedLoad(previous: previous)
          throw error
        }
      }
    } label: {
      Text(title).font(.system(size: 13, weight: selected ? .semibold : .regular))
        .foregroundStyle(selected ? MetasequoiaTheme.accent : Color.secondary)
        .padding(.horizontal, 13).frame(height: 32)
        .background(selected ? MetasequoiaTheme.accentSoft : MetasequoiaTheme.surface, in: Capsule())
        .contentShape(Capsule())
    }.buttonStyle(.plain).accessibilityIdentifier("communitySkinCategory-\(value?.rawValue ?? "all")")
      .accessibilityAddTraits(selected ? [.isSelected] : [])
  }
  var body: some View {
    gallery
    .toolbar {
      ToolbarItem(placement: .navigationBarTrailing) { if !embedded {
        Button { if signedIn { showPublish = true } else { showAccount = true } } label: {
          Label("发布", systemImage: "plus")
        }.accessibilityLabel("发布我的设计").accessibilityIdentifier("publishCommunitySkin")
      }
      }
    }
    .navigationTitle(onlyMine ? "我发布的皮肤" : (embedded ? "社区" : "皮肤社区"))
    // Embedded is the 社区 tab's own list, which carries the tab's large title.
    .navigationBarTitleDisplayMode(embedded ? .large : .inline)
    .refreshable { do { try await load() } catch { message = error.localizedDescription } }
    .task {
      signedIn = (try? await api.signedIn()) ?? false
      run { try await load() }
    }
    .onChange(of: signedIn) { _ in
      Task { do { try await load() } catch { message = error.localizedDescription } }
    }
    .sheet(isPresented: $showPublish) { CommunityPublishView { run { try await load() } } }
    .sheet(isPresented: $showAccount, onDismiss: {
      Task {
        signedIn = (try? await api.signedIn()) ?? false
        if signedIn { showPublish = true }
      }
    }) { AccountLoginSheet() }
    .alert("皮肤社区", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }
  @MainActor private func load(append: Bool = false) async throws {
    let id = UUID()
    requestID = id
    let page: CommunityPage
    do { page = try await api.list(offset: append ? skins.count : 0, search: search, mine: onlyMine, category: category) }
    catch {
      guard requestID == id else { return }
      throw error
    }
    guard requestID == id else { return }
    if append { let ids = Set(skins.map(\.id)); skins += page.skins.filter { !ids.contains($0.id) } }
    else { skins = page.skins }
    more = page.has_more
  }
  private func run(_ action: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }; busy = true
    Task { defer { busy = false }; do { try await action() } catch { message = error.localizedDescription } }
  }
}

private struct CommunitySkinCard: View {
  /// 胶囊按钮提供的操作，对应 Android 的 `CommunityAdapter.Action`：皮肤还不在本地皮肤库时为 获取，下载期间为 获取中，下载完成后为 使用，键盘正在绘制它时为 使用中。
  enum PillState {
    case available, taking, taken, inUse
    var title: String {
      switch self {
      case .available: "获取"
      case .taking: "获取中"
      case .taken: "使用"
      case .inUse: "使用中"
      }
    }
  }
  let skin: CommunitySkin
  let state: PillState
  let action: () -> Void

  private var pillLabel: some View {
    CommunityPillLabel(title: state.title, offersAction: state != .inUse).opacity(state == .taking ? 0.6 : 1)
  }

  var body: some View {
    let shape = RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous)
    NavigationLink { CommunitySkinDetail(initial: skin) } label: {
      VStack(spacing: 0) {
        CommunityDesignPreview(design: skin.design, style: .card)
          .padding([.top, .horizontal], 8)
        HStack(alignment: .center, spacing: 8) {
          VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
              Text(skin.name).font(.system(size: 14, weight: .semibold)).foregroundStyle(.primary).lineLimit(1)
              if skin.removed { CommunityRemovedBadge() }
            }
            Text(skin.owned ? "我的作品" : skin.author).font(.system(size: 11)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
            Text(CommunityListingText.uses(skin.downloads)).font(.system(size: 11)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
          }
          Spacer(minLength: 0)
          // 在行内占住胶囊按钮的位置；胶囊本身从链接外部覆盖绘制在上面，这样它对点击和 VoiceOver 来说仍是独立的按钮。
          pillLabel.hidden().accessibilityHidden(true)
            .anchorPreference(key: CommunityPillBoundsKey.self, value: .bounds) { $0 }
        }
        .padding(EdgeInsets(top: 10, leading: 10, bottom: 12, trailing: 10))
      }
      .frame(maxWidth: .infinity)
      .background(MetasequoiaTheme.surface, in: shape)
      .contentShape(shape)
    }
    .buttonStyle(.plain)
    .accessibilityIdentifier("communitySkinCard-\(skin.id)")
    .overlayPreferenceValue(CommunityPillBoundsKey.self) { anchor in
      GeometryReader { proxy in
        if let anchor {
          let rect = proxy[anchor]
          pill.frame(width: rect.width, height: rect.height).position(x: rect.midX, y: rect.midY)
        }
      }
    }
  }

  @ViewBuilder private var pill: some View {
    if state == .inUse {
      pillLabel.accessibilityLabel("使用中，\(skin.name)")
    } else {
      Button(action: action) { pillLabel }
        .buttonStyle(.plain)
        .accessibilityLabel("\(state.title)，\(skin.name)")
        .accessibilityIdentifier("communitySkinAction-\(skin.id)")
    }
  }
}

private struct CommunityPillBoundsKey: PreferenceKey {
  static let defaultValue: Anchor<CGRect>? = nil
  static func reduce(value: inout Anchor<CGRect>?, nextValue: () -> Anchor<CGRect>?) { value = value ?? nextValue() }
}

/// 把社区皮肤领取到本地皮肤库、把皮肤库副本放到键盘上，由画廊的胶囊按钮和详情页共用。
enum CommunitySkinInstall {
  /// 社区皮肤在皮肤库里的副本：它占用以皮肤自身 id 命名的槽位。
  static func saved(_ skin: CommunitySkin, in library: [SavedKeyboardSkin]) -> SavedKeyboardSkin? {
    guard let id = UUID(uuidString: skin.id) else { return nil }
    return library.first { $0.id == id }
  }

  /// 下载设计（会计入下载次数）并保存到皮肤库，替换同一皮肤之前的副本。皮肤库已满时抛出 本地皮肤已满 提示，而不是丢弃设计。
  @MainActor static func take(_ skin: CommunitySkin) async throws -> CustomKeyboardSkin {
    let design = try await SkinCommunityAPI.shared.download(skin.id)
    var library = CustomSkinLibrary.designs
    let id = UUID(uuidString: skin.id) ?? UUID()
    if let index = library.firstIndex(where: { $0.id == id }) { library[index].design = design }
    else {
      guard library.count < 12 else { throw CommunityFailure(message: "本地皮肤已满，请在「我的设计」删除一款后重试。") }
      library.append(SavedKeyboardSkin(id: id, name: skin.name, design: design))
    }
    guard CustomSkinLibrary.save(library) else { throw CommunityFailure(message: "无法保存皮肤，请检查设备存储。") }
    return design
  }

  /// 按编辑器 使用 的方式把设计放到键盘上：先写 App Group 副本，再选中共享文档里的自定义主题。文档拒绝这次修改时返回 false。
  static func apply(_ design: CustomKeyboardSkin) -> Bool {
    CustomKeyboardSkinStore.save(design)
    return GlobalThemePreference.apply(design)
  }
}

private struct CommunitySkinCategoryTag: View {
  let category: CommunitySkinCategory
  var body: some View {
    Text(category.label).font(.system(size: 10, weight: .medium)).lineLimit(1).fixedSize()
      .foregroundStyle(MetasequoiaTheme.accent)
      .padding(.horizontal, 6).padding(.vertical, 2)
      .background(MetasequoiaTheme.accentSoft, in: Capsule())
      .accessibilityLabel("分类：\(category.label)")
  }
}

struct CommunitySkinDetail: View {
  let initial: CommunitySkin
  @State private var updated: CommunitySkin?
  @State private var busy = false
  @State private var message: String?
  @State private var confirmsRemoval = false
  @State private var trial: KeyboardSkinTrial?
  @Environment(\.dismiss) private var dismiss
  private var skin: CommunitySkin { updated ?? initial }
  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 16) {
        CommunityDesignPreview(design: skin.design)
        HStack {
          Text(skin.name).font(.title2.bold())
          if skin.removed { CommunityRemovedBadge() }
        }
        HStack(spacing: 8) {
          Text(skin.author).foregroundStyle(.secondary)
          if let category = skin.category { CommunitySkinCategoryTag(category: category) }
        }
        Text(skin.description)
        Text("\(skin.downloads) 人下载 · \(skin.rating_average, specifier: "%.1f") 分 · \(skin.rating_count) 人评分")
          .font(.subheadline).foregroundStyle(.secondary)
        Button { run {
          let design = try await CommunitySkinInstall.take(skin)
          trial = try KeyboardSkinTrialStore().begin(name: skin.name, design: design)
          updated = try? await SkinCommunityAPI.shared.detail(skin.id)
        } } label: { Label("下载并试用", systemImage: "arrow.down.circle.fill").frame(maxWidth: .infinity) }
          .buttonStyle(.borderedProminent).disabled(busy).accessibilityIdentifier("downloadCommunitySkin")
        if !skin.owned {
          Text("我的评分（下载后可评，可重新选择）").font(.subheadline)
          HStack {
            ForEach(1...5, id: \.self) { stars in
              Button { run {
                try await SkinCommunityAPI.shared.rate(skin.id, stars: stars)
                updated = try await SkinCommunityAPI.shared.detail(skin.id)
              } } label: { Image(systemName: stars <= skin.my_rating ? "star.fill" : "star").frame(width: 44, height: 44) }
                .accessibilityLabel("评 \(stars) 星").disabled(busy)
            }
          }
          CommunityReportButton(kind: "skins", itemID: skin.id)
        } else {
          Menu {
            ForEach(CommunitySkinCategory.allCases) { category in
              Button { run {
                var changed = try await SkinCommunityAPI.shared.setCategory(skin.id, category: category)
                changed.moderation = changed.moderation ?? skin.moderation
                updated = changed
              } } label: {
                if category == (skin.category ?? .other) { Label(category.label, systemImage: "checkmark") }
                else { Text(category.label) }
              }
            }
          } label: {
            Label("修改分类：\((skin.category ?? .other).label)", systemImage: "tag")
          }.disabled(busy).accessibilityIdentifier("changeCommunitySkinCategory")
          Button("下架这款皮肤", role: .destructive) { confirmsRemoval = true }.disabled(busy)
        }
        if busy { ProgressView() }
      }.padding()
    }.navigationTitle("皮肤详情").navigationBarTitleDisplayMode(.inline)
      .task { run { updated = try await SkinCommunityAPI.shared.detail(initial.id) } }
      .sheet(item: $trial, onDismiss: {
        do { try KeyboardSkinTrialStore().restorePending() } catch { message = error.localizedDescription }
      }) { CommunitySkinTrialView(trial: $0) }
      .confirmationDialog("下架后其他用户无法再下载，已下载的本地皮肤会保留。", isPresented: $confirmsRemoval, titleVisibility: .visible) {
        Button("下架", role: .destructive) { run { try await SkinCommunityAPI.shared.unpublish(skin.id); dismiss() } }
      }
      .alert("皮肤社区", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) { Button("好", role: .cancel) {} } message: { Text(message ?? "") }
  }
  private func run(_ action: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }; busy = true
    Task { defer { busy = false }; do { try await action() } catch { message = error.localizedDescription } }
  }
}

struct CommunityPublishView: View {
  var onPublished: () -> Void
  var selectedSkinID: UUID? = nil
  @Environment(\.dismiss) private var dismiss
  @State private var library = CustomSkinLibrary.designs
  @State private var selected = UUID()
  @State private var publicationID = UUID().uuidString.lowercased()
  @State private var name = ""
  @State private var description = ""
  @State private var category: CommunitySkinCategory = .other
  @State private var agrees = false
  @State private var busy = false
  @State private var message: String?
  private var design: CustomKeyboardSkin? { library.first { $0.id == selected }?.design }

  /// The library is stored in an App Group JSON file, so returning from the editor needs an explicit refresh.
  private func refreshLibrary(preferring preferred: UUID? = nil) {
    library = CustomSkinLibrary.designs
    guard library.first(where: { $0.id == selected }) == nil else { return }
    if let first = library.first(where: { $0.id == preferred }) ?? library.first {
      selected = first.id
      name = first.name
      publicationID = UUID().uuidString.lowercased()
    }
  }
  var body: some View {
    NavigationView {
      Form {
        Section {
          if library.isEmpty {
            Text("还没有保存的设计。先设计一款并命名保存，回到这里就能发布它。")
              .foregroundStyle(.secondary)
          } else {
            Picker("我的皮肤", selection: $selected) { ForEach(library) { Text($0.name).tag($0.id) } }
              .onChange(of: selected) { id in name = library.first { $0.id == id }?.name ?? ""; publicationID = UUID().uuidString.lowercased() }
            if let design { CommunityDesignPreview(design: design) }
          }
          if selectedSkinID == nil {
            NavigationLink {
              CustomSkinEditorView(publishable: false).onDisappear { refreshLibrary() }
            } label: {
              SettingsRowLabel(title: library.isEmpty ? "去设计一款" : "继续编辑我的皮肤",
                               detail: "在编辑器里调好，到「我的」命名保存",
                               symbol: "paintbrush.pointed.fill")
            }
            .accessibilityIdentifier("designSkinFromPublish")
          }
        } header: {
          Text("选择已保存的设计")
        }
        Section("发布信息") {
          TextField("皮肤名称（最多 32 字）", text: $name).onChange(of: name) { name = String($0.prefix(32)); publicationID = UUID().uuidString.lowercased() }
          TextField("设计说明（最多 280 字）", text: $description).onChange(of: description) { description = String($0.prefix(280)); publicationID = UUID().uuidString.lowercased() }
          // 分类不计入请求摘要，修改分类不需要换新的发布 id。
          Picker("分类", selection: $category) {
            ForEach(CommunitySkinCategory.allCases) { Text($0.label).tag($0) }
          }.accessibilityIdentifier("communitySkinPublishCategory")
          Toggle("我拥有发布所用素材的权利，并同意其他用户免费下载使用", isOn: $agrees)
          Text("发布后，设计及照片壁纸将上传并公开。请勿包含私人照片或敏感信息。作者可随时下架。").font(.caption).foregroundStyle(.secondary)
        }
        Button("发布到社区") {
          guard let design else { return }; busy = true
          Task {
            defer { busy = false }
            do {
              try await SkinCommunityAPI.shared.publish(id: publicationID, name: name, description: description, design: design, category: category)
              onPublished(); dismiss()
            } catch { message = error.localizedDescription }
          }
        }.disabled(busy || !agrees || design == nil || name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
          .accessibilityIdentifier("confirmCommunitySkinPublication")
        if busy { ProgressView() }
      }.disabled(busy)
        .navigationTitle("发布皮肤")
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() }.disabled(busy) } }
        .onAppear { refreshLibrary(preferring: selectedSkinID) }
        .alert("发布失败", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) { Button("好", role: .cancel) {} } message: { Text(message ?? "") }
    }.interactiveDismissDisabled(busy)
  }
}

// A value-based preview must not change the user's active custom skin while browsing.
struct CommunityDesignPreview: View {
  /// `.detail` 是带候选行的大预览，用于详情页、发布表单和编辑器的模板。`.card` 是画廊卡片上设计稿的 MiniKb：空闲工具栏、小写按键和完整的底行，按 390 x 292 绘制后缩放适配。
  enum Style { case detail, card }
  let design: CustomKeyboardSkin
  var nineKey = false
  var style: Style = .detail
  private func color(_ rgb: UInt32) -> Color { Color(uiColor: CustomKeyboardSkin.color(rgb)) }
  var body: some View {
    switch style {
    case .detail:
      KeyboardPreviewCanvas { keyboard }.accessibilityHidden(true)
    case .card:
      KeyboardPreviewCanvas(referenceHeight: Self.cardHeight) { miniKeyboard }
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .accessibilityHidden(true)
    }
  }
  private var keyboard: some View {
    VStack(spacing: 6) {
      HStack { Text("你好"); Text("你号"); Spacer(); Text(nineKey ? "九键" : "全拼") }.font(.caption).foregroundStyle(color(design.accent)).frame(height: 32)
      if nineKey {
        HStack(spacing: 4) {
          VStack(spacing: 4) { key("，"); key("。"); key("？"); key("！") }.frame(width: 32)
          VStack(spacing: 4) {
            ForEach([["分词", "ABC", "DEF"], ["GHI", "JKL", "MNO"], ["PQRS", "TUV", "WXYZ"]], id: \.self) { row in
              HStack(spacing: 4) { ForEach(row, id: \.self) { key($0) } }
            }
          }
          VStack(spacing: 4) { key("⌫"); key("."); key("0") }.frame(width: 36)
        }.frame(maxHeight: .infinity)
      } else {
        ForEach(["QWERTYUIOP", "ASDFGHJKL", "⇧ZXCVBNM⌫"], id: \.self) { row in
          HStack(spacing: 3) { ForEach(Array(row).map(String.init), id: \.self) { key($0) } }
        }
      }
      HStack(spacing: 3) { key("123"); key("，"); key("空格").frame(minWidth: 100); key("↵") }.frame(height: 44)
    }.padding(10).background {
      KeyboardSkinBackdrop(skin: .designed(design))
    }.clipShape(RoundedRectangle(cornerRadius: 12)).accessibilityHidden(true)
  }
  private func key(_ text: String) -> some View {
    Text(text).font(.system(size: 14, weight: .medium, design: design.monospaced ? .monospaced : .default))
      .foregroundStyle(color(text == "↵" ? CustomKeyboardSkin.readableText(on: design.actionBackground) : design.keyForeground)).frame(maxWidth: .infinity, maxHeight: .infinity)
      .background { SkinKeySurface(design: design, action: text == "↵") }
  }

  // ---- 卡片样式（设计稿的 MiniKb） ----

  private static let cardWidth: CGFloat = 390
  private static let cardHeight: CGFloat = 292
  /// 按键横跨整张卡片，两侧各留 3pt。
  private static let rowWidth: CGFloat = cardWidth - 6
  private static let keyGap: CGFloat = 6
  private static let digitHints: [Character: String] = ["q": "1", "w": "2", "e": "3", "r": "4", "t": "5", "y": "6", "u": "7", "i": "8", "o": "9", "p": "0"]

  private struct MiniKey {
    enum Role { case letter, function, space, action }
    var text: String?
    var symbol: String?
    var weight: CGFloat = 1
    var role: Role = .letter
    var hint: String?
  }

  private var fontDesign: Font.Design { design.monospaced ? .monospaced : .default }
  private var foreground: Color { color(design.keyForeground) }
  /// 按键提示和空格键标签，与键盘对设计使用的 `secondary` 一致：按键颜色的 60%。
  private var secondary: Color { foreground.opacity(0.6) }

  private var miniKeyboard: some View {
    VStack(spacing: 8) {
      miniToolbar.frame(height: 50)
      VStack(spacing: 11) {
        miniRow(Array("qwertyuiop").map { MiniKey(text: String($0), hint: Self.digitHints[$0]) }, width: Self.rowWidth)
        miniRow(Array("asdfghjkl").map { MiniKey(text: String($0)) }, width: Self.rowWidth * 0.9)
        miniRow([MiniKey(symbol: "shift", weight: 1.4, role: .function)]
                + Array("zxcvbnm").map { MiniKey(text: String($0)) }
                + [MiniKey(symbol: "delete.left", weight: 1.4, role: .function)], width: Self.rowWidth)
        miniRow([MiniKey(text: "123", weight: 1.25, role: .function), MiniKey(text: "中", weight: 1.05, role: .function),
                 MiniKey(text: "，"), MiniKey(text: "全拼", symbol: "mic", weight: 4, role: .space),
                 MiniKey(text: "。"), MiniKey(symbol: "return", weight: 1.9, role: .action)], width: Self.rowWidth)
      }
      Capsule().fill(foreground.opacity(0.85)).frame(width: 134, height: 5).frame(maxHeight: .infinity)
    }
    .padding(.horizontal, 3)
    .frame(width: Self.cardWidth, height: Self.cardHeight)
    .background { KeyboardSkinBackdrop(skin: .designed(design)) }
  }

  /// 用皮肤配色绘制的空闲工具栏：品牌圆片、五个默认工具和收起箭头，各占等宽的一列。
  private var miniToolbar: some View {
    HStack(spacing: 0) {
      brandDisc.frame(maxWidth: .infinity)
      ForEach(["face.smiling", "bubble.left", "doc.on.clipboard", "paintpalette", "keyboard"], id: \.self) { symbol in
        Image(systemName: symbol).font(.system(size: 19)).frame(maxWidth: .infinity)
      }
      Image(systemName: "chevron.down").font(.system(size: 17, weight: .medium)).frame(maxWidth: .infinity)
    }
    .foregroundStyle(foreground)
  }

  /// 由设计的强调色绘制的键盘品牌圆片：圆片为 mix(accent 14%, 按键颜色)，标志的边框为 mix(accent 82%, #000)，再加上白色描边。
  private var brandDisc: some View {
    let accent = CustomKeyboardSkin.color(design.accent)
    let disc = AppThemePalette.mix(accent, 14, CustomKeyboardSkin.color(design.keyBackground))
    let frame = AppThemePalette.mix(accent, 82, .black)
    let markSize: CGFloat = 16
    let scale = MSIMELogo.scale(for: CGRect(x: 0, y: 0, width: markSize, height: markSize))
    return ZStack {
      Circle().fill(Color(uiColor: disc))
      ZStack {
        MSIMELogoFrame().fill(Color(uiColor: frame))
        MSIMELogoStroke().stroke(.white, style: StrokeStyle(lineWidth: MSIMELogo.strokeWidth * scale, lineCap: .round, lineJoin: .round))
      }
      .frame(width: markSize, height: markSize)
    }
    .frame(width: 26, height: 26)
  }

  /// 一行按键，宽度按各自权重分配，与 MiniKb 的弹性行一致。
  private func miniRow(_ keys: [MiniKey], width: CGFloat) -> some View {
    let unit = (width - Self.keyGap * CGFloat(keys.count - 1)) / keys.reduce(0) { $0 + $1.weight }
    return HStack(spacing: Self.keyGap) {
      ForEach(keys.indices, id: \.self) { index in
        miniKey(keys[index]).frame(width: unit * keys[index].weight)
      }
    }
    .frame(height: 43)
  }

  private func miniKey(_ key: MiniKey) -> some View {
    let action = key.role == .action
    let tint = action ? color(CustomKeyboardSkin.readableText(on: design.actionBackground)) : key.role == .space ? secondary : foreground
    return ZStack(alignment: .topTrailing) {
      SkinKeySurface(design: design, action: action)
      HStack(spacing: 4) {
        if let symbol = key.symbol {
          Image(systemName: symbol).font(.system(size: key.role == .space ? 16 : 19))
        }
        if let text = key.text {
          Text(text).font(.system(size: key.role == .letter ? 22 : key.role == .space ? 13 : 15,
                                  weight: key.role == .function ? .medium : .regular, design: fontDesign))
        }
      }
      .lineLimit(1)
      .foregroundStyle(tint)
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      if let hint = key.hint {
        Text(hint).font(.system(size: 10, design: fontDesign)).foregroundStyle(secondary)
          .padding(.top, 3).padding(.trailing, 5)
      }
    }
  }
}

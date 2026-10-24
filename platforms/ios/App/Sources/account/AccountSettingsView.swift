import SwiftUI

/// 「我的」页，对应 Android 的 `AccountFragment`：先是资料卡，然后是「我的内容」「同步」「通用」三组，以及一组不带标题的下载、帮助、关于和两个重播入口，铺在季节页面底色上。
///
/// 页面先读本地会话并立即绘制，再去取需要联网的内容（资料和云剪贴板条数）；没拿到数字的行就不显示数字。「云同步」和 Android 一样是一个开关，副标题是 Rust 给的同步状态行；手动上传下载设置、管理云词库在它下面的「手动同步」里。
struct AccountSettingsView: View {
  @EnvironmentObject private var router: SettingsRouter
  @Environment(\.horizontalSizeClass) private var widthClass
  @AppStorage(GlobalThemePreference.key, store: KeyboardFeedbackPreference.defaults)
  private var skin = GlobalThemeCatalog.systemId
  @State private var signedIn = false
  @State private var user: CommunityUser?
  @State private var profile: CommunityProfile?
  @State private var needsRecovery = false
  @State private var busy = false
  @State private var message: String?
  /// 「云剪贴板」行尾的值：「N 条」或「已关闭」，条数到达之前为 `nil`。
  @State private var clipboardValue: String?
  @State private var showsLogin = false
  @State private var showsAppTheme = false
  @State private var replay: Replay?
  @State private var pending: Task<Void, Never>?
  @State private var operation: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()
  @State private var syncTask: Task<Void, Never>?
  @State private var syncGeneration = OperationGeneration()
  @ObservedObject private var cloudSync = CloudSync.shared
  private let api = SkinCommunityAPI.shared

  /// What 我的 can play again. Both cover the whole window, so neither slides up as a sheet over the tab bar.
  private enum Replay: String, Identifiable {
    case onboarding, splash
    var id: Self { self }
  }

  private var isRegular: Bool { widthClass == .regular }

  private var version: String {
    Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "开发构建"
  }

  private var displayName: String { user?.preferredDisplayName ?? "水杉用户" }

  /// 账号带着邮箱（Rust 只交出 Google 身份的验证邮箱）时显示邮箱地址，否则显示账号的登录方式。资料到达之前只说明账号已登录。
  private var signedInSubtitle: String {
    guard let profile else { return "已登录" }
    if let email = profile.user.email, !email.isEmpty { return email }
    let providers = Set(profile.providers)
    guard let provider = ["email", "apple", "phone", "sms", "google"].first(where: providers.contains),
          let name = profile.providerNames[provider] else { return "已登录" }
    let separator = name.unicodeScalars.allSatisfy(\.isASCII) ? " " : ""
    return "通过\(separator)\(name)\(separator)登录"
  }

  /// 键盘皮肤在目录里的标题；目录里没有的 id 返回 `nil`。
  private var skinValue: String? {
    GlobalThemeCatalog.contains(skin) ? GlobalThemeCatalog.title(skin) : nil
  }

  private var appThemeCatalog: [AppThemePalette.CatalogEntry] { AppThemePalette.catalog() }

  /// 水杉四季跟随月份时显示「四季 · 秋杉」这样的标题，否则显示固定主题的标题。实际绘制的主题从解析结果读取，存储的 id 已经解析不到时会回落到水杉四季。
  private var appThemeValue: String? {
    let resolved = AppThemePalette.resolved(dark: false)
    let id = resolved?.id ?? AppThemePalette.selectedID
    let entry = appThemeCatalog.first { $0.id == id }
    if entry?.seasonal ?? (id == AppThemePalette.defaultID), let season = resolved?.season {
      return "四季 · " + AppThemePalette.seasonTitle(season)
    }
    return entry?.title
  }

  private var appThemeOptions: [DesignOption<String>] {
    appThemeCatalog.map { entry in
      DesignOption(title: entry.seasonal ? entry.title + "（自动）" : entry.title, value: entry.id,
                   identifier: "appTheme_\(entry.id)")
    }
  }

  /// 路由推入的页面；「应用主题」是浮层，所以不会成为导航目标。
  private var pushedRoute: Binding<SettingsRouter.AccountRoute?> {
    Binding(get: { router.accountRoute == .appTheme ? nil : router.accountRoute },
            set: { router.accountRoute = $0 })
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 18) {
        profileSection
        contentGroup
        syncGroup
        generalGroup
        moreGroup
      }
      .padding(.top, 4)
      .padding(.horizontal, isRegular ? 28 : 16)
      .padding(.bottom, 24)
      .frame(maxWidth: isRegular ? 760 : .infinity)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .navigationTitle("我的")
    .navigationBarTitleDisplayMode(.large)
    .tint(MetasequoiaTheme.accent)
    .task { request.appear(); startReload() }
    .sheet(isPresented: $showsLogin, onDismiss: { startReload() }) { AccountLoginSheet() }
    .designOptionSheet(isPresented: $showsAppTheme, title: "应用主题", message: "四季会随季节自动更换配色",
                       options: appThemeOptions, selected: AppThemePalette.selectedID) { id in
      AppThemePalette.select(id)
    }
    .navigationDestination(item: pushedRoute) { route in
      switch route {
      case .feedback: FeedbackView()
      case .about: AboutView()
      case .appTheme: EmptyView()
      }
    }
    .onChange(of: router.accountRoute, initial: true) { _, route in
      guard route == .appTheme else { return }
      router.accountRoute = nil
      showsAppTheme = true
    }
    .alert("账号与登录", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
    .fullScreenCover(item: $replay) { replay in
      switch replay {
      case .onboarding:
        if UIDevice.current.userInterfaceIdiom == .pad && widthClass == .regular {
          OnboardingModalCard { dismissReplay() }.presentationBackground(.clear)
        } else {
          WelcomeFlowView(onFinish: { dismissReplay() })
        }
      case .splash:
        SplashView { dismissReplay() }
      }
    }
    .onDisappear {
      ViewRequestTask.disappear(request: $request, pending: $pending)
      ViewRequestTask.cancel(generation: $syncGeneration, task: $syncTask)
      busy = false
    }
  }

  // MARK: - 资料卡

  @ViewBuilder private var profileSection: some View {
    VStack(spacing: 8) {
      // 资料页是推进去的二级页面,不是浮层:它要承载退出登录这类收尾操作,而这些操作做完之后回到的是一个已经变了的「我的」页 —— 浮层盖在旧内容上关掉的那一下,看起来就像什么都没发生。
      if signedIn {
        NavigationLink {
          AccountProfileEditor(initialUser: user, onSaved: { user = $0 }, onSignedOut: signedOut)
        } label: {
          profileCard
        }
        .buttonStyle(AccountCardButtonStyle())
        .accessibilityLabel("\(displayName)，个人资料")
        .accessibilityIdentifier("accountProfileCard")
      } else {
        Button { showsLogin = true } label: { profileCard }
          .buttonStyle(AccountCardButtonStyle())
          .accessibilityLabel("未登录，点按登录")
          .accessibilityIdentifier("accountProfileCard")
      }
      if needsRecovery && !signedIn {
        Button("清除失效登录状态") {
          run { id in
            try await api.clearExpiredLogin()
            try request.checkCurrent(id)
            needsRecovery = false
            await reload(id: id)
          }
        }
        .font(.system(size: 13))
        .disabled(busy)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 4)
        .accessibilityIdentifier("accountClearExpiredLogin")
      }
    }
  }

  /// 设计稿里的账号卡：56pt 圆形头像（已登录时是强调色底上的名字首字，未登录时是分段控件灰底上的问号），旁边是 18pt 粗体名字、一行 13pt 状态文字和行尾的箭头。
  private var profileCard: some View {
    HStack(spacing: 14) {
      Text(signedIn ? String(displayName.prefix(1)) : "?")
        .font(.system(size: 22, weight: .bold))
        .foregroundStyle(signedIn ? MetasequoiaTheme.onAccent : MetasequoiaTheme.sub)
        .frame(width: 56, height: 56)
        .background(signedIn ? MetasequoiaTheme.accent : MetasequoiaTheme.segBg, in: Circle())
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 3) {
        Text(signedIn ? displayName : "未登录")
          .font(.system(size: 18, weight: .bold)).foregroundStyle(.primary)
          .lineLimit(1)
        Text(signedIn ? signedInSubtitle : "登录后同步词库、皮肤和设置")
          .designSubtext()
          .lineLimit(1)
      }
      Spacer(minLength: 8)
      DesignChevron(fontSize: 20)
    }
    .padding(16)
    .frame(maxWidth: .infinity, alignment: .leading)
    .contentShape(Rectangle())
  }

  // MARK: - 分组

  private var contentGroup: some View {
    MeGroup(title: "我的内容") {
      NavigationLink { SkinSettingsView() } label: {
        MeRow(symbol: "paintpalette.fill", title: "我的皮肤", value: skinValue)
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountSkinLink")
      MeDivider()
      NavigationLink { DictionarySettingsView() } label: {
        MeRow(symbol: "book.closed.fill", title: "我的词库")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountDictionaryLink")
      MeDivider()
      NavigationLink { PersonalDictionaryView() } label: {
        MeRow(symbol: "character.book.closed.fill", title: "自造词")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountPersonalDictionary")
      MeDivider()
      if signedIn {
        NavigationLink {
          CloudClipboardView()
        } label: {
          MeRow(symbol: "doc.on.clipboard.fill", title: "云剪贴板", value: clipboardValue)
        }
        .buttonStyle(PressFillButtonStyle())
        .accessibilityIdentifier("accountCloudClipboard")
        MeDivider()
        NavigationLink { AccountPublishedContentView() } label: {
          MeRow(symbol: "tray.full.fill", title: "我的发布与收藏")
        }
        .buttonStyle(PressFillButtonStyle())
        .accessibilityIdentifier("accountPublishedContent")
      } else {
        Button { showsLogin = true } label: {
          MeRow(symbol: "doc.on.clipboard.fill", title: "云剪贴板")
        }
        .buttonStyle(PressFillButtonStyle())
        .accessibilityIdentifier("accountCloudClipboard")
      }
    }
  }

  private var syncGroup: some View {
    MeGroup(title: "同步") {
      // 与 Android 的同步行相同：整行是开关，副标题是一行状态。打开时马上比对一次，之后每次回到前台按节流自动同步。
      Toggle(isOn: Binding(get: { signedIn && cloudSync.status.enabled }, set: setCloudSync)) {
        HStack(spacing: 12) {
          Image(systemName: "arrow.triangle.2.circlepath.circle.fill").font(.system(size: 18))
            .foregroundStyle(MetasequoiaTheme.accent)
            .frame(width: 28, height: 28)
            .accessibilityHidden(true)
          VStack(alignment: .leading, spacing: 1) {
            Text("云同步").font(.system(size: 16)).foregroundStyle(.primary)
            Text(signedIn ? cloudSync.status.statusLine : "登录后可用")
              .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
              .fixedSize(horizontal: false, vertical: true)
          }
        }
      }
      .toggleStyle(.switch)
      .tint(MetasequoiaTheme.switchOn)
      .padding(.vertical, 8)
      .padding(.horizontal, 14)
      .frame(maxWidth: .infinity, minHeight: 48, alignment: .leading)
      .disabled(!signedIn)
      .opacity(signedIn ? 1 : 0.38)
      .accessibilityIdentifier("accountCloudSync")
      MeDivider()
      NavigationLink { AccountCloudSyncView() } label: {
        MeRow(symbol: "arrow.up.arrow.down.circle.fill", title: "手动同步",
              subtitle: signedIn ? "上传或下载设置，管理云词库；词库不自动同步" : "登录后可用")
      }
      .buttonStyle(PressFillButtonStyle())
      .disabled(!signedIn)
      .opacity(signedIn ? 1 : 0.38)
      .accessibilityIdentifier("accountManualSync")
      MeDivider()
      // 与 Android「我的」→「同步」里的同名一行相同：输入 QQ 群里机器人给的绑定码，之后在群里用 /stats 看打字数据。
      NavigationLink { QQLinksView() } label: {
        MeRow(symbol: "person.2.circle.fill", title: "绑定 QQ 群助手",
              subtitle: signedIn ? "在 QQ 群里用 /stats 查看你的打字数据" : "登录后可用")
      }
      .buttonStyle(PressFillButtonStyle())
      .disabled(!signedIn)
      .opacity(signedIn ? 1 : 0.38)
      .accessibilityIdentifier("accountQQLinks")
    }
  }

  /// 打开或关闭云同步，与 Android 的 `setSync` 相同：结果用一条提示说明，打开后马上比对一次（首次的「合并 / 使用云端」选择也当场弹出）。
  @MainActor private func setCloudSync(_ enabled: Bool) {
    guard request.isVisible else { return }
    let id = syncGeneration.begin()
    ViewRequestTask.cancel(task: $syncTask)
    syncTask = Task { @MainActor in
      defer { if syncGeneration.isCurrent(id) { syncTask = nil } }
      do {
        try await cloudSync.setEnabled(enabled)
        guard request.isVisible, syncGeneration.isCurrent(id), !Task.isCancelled else { return }
        ToastCenter.shared.show(enabled ? "云同步已开启" : "云同步已关闭")
      } catch is CancellationError { }
      catch {
        guard request.isVisible, syncGeneration.isCurrent(id), !Task.isCancelled else { return }
        ToastCenter.shared.show((error as? BackendRustClient.Failure)?.code == "cloud_sync_signed_out"
                                ? "登录后才能打开云同步" : "云同步设置没有保存，请重试")
      }
    }
  }

  private var generalGroup: some View {
    MeGroup(title: "通用") {
      if !appThemeCatalog.isEmpty {
        Button { showsAppTheme = true } label: {
          MeRow(symbol: "paintbrush.fill", title: "应用主题", value: appThemeValue)
        }
        .buttonStyle(PressFillButtonStyle())
        .accessibilityIdentifier("accountAppTheme")
        MeDivider()
      }
      NavigationLink { PrivacySettingsView() } label: {
        MeRow(symbol: "lock.shield.fill", title: "隐私", value: "本地优先")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountPrivacyLink")
      MeDivider()
      // 皮肤编辑器的入口只留皮肤页那一个。这里原来还有一个「我的设计」指向同一个编辑器,于是同一件事在两个标签页下各有一条路。
      NavigationLink { AppIconSettingsView() } label: {
        MeRow(symbol: "app.badge.fill", title: "App 图标")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountAppIcon")
    }
  }

  private var moreGroup: some View {
    MeGroup(title: nil) {
      // 沿用本仓库原来页面上的入口：桌面版从这里下载，上游的账号页没有对应的入口可以继承。
      NavigationLink { DesktopDownloadView() } label: {
        MeRow(symbol: "arrow.down.to.line", title: "其他平台下载", value: "7 个平台")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("desktopDownloadLink")
      MeDivider()
      NavigationLink { FeedbackView() } label: {
        MeRow(symbol: "person.bubble.fill", title: "帮助与反馈")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("accountFeedbackLink")
      MeDivider()
      NavigationLink { AboutView() } label: {
        MeRow(symbol: "info.circle.fill", title: "关于", value: version)
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("aboutSettingsLink")
      MeDivider()
      Button { present(.onboarding) } label: {
        MeRow(symbol: "sparkles", title: "新手引导")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("replayOnboardingLink")
      MeDivider()
      Button { present(.splash) } label: {
        MeRow(symbol: "play.circle.fill", title: "开屏动画", value: "播放")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("replaySplashLink")
    }
  }

  // MARK: - 加载

  /// 先读本地会话，已登录时再取资料和云剪贴板条数。会话读不出来时弹出提示，并提供「清除失效登录状态」。
  @MainActor private func startReload() {
    guard !busy else { return }
    ViewRequestTask.startLoad(request: $request, pending: $pending) { id in await reload(id: id) }
  }

  @MainActor private func reload(id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let currentUser = try await api.currentUser()
      try request.checkCurrent(id)
      user = currentUser
      signedIn = currentUser != nil
      needsRecovery = false
    } catch is CancellationError {
      return
    } catch {
      if request.isCurrent(id), !AccountRequestPolicy.isCancellation(error) {
        needsRecovery = true; report(error)
      }
      return
    }
    // 与当前登录对齐：换了账号或已经退出时，这一步关掉开关、清空同步进度。
    await cloudSync.refresh()
    guard (try? request.checkCurrent(id)) != nil else { return }
    guard signedIn else {
      profile = nil
      clipboardValue = nil
      return
    }
    // 每次从推入的页面返回都会重新加载，所以资料取不到时资料卡只停在「已登录」；原因由资料页自己说明。
    if let result = try? await api.profile() {
      guard (try? request.checkCurrent(id)) != nil else { return }
      profile = result
      user = result.user
    }
    await loadClipboardCount(id: id)
  }

  /// 「云剪贴板」行尾的条数；本机等待批准、或有其他设备等待本机批准时显示这件事。取不到时这一行不显示值，不为一个数字弹出提示。
  @MainActor private func loadClipboardCount(id: UUID) async {
    do {
      let deviceName = IOSCloudClipboardDriver.deviceName(UIDevice.current)
      let page = try BackendRustAccount.decode(BackendAccountClient.ClipboardPage.self,
        try await BackendRustAccount.current().cloudClipboard(["operation": "list", "search": ""], deviceName: deviceName))
      try request.checkCurrent(id)
      let waiting = page.e2e?.pending_devices ?? 0
      if !page.enabled { clipboardValue = "已关闭" }
      else if page.isPending { clipboardValue = "等待批准" }
      else if waiting > 0 { clipboardValue = "\(waiting) 台设备待批准" }
      else { clipboardValue = "\(page.items.count) 条" }
    } catch is CancellationError { }
    catch {
      if request.isCurrent(id), !AccountRequestPolicy.isCancellation(error) { clipboardValue = nil }
    }
  }

  /// 资料页退出登录之后回到这里。清掉本地会话状态,下一次登录从登录面板重新开始。
  private func signedOut() {
    signedIn = false
    user = nil
    profile = nil
    clipboardValue = nil
    startCloudRefresh()
  }

  @MainActor private func startCloudRefresh() {
    guard request.isVisible else { return }
    let id = syncGeneration.begin()
    ViewRequestTask.cancel(task: $syncTask)
    syncTask = Task { @MainActor in
      defer { if syncGeneration.isCurrent(id) { syncTask = nil } }
      // 刚退出登录：先让 Rust 清空这个账号的同步进度、作废进行中的那一轮，再对齐状态。
      await cloudSync.signOut()
      await cloudSync.refresh()
      guard request.isVisible, syncGeneration.isCurrent(id), !Task.isCancelled else { return }
    }
  }

  /// 取消不写进 message,别的都写。alert 绑在 message 上,写进去就是弹出来。
  private func report(_ error: Error) {
    guard !AccountRequestPolicy.isCancellation(error) else { return }
    message = error.localizedDescription
  }

  @MainActor private func run(_ action: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $operation,
                        canceling: $pending, clearMessage: false,
                        isCancellation: AccountRequestPolicy.isCancellation, action: action)
  }

  /// 这些全屏层出现和消失时不带上滑动画：启动页和 iPad 卡片各自绘制入场动画，滑动的遮罩看起来像浮层，而不像重播。
  private func present(_ item: Replay) {
    var transaction = Transaction()
    transaction.disablesAnimations = true
    withTransaction(transaction) { replay = item }
  }

  private func dismissReplay() {
    var transaction = Transaction()
    transaction.disablesAnimations = true
    withTransaction(transaction) { replay = nil }
  }
}

/// 表单里的登录区，给要先有账号才能继续的流程用（发布保存的皮肤）。按钮与登录面板相同，登录成功后把 `signedIn` 置为真。
///
/// 本机存的登录状态读不出来时（钥匙串条目损坏或过期），`SavedSkinPublishFlow` 也会把用户带到这里，所以这里和「我的」页一样提供「清除失效登录状态」，清掉后可以重新登录。
struct AppleAccountSection: View {
  @Binding var signedIn: Bool
  @StateObject private var model = CodeLoginModel()
  @State private var needsRecovery = false
  @State private var busy = false
  @State private var message: String?
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()

  var body: some View {
    Section {
      AccountLoginOptions(model: model) { _ in signedIn = true }
        .listRowInsets(EdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16))
        // task 挂在这一行上，不挂在 Section 上：Form 会把 Section 上的修饰符分给每一行，出现「清除失效登录状态」那一行时会再读一次登录状态。
        .task { request.appear(); startCheckSession() }
        .onDisappear {
          ViewRequestTask.disappear(request: $request, pending: $pending)
          busy = false
          model.invalidate()
        }
      if needsRecovery {
        Button("清除失效登录状态") { clearExpiredLogin() }
          .disabled(busy)
          .accessibilityIdentifier("publishClearExpiredLogin")
      }
    } footer: {
      Text(message ?? "登录后可在皮肤社区发布、下载和评分。日常输入无需登录。")
    }
  }

  /// 读一次本机的登录状态：读不出来（不是取消）就提供清除。
  @MainActor private func startCheckSession() {
    ViewRequestTask.startLoad(request: $request, pending: $pending) { id in await checkSession(id: id) }
  }

  @MainActor private func checkSession(id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      _ = try await SkinCommunityAPI.shared.currentUser()
      try request.checkCurrent(id)
      needsRecovery = false
    } catch is CancellationError {
    } catch {
      if request.isCurrent(id), !AccountRequestPolicy.isCancellation(error) {
        needsRecovery = true
        message = error.localizedDescription
      }
    }
  }

  @MainActor private func clearExpiredLogin() {
    run { id in
      try await SkinCommunityAPI.shared.clearExpiredLogin()
      try request.checkCurrent(id)
      needsRecovery = false
      message = nil
    }
  }

  @MainActor private func run(_ action: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending,
                        canceling: $pending, clearMessage: false,
                        isCancellation: AccountRequestPolicy.isCancellation, action: action)
  }
}

// MARK: - 行

/// 「我的」页及其子页面的一行：28pt 图标位里放强调色的 18pt 图标，16pt 标题下可带 12pt 副标题，可选的 14pt 值和一个箭头；最小高度 48pt，内边距 8/14。放进使用 `PressFillButtonStyle` 的 `NavigationLink` 或 `Button` 里。
struct MeRow: View {
  let symbol: String
  let title: String
  var subtitle: String? = nil
  var value: String? = nil

  var body: some View {
    HStack(spacing: 12) {
      Image(systemName: symbol).font(.system(size: 18))
        .foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 28, height: 28)
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 1) {
        Text(title).font(.system(size: 16)).foregroundStyle(.primary)
        if let subtitle {
          Text(subtitle).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      .multilineTextAlignment(.leading)
      Spacer(minLength: 8)
      if let value {
        Text(value).font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
      }
      DesignChevron()
    }
    .padding(.vertical, 8)
    .padding(.horizontal, 14)
    .frame(maxWidth: .infinity, minHeight: 48, alignment: .leading)
    .contentShape(Rectangle())
  }
}

/// 「我的」页的一组：可选的 13pt 标题，位于圆角 10 的卡片上方 7pt。标题比卡片边缘缩进 4pt，与设计稿各标签页一致。
private struct MeGroup<Content: View>: View {
  let title: String?
  private let content: Content

  init(title: String?, @ViewBuilder content: () -> Content) {
    self.title = title
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 7) {
      if let title {
        SettingsGroupHeader(title: title).padding(.horizontal, 4)
      }
      DesignCard(radius: MetasequoiaTheme.tabCardRadius) { content }
    }
  }
}

/// 「我的」页卡片里各行之间的通栏细线。
private struct MeDivider: View {
  var body: some View { DesignDivider(leading: 0) }
}

/// 资料卡自己的卡片底：季节卡片色，圆角 10，按下时的填充裁剪在卡片内。
private struct AccountCardButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    let shape = RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous)
    configuration.label
      .background(configuration.isPressed ? MetasequoiaTheme.pressFill : Color.clear)
      .background(MetasequoiaTheme.surface)
      .clipShape(shape)
      .contentShape(shape)
  }
}

// MARK: - 子页面

/// 「手动同步」：自动同步之外，在这台设备上立刻上传或下载设置；个人词库不在自动同步里（键盘里的 Engine 持有它，应用进程导不出来），云词库的上传下载也在这里。
private struct AccountCloudSyncView: View {
  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 7) {
        MeGroup(title: nil) {
          NavigationLink { SettingsSyncView() } label: {
            MeRow(symbol: "arrow.triangle.2.circlepath", title: "设置同步", subtitle: "上传本机设置，或下载云端设置并应用")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("accountSettingsSync")
          MeDivider()
          NavigationLink { CloudDictionaryView() } label: {
            MeRow(symbol: "icloud.fill", title: "云词库", subtitle: "查看和管理保存在云端的词库")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("accountCloudDictionary")
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 24)
    }
    .designCanvasBackground()
    .navigationTitle("手动同步").navigationBarTitleDisplayMode(.inline)
  }
}

/// 「我的发布与收藏」：已发布的皮肤、词库和回复模板，以及按类别分开的「我发布的」/「收藏的」列表。
private struct AccountPublishedContentView: View {
  var body: some View {
    ScrollView {
      VStack(spacing: 18) {
        MeGroup(title: "我发布的") {
          NavigationLink { SkinCommunityView(onlyMine: true) } label: {
            MeRow(symbol: "paintpalette.fill", title: "我的皮肤")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("accountPublishedSkins")
          MeDivider()
          NavigationLink { CommunityResourcesAccountView() } label: {
            MeRow(symbol: "tray.full.fill", title: "词包与回复模板")
          }
          .buttonStyle(PressFillButtonStyle())
          .accessibilityIdentifier("accountCommunityResources")
          ForEach(CommunityResourceKind.allCases) { kind in
            MeDivider()
            NavigationLink { CommunityResourcesView(kind: kind, initialScope: "mine") } label: {
              MeRow(symbol: kind.icon, title: "我发布的\(kind.title)")
            }
            .buttonStyle(PressFillButtonStyle())
          }
        }
        MeGroup(title: "收藏") {
          ForEach(Array(CommunityResourceKind.allCases.enumerated()), id: \.element.id) { index, kind in
            if index > 0 { MeDivider() }
            NavigationLink { CommunityResourcesView(kind: kind, initialScope: "saved") } label: {
              MeRow(symbol: "bookmark.fill", title: "收藏的\(kind.title)")
            }
            .buttonStyle(PressFillButtonStyle())
          }
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 16)
      .padding(.bottom, 24)
    }
    .designCanvasBackground()
    .navigationTitle("我的发布与收藏").navigationBarTitleDisplayMode(.inline)
  }
}

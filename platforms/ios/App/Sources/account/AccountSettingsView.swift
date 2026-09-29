import SwiftUI
import AuthenticationServices

/// 任务取消不是错误。切换标签页会让 `.task` 连同它已经发出的请求一起取消,而 URLSession 抛回来的是 `URLError.cancelled` —— 它的 `localizedDescription` 就是 "cancelled"。把它当错误弹出来,结果就是每次切到「我的」都跳一个写着 cancelled 的框。
///
/// 只判 `CancellationError` 不够:那是 Swift 结构化并发自己抛的那一种,而这里真正会抛的是网络层那一种,两者没有继承关系。
private func isCancellation(_ error: Error) -> Bool {
  if error is CancellationError { return true }
  if let error = error as? URLError { return error.code == .cancelled }
  return false
}

struct AccountSettingsView: View {
  @Environment(\.horizontalSizeClass) private var widthClass
  @State private var signedIn = false
  @State private var replay: Replay?

  /// What 我的 can play again. Both cover the whole window, so neither slides up as a sheet over the tab bar.
  private enum Replay: String, Identifiable {
    case onboarding, splash
    var id: Self { self }
  }

  private var version: String {
    Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "开发构建"
  }

  var body: some View {
    List {
      AppleAccountSection(signedIn: $signedIn)

      Section {
        if signedIn {
          NavigationLink(destination: CloudClipboardView(session: .shared, client: BackendAccountClient())) {
            SettingsNavLabel(title: "云剪贴板", symbol: "doc.on.clipboard")
          }.accessibilityIdentifier("accountCloudClipboard")
        }
        NavigationLink(destination: DictionarySettingsView()) {
          SettingsNavLabel(title: "词库", symbol: "books.vertical")
        }.accessibilityIdentifier("accountDictionaryLink")
        // Kept from this repository's own page: the desktop build is downloaded from here, and the upstream account page has no equivalent entry to inherit it from.
        NavigationLink(destination: DesktopDownloadView()) {
          SettingsNavLabel(title: "其他平台下载", symbol: "desktopcomputer")
        }.accessibilityIdentifier("desktopDownloadLink")
      } header: {
        SettingsGroupHeader(title: "工具")
      }

      if signedIn {
        Section {
          NavigationLink(destination: SettingsSyncView(session: .shared, client: BackendAccountClient())) {
            SettingsNavLabel(title: "设置同步", symbol: "arrow.triangle.2.circlepath")
          }.accessibilityIdentifier("accountSettingsSync")
          NavigationLink(destination: CloudDictionaryView()) {
            SettingsNavLabel(title: "云词库", symbol: "icloud")
          }.accessibilityIdentifier("accountCloudDictionary")
        } header: {
          SettingsGroupHeader(title: "同步")
        }
      }

      Section {
        NavigationLink(destination: PersonalDictionaryView()) {
          SettingsNavLabel(title: "自造词", symbol: "character.book.closed")
        }.accessibilityIdentifier("accountPersonalDictionary")
        if signedIn {
          NavigationLink(destination: SkinCommunityView(onlyMine: true)) {
            SettingsNavLabel(title: "我的皮肤", symbol: "paintpalette")
          }.accessibilityIdentifier("accountPublishedSkins")
          NavigationLink(destination: CommunityResourcesAccountView()) {
            SettingsNavLabel(title: "词包与回复模板", symbol: "tray.full")
          }.accessibilityIdentifier("accountCommunityResources")
          ForEach(CommunityResourceKind.allCases) { kind in
            NavigationLink(destination: CommunityResourcesView(kind: kind, initialScope: "mine")) {
              SettingsNavLabel(title: "我发布的\(kind.title)", symbol: kind.icon)
            }
          }
          ForEach(CommunityResourceKind.allCases) { kind in
            NavigationLink(destination: CommunityResourcesView(kind: kind, initialScope: "saved")) {
              SettingsNavLabel(title: "收藏的\(kind.title)", symbol: "bookmark")
            }
          }
        }
      } header: {
        SettingsGroupHeader(title: "我的内容")
      }

      // 皮肤编辑器的入口只留皮肤页那一个。这里原来还有一个「我的设计」指向同一个编辑器,于是同一件事在两个标签页下各有一条路。
      Section {
        NavigationLink(destination: AppIconSettingsView()) {
          SettingsNavLabel(title: "App 图标", symbol: "app.badge")
        }.accessibilityIdentifier("accountAppIcon")
      } header: {
        SettingsGroupHeader(title: "个性化")
      }

      Section {
        Button { present(.onboarding) } label: {
          SettingsNavLabel(title: "新手引导", symbol: "sparkles")
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("replayOnboardingLink")
        Button { present(.splash) } label: {
          SettingsNavLabel(title: "开屏动画", symbol: "play.circle", value: "播放")
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("replaySplashLink")
        Link(destination: URL(string: "https://msime.app/privacy/")!) {
          SettingsNavLabel(title: "隐私", symbol: "hand.raised", value: "本地优先")
        }
        // A plain style, or the list tints a Link's whole label in the accent like a button.
        .buttonStyle(.plain)
        .accessibilityIdentifier("accountPrivacyLink")
        NavigationLink(destination: FeedbackView()) {
          SettingsNavLabel(title: "帮助与反馈", symbol: "questionmark.bubble")
        }.accessibilityIdentifier("accountFeedbackLink")
        NavigationLink(destination: AboutView()) {
          SettingsNavLabel(title: "关于", symbol: "info.circle", value: version)
        }.accessibilityIdentifier("aboutSettingsLink")
      } footer: {
        Text("皮肤设计和打字统计保存在本机。只有你主动发布的作品会分享至社区；Apple 登录不会自动上传本地设计或输入记录。")
      }
    }
    .listStyle(.insetGrouped)
    .environment(\.defaultMinListRowHeight, 48)
    .navigationTitle("我的")
    .navigationBarTitleDisplayMode(.large)
    .tint(MetasequoiaTheme.accent)
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
  }

  /// The covers appear and leave without the slide-up: the splash and the iPad card draw their own entrance, and a sliding scrim reads as a sheet rather than a replay.
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

struct AppleAccountSection: View {
  @Binding var signedIn: Bool
  @StateObject private var codeModel = CodeLoginModel()
  @Environment(\.colorScheme) private var colorScheme
  @State private var codeChannel: CodeLoginChannel?
  @State private var user: CommunityUser?
  @State private var needsRecovery = false
  @State private var busy = false
  @State private var message: String?
  @State private var challenge: CommunityChallenge?
  private let api = SkinCommunityAPI.shared

  private var displayName: String {
    user?.preferredDisplayName ?? "水杉用户"
  }

  /// The account card of the design: a 56pt round avatar -- the name's first character on the accent when signed in, a question mark on grey when not -- beside the name in 18pt bold and one line of status.
  private var profileCard: some View {
    HStack(spacing: 14) {
      Text(signedIn ? String(displayName.prefix(1)) : "?")
        .font(.system(size: 22, weight: .semibold))
        .foregroundStyle(signedIn ? MetasequoiaTheme.onAccent : Color.secondary)
        .frame(width: 56, height: 56)
        .background(signedIn ? MetasequoiaTheme.accent : Color(uiColor: .tertiarySystemFill), in: Circle())
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 4) {
        Text(signedIn ? displayName : "未登录")
          .font(.system(size: 18, weight: .bold))
        Text(signedIn ? "水杉账号已登录" : "登录后同步词库、皮肤和设置")
          .font(.system(size: 13)).foregroundStyle(.secondary)
      }
      Spacer()
    }
    .contentShape(Rectangle())
  }

  /// 资料页退出登录之后回到这里。清掉本地会话状态,并把下一次 Apple 登录的挑战重新取一份。
  private func signOut() {
    signedIn = false
    user = nil
    Task { await prepareLogin() }
  }

  var body: some View {
    Section {
      // 资料页是推进去的二级页面,不是浮层:它要承载退出登录这类收尾操作,而这些操作做完之后回到的是一个已经变了的「我的」页 —— 浮层盖在旧内容上关掉的那一下,看起来就像什么都没发生。
      if signedIn {
        NavigationLink {
          AccountProfileEditor(initialUser: user, onSaved: { user = $0 }, onSignedOut: signOut)
        } label: {
          profileCard
        }
        .padding(.vertical, 6)
        .accessibilityIdentifier("accountProfileCard")
      } else {
        profileCard
          .padding(.vertical, 6)
          .accessibilityIdentifier("accountProfileCard")
      }
      if !signedIn {
        if let challenge {
          SignInWithAppleButton(.signIn) { request in
            request.nonce = challenge.nonce
            request.state = challenge.challenge_id
          } onCompletion: { result in
            switch result {
            case .success(let authorization):
              guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
                    credential.state == challenge.challenge_id,
                    let data = credential.identityToken, let token = String(data: data, encoding: .utf8) else {
                message = "Apple 登录未返回有效凭据，请重试。"; Task { await prepareLogin() }; return
              }
              run {
                do { try await api.login(challenge: challenge.challenge_id, identityToken: token) }
                catch { await prepareLogin(); throw error }
                user = try await api.currentUser()
                signedIn = true
              }
            case .failure(let error):
              if (error as? ASAuthorizationError)?.code != .canceled { message = error.localizedDescription }
              Task { await prepareLogin() }
            }
          }.accessibilityIdentifier("backendAppleSignIn").signInWithAppleButtonStyle(colorScheme == .dark ? .white : .black).frame(height: 44).disabled(busy)
        } else if codeModel.providers["apple"] == true {
          Button("准备 Apple 登录") { Task { await prepareLogin() } }.disabled(busy)
        }
        ForEach(CodeLoginChannel.allCases) { channel in
          if codeModel.providers[channel.rawValue] == true {
            Button(channel.title) { codeModel.user = nil; codeModel.message = nil; codeChannel = channel }
              .accessibilityIdentifier("backendCodeLogin_\(channel.rawValue)")
          }
        }
        Button("刷新登录方式") { Task { await codeModel.loadProviders(); await prepareLogin() } }
        if let status = codeModel.message { Text(status).font(.caption).foregroundStyle(.secondary) }
        Text("登录后可在皮肤社区发布、下载和评分。日常输入无需登录。").font(.caption).foregroundStyle(.secondary)
        if needsRecovery {
          Button("清除失效登录状态") { run { try await api.clearExpiredLogin(); signedIn = false; needsRecovery = false; await prepareLogin() } }
            .font(.caption)
        }
      }
    }
    .task {
      await codeModel.loadProviders()
      do { user = try await api.currentUser(); signedIn = user != nil }
      catch { if !isCancellation(error) { needsRecovery = true; report(error) } }
      if signedIn {
        do { user = try await api.profile().user }
        catch { report(error) }
      } else { await prepareLogin() }
    }
    .sheet(item: $codeChannel, onDismiss: {
      Task {
        do { user = try await api.currentUser(); signedIn = user != nil }
        catch { report(error) }
      }
    }) { channel in CodeLoginView(model: codeModel, channel: channel) }
    .alert("账号与登录", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }
  @MainActor private func prepareLogin() async {
    challenge = nil
    guard codeModel.providers["apple"] == true else { return }
    do { challenge = try await api.challenge() } catch { report(error) }
  }
  /// 取消不写进 message,别的都写。alert 绑在 message 上,写进去就是弹出来。
  private func report(_ error: Error) {
    guard !isCancellation(error) else { return }
    message = error.localizedDescription
  }
  private func run(_ action: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }; busy = true
    Task {
      defer { busy = false }
      do { try await action() }
      catch {
        report(error)
        if let state = try? await api.signedIn() {
          signedIn = state
          if !state { await prepareLogin() }
        }
      }
    }
  }
}

struct AccountProfileEditor: View {
  var initialUser: CommunityUser? = nil
  var onSaved: (CommunityUser) -> Void
  var onSignedOut: () -> Void
  @Environment(\.dismiss) private var dismiss
  @FocusState private var editingName: Bool
  @State private var profile: CommunityProfile?
  @State private var name = ""
  @State private var loading = true
  @State private var saving = false
  @State private var busy = false
  @State private var message: String?
  @State private var copiedID = false
  @State private var confirmDiscard = false
  @State private var confirmSignOut = false
  @State private var confirmLogoutAll = false
  @State private var confirmRelogin = false
  @State private var confirmDeleteAccount = false

  private var normalizedName: String { CommunityProfilePolicy.normalizedName(name) }
  private var validName: Bool { CommunityProfilePolicy.validName(name) }
  private var hasChanges: Bool {
    guard let profile else { return false }
    return normalizedName != profile.user.preferredDisplayName
  }
  private var previewName: String {
    if profile == nil { return initialUser?.preferredDisplayName ?? "水杉用户" }
    return normalizedName.isEmpty ? "你的昵称" : normalizedName
  }
  private var nameHint: String {
    if normalizedName.isEmpty { return "取一个喜欢的名字，让大家记住你。" }
    if !validName { return "昵称最多 64 个字符，请勿使用换行或控制字符。" }
    return "昵称会显示在社区作品中，已发布的作品也会同步更新。"
  }
  private var joined: String? {
    guard let value = profile?.user.created_at else { return nil }
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let fractional = formatter.date(from: value)
    formatter.formatOptions = [.withInternetDateTime]
    guard let date = fractional ?? formatter.date(from: value) else { return nil }
    return date.formatted(date: .abbreviated, time: .omitted)
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        profilePreview
        if loading {
          ProgressView("正在加载资料")
            .font(.subheadline).frame(maxWidth: .infinity).padding(.vertical, 28)
        } else if profile != nil {
          nicknameCard
          accountDetails
          accountActions
        }
        if let message, profile == nil {
          VStack(alignment: .leading, spacing: 12) {
            Label(message, systemImage: "exclamationmark.circle")
              .font(.subheadline).foregroundStyle(.secondary)
            if profile == nil {
              Button("重新加载") { Task { await load() } }
                .font(.subheadline.weight(.semibold))
            }
          }
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(20)
          .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
          .accessibilityIdentifier("accountProfileError")
        }
      }
      .frame(maxWidth: 540)
      .padding(.horizontal, 24).padding(.top, 24).padding(.bottom, 28)
      .frame(maxWidth: .infinity)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("编辑资料")
    .navigationBarTitleDisplayMode(.inline)
    // The stock back button pops without asking, and a half-typed nickname would go with it. This one runs the same discard prompt the close button used to.
    .navigationBarBackButtonHidden(true)
    .toolbar {
      ToolbarItem(placement: .navigationBarLeading) {
        Button {
          editingName = false
          if hasChanges { confirmDiscard = true } else { dismiss() }
        } label: {
          Image(systemName: "chevron.backward").font(.body.weight(.semibold))
        }
        .accessibilityLabel("返回").disabled(saving || busy)
      }
      ToolbarItemGroup(placement: .keyboard) {
        Spacer()
        Button("完成") { editingName = false }
      }
    }
    .safeAreaInset(edge: .bottom, spacing: 0) { saveBar }
    // 保存栏已经占着下沿,标签栏再浮一条上来,两条栏会叠在一起。
    .toolbar(.hidden, for: .tabBar)
    .confirmationDialog("要放弃这次修改吗？", isPresented: $confirmDiscard, titleVisibility: .visible) {
      Button("放弃修改", role: .destructive) { dismiss() }
      Button("继续编辑", role: .cancel) {}
    }
    .confirmationDialog("退出登录后，社区功能要重新登录才能使用。", isPresented: $confirmSignOut, titleVisibility: .visible) {
      Button("退出登录", role: .destructive) { perform { try await SkinCommunityAPI.shared.logout() } }
    }
    .confirmationDialog("退出所有设备后，所有设备都需要重新登录。", isPresented: $confirmLogoutAll, titleVisibility: .visible) {
      Button("退出所有设备", role: .destructive) { perform { try await SkinCommunityAPI.shared.logout(all: true) } }
    }
    .confirmationDialog("清掉本机的登录状态后需要重新登录一次。", isPresented: $confirmRelogin, titleVisibility: .visible) {
      Button("重新登录") { perform { try await SkinCommunityAPI.shared.clearExpiredLogin() } }
    }
    .confirmationDialog("注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。", isPresented: $confirmDeleteAccount, titleVisibility: .visible) {
      Button("注销账号", role: .destructive) { perform { try await SkinCommunityAPI.shared.logout(deleteAccount: true) } }
    }
    .tint(MetasequoiaTheme.accent)
    .task { await load() }
  }

  /// 账号操作跟着资料一起放在这一页:它们问的是同一件事 —— 这个账号 —— 而每一个做完之后回到的「我的」页都已经不是刚才那一页了,推进来再退回去,这个变化看得见。放在首页时它们是一行没有图标的绿字,或者一个说不出内容的 ··· 菜单。
  private var accountActions: some View {
    VStack(alignment: .leading, spacing: 12) {
      Text("账号操作").font(.subheadline.weight(.semibold))
      VStack(spacing: 0) {
        actionRow("退出登录", detail: "本机的皮肤设计和打字统计不受影响", symbol: "rectangle.portrait.and.arrow.right",
                  identifier: "signOutAccount") { confirmSignOut = true }
        Divider().padding(.leading, 54)
        actionRow("退出所有设备", detail: "所有设备都需要重新登录", symbol: "iphone.and.arrow.forward",
                  identifier: "signOutEverywhere") { confirmLogoutAll = true }
        Divider().padding(.leading, 54)
        actionRow("重新登录", detail: "登录状态出错时清掉它再登一次", symbol: "arrow.clockwise",
                  identifier: "clearExpiredLogin") { confirmRelogin = true }
      }
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
      actionRow("注销账号", detail: "删除云端账号数据，无法撤销", symbol: "trash",
                identifier: "deleteAccount", destructive: true) { confirmDeleteAccount = true }
        .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
        .padding(.top, 4)
    }
  }

  private func actionRow(_ title: String, detail: String, symbol: String, identifier: String,
                         destructive: Bool = false, action: @escaping () -> Void) -> some View {
    Button(action: action) {
      HStack(alignment: .center, spacing: 12) {
        Image(systemName: symbol).font(.subheadline)
          .foregroundStyle(destructive ? Color.red : MetasequoiaTheme.accent).frame(width: 22)
        VStack(alignment: .leading, spacing: 5) {
          Text(title).font(.subheadline.weight(.medium))
            .foregroundStyle(destructive ? Color.red : Color.primary)
          Text(detail).font(.caption).foregroundStyle(.secondary)
        }
        Spacer(minLength: 8)
        if busy { ProgressView() }
      }
      .frame(maxWidth: .infinity, alignment: .leading).padding(20)
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .disabled(saving || busy)
    .accessibilityIdentifier(identifier)
  }

  /// 四个动作都以「这台设备不再登录」收尾,所以走同一条路:调用、回调父视图、退回上一页。失败时留在原地把原因说出来。
  private func perform(_ action: @escaping () async throws -> Void) {
    editingName = false
    busy = true
    message = nil
    Task {
      defer { busy = false }
      do {
        try await action()
        onSignedOut()
        dismiss()
      } catch { report(error) }
    }
  }

  private var profilePreview: some View {
    VStack(spacing: 14) {
      ZStack {
        Circle().fill(MetasequoiaTheme.accent.opacity(0.07)).frame(width: 104, height: 104)
        Circle()
          .fill(LinearGradient(colors: [MetasequoiaTheme.needle, MetasequoiaTheme.forest],
                               startPoint: .topLeading, endPoint: .bottomTrailing))
          .frame(width: 84, height: 84)
        Text(String(previewName.prefix(1)))
          .font(.system(size: 32, weight: .medium, design: .rounded)).foregroundStyle(.white)
      }
      .accessibilityHidden(true)
      VStack(spacing: 6) {
        Text(previewName).font(.title2.weight(.semibold))
          .multilineTextAlignment(.center).lineLimit(2)
        Text("在水杉，留下你的名字")
          .font(.subheadline).foregroundStyle(.secondary)
      }
    }
    .frame(maxWidth: .infinity).padding(.vertical, 8)
    .accessibilityIdentifier("accountProfilePreview")
  }

  private var nicknameCard: some View {
    VStack(alignment: .leading, spacing: 12) {
      Text("社区昵称").font(.subheadline.weight(.semibold))
      VStack(alignment: .leading, spacing: 16) {
        HStack(spacing: 12) {
          TextField("设置你的昵称", text: $name)
            .font(.title3.weight(.medium))
            .textContentType(.nickname).submitLabel(.done)
            .focused($editingName).onSubmit { editingName = false }
            .disabled(saving)
            .accessibilityIdentifier("accountNicknameField")
          if !name.isEmpty {
            Button { name = ""; editingName = true } label: {
              Image(systemName: "xmark.circle.fill").foregroundStyle(.tertiary)
                .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain).accessibilityLabel("清空昵称").disabled(saving)
          }
        }
        Divider()
        HStack(alignment: .top, spacing: 16) {
          Text(nameHint).fixedSize(horizontal: false, vertical: true)
          Spacer(minLength: 0)
          Text("\(normalizedName.unicodeScalars.count)/64")
            .monospacedDigit().fixedSize()
            .accessibilityLabel("已输入 \(normalizedName.unicodeScalars.count) 个字符，最多 64 个")
        }
        .font(.caption)
        .foregroundStyle(validName || normalizedName.isEmpty ? Color.secondary : Color.red)
      }
      .padding(20)
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
      .overlay {
        RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous)
          .strokeBorder(editingName ? MetasequoiaTheme.accent.opacity(0.5) : .clear, lineWidth: 1.5)
      }
    }
  }

  private var accountDetails: some View {
    VStack(alignment: .leading, spacing: 12) {
      Text("账号信息").font(.subheadline.weight(.semibold))
      VStack(spacing: 0) {
        if let profile {
          Button {
            UIPasteboard.general.string = profile.user.id
            copiedID = true
          } label: {
            detailRow("账号 ID", value: "#" + profile.user.id.prefix(6).uppercased(),
                      symbol: "number", accessory: copiedID ? "checkmark" : "doc.on.doc")
          }
          .buttonStyle(.plain)
          .accessibilityLabel(copiedID ? "账号 ID 已复制" : "复制完整账号 ID")
          .accessibilityIdentifier("copyAccountID")
          if copiedID {
            Text("账号 ID 已复制").font(.caption).foregroundStyle(MetasequoiaTheme.accent)
              .frame(maxWidth: .infinity, alignment: .leading)
              .padding(.horizontal, 20).padding(.bottom, 12)
          }
          Divider().padding(.leading, 54)
          detailRow("登录方式", value: loginProviders(profile), symbol: "person.badge.key")
          if let joined {
            Divider().padding(.leading, 54)
            detailRow("加入水杉", value: joined, symbol: "calendar")
          }
        }
      }
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
      Label("轻点账号 ID 即可复制", systemImage: "lock")
        .font(.caption).foregroundStyle(.secondary).padding(.horizontal, 4)
    }
  }

  private func detailRow(_ title: String, value: String, symbol: String,
                         accessory: String? = nil) -> some View {
    HStack(alignment: .center, spacing: 12) {
      Image(systemName: symbol).font(.subheadline)
        .foregroundStyle(MetasequoiaTheme.accent).frame(width: 22)
      VStack(alignment: .leading, spacing: 5) {
        Text(title).font(.caption).foregroundStyle(.secondary)
        Text(value.isEmpty ? "暂无信息" : value).font(.subheadline.weight(.medium))
          .foregroundStyle(.primary)
      }
      Spacer(minLength: 8)
      if let accessory {
        Image(systemName: accessory).font(.subheadline)
          .foregroundStyle(MetasequoiaTheme.accent)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading).padding(20)
  }

  private func loginProviders(_ profile: CommunityProfile) -> String {
    profile.identities.map {
      switch $0.provider {
      case "apple": return "Apple"
      case "email": return "邮箱"
      case "phone", "sms": return "手机号"
      default: return $0.provider
      }
    }.joined(separator: "、")
  }

  private var saveBar: some View {
    VStack(spacing: 12) {
      if let message, profile != nil {
        Label(message, systemImage: "exclamationmark.circle")
          .font(.caption).foregroundStyle(.red)
          .fixedSize(horizontal: false, vertical: true)
          .accessibilityIdentifier("accountProfileSaveError")
      }
      Button(action: save) {
        HStack(spacing: 10) {
          if saving { ProgressView().tint(MetasequoiaTheme.onAccent) }
          Text(saving ? "正在保存…" : "保存修改").font(.body.weight(.semibold))
        }
        .foregroundStyle(MetasequoiaTheme.onAccent)
        .frame(maxWidth: .infinity).padding(.vertical, 17)
        .background(MetasequoiaTheme.accent.opacity(canSave || saving ? 1 : 0.45),
                    in: RoundedRectangle(cornerRadius: 18))
      }
      .buttonStyle(.plain)
      .disabled(!canSave)
      .accessibilityIdentifier("saveAccountProfile")
    }
    .frame(maxWidth: 540)
    .padding(.horizontal, 24).padding(.top, 12).padding(.bottom, 12)
    .frame(maxWidth: .infinity)
    .background(.regularMaterial)
  }

  private var canSave: Bool { !loading && !saving && profile != nil && validName && hasChanges }

  private func save() {
    guard canSave else { return }
    editingName = false
    saving = true
    message = nil
    let submittedName = normalizedName
    Task {
      defer { saving = false }
      do {
        let updated = try await SkinCommunityAPI.shared.updateProfile(name: submittedName)
        onSaved(updated.user)
        dismiss()
      } catch { report(error) }
    }
  }

  @MainActor private func load() async {
    loading = true
    message = nil
    defer { loading = false }
    do {
      let result = try await SkinCommunityAPI.shared.profile()
      profile = result
      name = result.user.preferredDisplayName
      onSaved(result.user)
    } catch { report(error) }
  }

  private func report(_ error: Error) {
    guard !isCancellation(error) else { return }
    message = error.localizedDescription
  }
}

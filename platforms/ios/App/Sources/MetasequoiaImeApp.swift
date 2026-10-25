import SwiftUI
#if canImport(GoogleSignIn)
import GoogleSignIn
#endif

@main
struct MetasequoiaImeApp: App {
  @StateObject private var onboardingNavigation = AppNavigation()
  // 各标签页的导航和深链接路由放在这里，在以 `paletteVersion` 为 key 的内容之外，这样在「我的」里选择应用主题只会重绘颜色，不会把用户甩回「设置」标签页。
  @StateObject private var tabNavigation = AppNavigation()
  @StateObject private var router = SettingsRouter()
  @Environment(\.scenePhase) private var scenePhase
  @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding = false
  /// 作为内容的 key，使季节或所选应用主题变化时所有动态颜色都重新解析；UIKit 只在 trait 变化时才重新解析动态颜色提供者。
  @State private var paletteVersion = AppThemePalette.version

  init() {
    // 账号与云端业务的 Rust 入口都经这张函数表调用（shared/backend/rust/MSIMERustClient.h），会话存储也在这里注册，要在任何页面用到 `BackendRustAccount.shared` 之前。
    // 云同步交给 Rust 以前的状态（App Group 的 `CloudSync/state.json`）导入一次，要在用户能动开关之前；导入过、或者已经有进度时 Rust 什么也不做。
    CloudSyncState.shared.importLegacy()
    if let account = IOSRustAccount.install() {
      // 第一次启动就注册本机匿名账号，键盘需要之前就有；已有合法的会话时 Rust 只读一次存储、不联网，失败了下次启动再试。
      Task.detached(priority: .utility) { await IOSRustAccount.ensureAnonymous(account) }
    }
    CrashDiagnostics.shared.start()
    try? KeyboardSkinTrialStore().restorePending()
    #if DEBUG
    let arguments = ProcessInfo.processInfo.arguments
    if arguments.contains("--reset-onboarding-for-ui-tests") {
      UserDefaults.standard.removeObject(forKey: "hasCompletedOnboarding")
    }
    // Scheme visibility lives in the app group and outlives the app, so a test that hides a scheme
    // would otherwise decide what later tests -- in this bundle and in the keyboard unit tests that
    // share the group -- can select. Restorable on its own so a test can undo the damage it did
    // without also throwing away onboarding state it still needs.
    if arguments.contains("--reset-onboarding-for-ui-tests") || arguments.contains("--reset-input-schemes-for-ui-tests") {
      UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier)?
        .removeObject(forKey: InputSchemePreference.enabledSchemesKey)
      // 设置页会把共享文档里记的启用列表抄回 App Group，所以文档里的那份也要清掉，否则隐藏的方案在下一次打开设置页时又回来。
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0.removeValue(forKey: "touch_keyboard_schemes") }
    }
    // The saved-skin library holds twelve. A UI test that saves one and fails before deleting it
    // leaves it behind, and twelve such runs make every later save fail with no way back short of
    // erasing the simulator -- the app group outlives uninstalling the app.
    if arguments.contains("--reset-custom-skins-for-ui-tests") {
      CustomSkinLibrary.removeAll()
    }
    #endif
    _hasCompletedOnboarding = AppStorage(wrappedValue: false, "hasCompletedOnboarding")
    Self.applyChrome()
  }

  var body: some Scene {
    WindowGroup {
      #if DEBUG && targetEnvironment(simulator)
      if ProcessInfo.processInfo.arguments.contains("-communityReplyEditorPreview") {
        CommunityResourceEditor(kind: .reply)
      } else if ProcessInfo.processInfo.arguments.contains("-launchScreenPreview") {
        LaunchScreenPreview().ignoresSafeArea()
      } else if ProcessInfo.processInfo.arguments.contains("-keyboardReplyPreview") {
        ReplyKeyboardPreview().frame(width: 390, height: 260)
      } else if ProcessInfo.processInfo.arguments.contains("-keyboardAIPreview") {
        KeyboardAIView(text: previewText,
          configuration: CustomServiceConfiguration(endpoint: "https://fixture.invalid/v1/chat/completions", model: "fixture"),
          canSend: { false }, insert: { _ in false }, close: {})
          .frame(width: 320, height: previewHeight)
          .environment(\.sizeCategory, previewSizeCategory)
      } else if ProcessInfo.processInfo.arguments.contains("-keyboardVoicePreview") {
        KeyboardVoicePreviewFixture().frame(width: 320, height: previewHeight)
          .environment(\.sizeCategory, previewSizeCategory)
      } else { applicationContent }
      #else
      applicationContent
      #endif
    }
    .onChange(of: scenePhase) { _, phase in
      guard phase == .active else { return }
      // 「水杉四季」跟随月份，所以启动或从后台返回时会重新解析季节；季节变化时发出 `AppThemePalette.didChange`。
      AppThemePalette.refresh()
      applyAppearance()
      // Sends what the keyboard queued, which it cannot send itself without Full Access.
      Task.detached(priority: .utility) { UsageReporting.flush() }
      // 与 Android 的 `HomeActivity.onResume` 相同：回到前台时按节流决定要不要跑一轮云同步。
      CloudSync.shared.onActive()
      Task.detached(priority: .utility) { _ = try? MetasequoiaInputSessionBridge.dictionaryUpdates("automatic") }
      // 打开了「上传到云端」时补传打字统计，十分钟内只真正发一次（节流在共享层）。
      Task.detached(priority: .utility) { await TypingUpload.shared.onActive() }
    }
  }

  /// 在每个窗口上应用「设置界面主题」（见 `AppAppearancePreference`）。用窗口级覆盖而不是 `preferredColorScheme`，这样切回「跟随系统」时能可靠地把样式交还给设备，sheet 也会跟着变。窗口的 tint 是季节强调色，所以 SwiftUI tint 管不到的 alert、菜单和系统选择器也跟随应用主题。
  private func applyAppearance() {
    let style = AppAppearancePreference.style(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
    for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
      for window in scene.windows {
        window.overrideUserInterfaceStyle = style
        window.tintColor = MetasequoiaTheme.accentUIColor
      }
    }
  }

  /// iOS 26 之前的经典导航栏和标签栏使用季节页面色：内容滚到栏下方后是盖在页面上的半透明栏加季节分隔线，静止时是透明栏，让大标题直接落在页面上；标签栏是不带顶部线的实色栏。iOS 26 及以后保留系统的 Liquid Glass，它本身已经符合设计稿。appearance 代理只作用于之后创建的栏，所以配色变化后要重建内容（`paletteVersion`）。
  private static func applyChrome() {
    guard #unavailable(iOS 26) else { return }
    let titleAttributes: [NSAttributedString.Key: Any] = [
      .font: UIFont.systemFont(ofSize: 17, weight: .semibold),
      .foregroundColor: UIColor.label,
    ]
    let bar = UINavigationBarAppearance()
    bar.configureWithDefaultBackground()
    bar.backgroundEffect = UIBlurEffect(style: .systemThinMaterial)
    bar.backgroundColor = UIColor { MetasequoiaTheme.canvasUIColor.resolvedColor(with: $0).withAlphaComponent(0.82) }
    bar.shadowColor = MetasequoiaTheme.hairUIColor
    bar.titleTextAttributes = titleAttributes
    let edge = UINavigationBarAppearance()
    edge.configureWithTransparentBackground()
    edge.titleTextAttributes = titleAttributes
    let navigationBar = UINavigationBar.appearance()
    navigationBar.standardAppearance = bar
    navigationBar.compactAppearance = bar
    navigationBar.scrollEdgeAppearance = edge
    navigationBar.compactScrollEdgeAppearance = edge

    let tabs = UITabBarAppearance()
    tabs.configureWithOpaqueBackground()
    tabs.backgroundColor = MetasequoiaTheme.canvasUIColor
    tabs.shadowColor = .clear
    let tabBar = UITabBar.appearance()
    tabBar.standardAppearance = tabs
    tabBar.scrollEdgeAppearance = tabs
  }

  #if DEBUG && targetEnvironment(simulator)
  private var previewHeight: CGFloat {
    ProcessInfo.processInfo.arguments.contains("-keyboardCompactPreview") ? 216 : 260
  }
  private var previewSizeCategory: ContentSizeCategory {
    ProcessInfo.processInfo.arguments.contains("-keyboardLargeType") ? .accessibilityExtraExtraExtraLarge : .large
  }
  private var previewText: String {
    ProcessInfo.processInfo.arguments.contains("-keyboardLongPreview")
      ? String(repeating: "用于检测滚动区的测试段落。", count: 50)
      : "这是一段待润色的测试文字。只有点击发送才会请求服务。"
  }
  #endif

  private var applicationContent: some View {
    FirstRunContainer(hasCompletedOnboarding: $hasCompletedOnboarding, navigation: tabNavigation, router: router)
      .environmentObject(onboardingNavigation)
      .toggleStyle(GreenSwitchToggleStyle())
      .id(paletteVersion)
      .overlay(alignment: .bottom) { ToastOverlay() }
      .modifier(CloudSyncFirstRunPrompt(sync: .shared))
      .onAppear(perform: applyAppearance)
    .onReceive(NotificationCenter.default.publisher(for: AppAppearancePreference.didChange)) { _ in applyAppearance() }
    .onReceive(NotificationCenter.default.publisher(for: AppThemePalette.didChange)) { _ in
      Self.applyChrome()
      applyAppearance()
      paletteVersion = AppThemePalette.version
    }
  }
}

#if DEBUG && targetEnvironment(simulator)
private struct KeyboardVoicePreviewFixture: View {
  @State private var entry = try? VoiceTextHandoffStore().read()
  @State private var inserted = ""
  @State private var closed = false
  var body: some View {
    if closed { Text(inserted.isEmpty ? "已关闭" : "插入验证：" + inserted) }
    else {
      KeyboardVoiceView(entry: entry, insert: {
        guard let entry else { throw VoiceTextHandoffStore.Failure.stale }
        inserted = try VoiceTextHandoffStore().consume(entry.id)
      }, close: { closed = true })
    }
  }
}
#endif

/// The splash and the onboarding sit in front of the tabs until onboarding is done. A phone shows the onboarding full screen; an iPad at regular width shows the tabs with the onboarding as a modal card over them, as the design does.
private struct FirstRunContainer: View {
  @Binding var hasCompletedOnboarding: Bool
  let navigation: AppNavigation
  let router: SettingsRouter
  @State private var showsSplash = true
  @Environment(\.horizontalSizeClass) private var widthClass

  private var isTablet: Bool { UIDevice.current.userInterfaceIdiom == .pad && widthClass == .regular }

  var body: some View {
    if !hasCompletedOnboarding && showsSplash {
      SplashView { showsSplash = false }
    } else if hasCompletedOnboarding || isTablet {
      // Hidden before the overlay is attached, so only the tabs leave the accessibility tree and the card stays in it.
      MainTabView(navigation: navigation, router: router)
        .accessibilityHidden(!hasCompletedOnboarding)
        .overlay {
          if !hasCompletedOnboarding { OnboardingModalCard { hasCompletedOnboarding = true } }
        }
    } else {
      WelcomeFlowView(onFinish: { hasCompletedOnboarding = true })
    }
  }
}

/// SwiftUI 在部分系统版本上不会把重复选择写回 Binding，转发系统回调以恢复设置根页。
private struct TabReselectionObserver: UIViewControllerRepresentable {
  let reselectSettings: () -> Void

  func makeUIViewController(context: Context) -> Probe {
    Probe(reselectSettings: reselectSettings)
  }
  func updateUIViewController(_ controller: Probe, context: Context) {
    controller.reselectSettings = reselectSettings
    controller.attachLater()
  }
  static func dismantleUIViewController(_ controller: Probe, coordinator: ()) { controller.detach() }

  final class Probe: UIViewController, UITabBarControllerDelegate {
    var reselectSettings: () -> Void
    private weak var observed: UITabBarController?
    private weak var originalDelegate: UITabBarControllerDelegate?

    init(reselectSettings: @escaping () -> Void) {
      self.reselectSettings = reselectSettings
      super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func viewDidAppear(_ animated: Bool) {
      super.viewDidAppear(animated)
      attach()
    }
    override func viewDidLayoutSubviews() {
      super.viewDidLayoutSubviews()
      attach()
    }
    override func didMove(toParent parent: UIViewController?) {
      super.didMove(toParent: parent)
      if parent == nil { detach() }
      else { attachLater() }
    }
    func attachLater() {
      DispatchQueue.main.async { [weak self] in self?.attach() }
    }
    func attach() {
      guard let controller = tabBarController, controller.delegate !== self else { return }
      detach()
      observed = controller
      originalDelegate = controller.delegate
      controller.delegate = self
    }
    func detach() {
      if observed?.delegate === self { observed?.delegate = originalDelegate }
      observed = nil
      originalDelegate = nil
    }
    func tabBarController(_ controller: UITabBarController, shouldSelect viewController: UIViewController) -> Bool {
      let reselectedSettings = controller.selectedIndex == 0 && controller.selectedViewController === viewController
      let allowed = originalDelegate?.tabBarController?(controller, shouldSelect: viewController) ?? true
      // 重选已经显示的标签不需要一次新的选择，即使 SwiftUI 拒绝重复选择也要恢复根页。
      if reselectedSettings { reselectSettings() }
      return allowed
    }
    // 其余可选回调交还 SwiftUI 原来的委托，保留系统选择、动画及适配行为。
    override func responds(to selector: Selector!) -> Bool {
      super.responds(to: selector) || originalDelegate?.responds(to: selector) == true
    }
    override func forwardingTarget(for selector: Selector!) -> Any? {
      originalDelegate?.responds(to: selector) == true ? originalDelegate : super.forwardingTarget(for: selector)
    }
  }
}

private struct MainTabView: View {
  @ObservedObject var navigation: AppNavigation
  @ObservedObject var router: SettingsRouter
  @Environment(\.horizontalSizeClass) private var widthClass
  var body: some View {
    // On iOS 26 and later the system draws this as the floating glass pill; earlier releases keep the classic bar.
    TabView(selection: Binding(get: { navigation.tab }, set: { navigation.select($0, router: router) })) {
      settingsTab
        .background(TabReselectionObserver {
          navigation.select(.settings, router: router)
        }.frame(width: 0, height: 0))
        .tabItem { Label("首页", systemImage: "house.fill") }.tag(AppNavigation.Tab.settings)
      NavigationStack { CommunityHomeView() }.id(navigation.communityRoot)
        .tabItem { Label("社区", systemImage: "globe.asia.australia.fill") }.tag(AppNavigation.Tab.community)
      NavigationStack { TypingStatisticsView() }
        .tabItem { Label("统计", systemImage: "chart.bar.fill") }.tag(AppNavigation.Tab.statistics)
      NavigationStack { AccountSettingsView() }
        .tabItem { Label("我的", systemImage: "person.fill") }.tag(AppNavigation.Tab.account)
    }
    .environmentObject(navigation)
    .environmentObject(router)
    .tint(MetasequoiaTheme.accent)
    // 键盘发来的本版本 URL scheme（`MSIMEAppEdition.urlScheme`，full 是 msime://）：settings[/页面]、feedback、about、voice、voice-session。不加这一条应用照样会被拉起来,但会停在上次离开的那个标签页 —— 用户是从键盘面板点过来的,落点应该是对应的页面；认不出的链接落在设置。
    .onOpenURL { url in
      #if canImport(GoogleSignIn)
      // Google 登录的回调（scheme 是倒序的 iOS client ID）交还给 GoogleSignIn；它不认的链接返回 false。
      if GIDSignIn.sharedInstance.handle(url) { return }
      #endif
      guard url.scheme == MSIMEAppEdition.urlScheme else { return }
      if url.host == "voice-session" {
        openVoiceSession()
        return
      }
      // 链接指名了设置页时先把设置栈重建回根页面：切走标签页时那条栈原样保留，同一页面已经开着时写入相同的值不会触发导航，用户会停在上次在那里打开的更深一层页面，而不是链接要的页面。
      if SettingsRouter.settingsPage(in: url) != nil {
        navigation.settingsRoot = UUID()
        navigation.settingsChat = false
      }
      navigation.tab = router.handle(url) ?? .settings
      if url.host == "voice" { navigation.recordsVoice = true }
    }
    .sheet(isPresented: $navigation.recordsVoice) {
      NavigationView {
        ServiceSettingsView(kind: .voice, autoStartVoice: true)
          .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("完成") { navigation.recordsVoice = false } }
          }
      }.navigationViewStyle(.stack).tint(MetasequoiaTheme.accent)
    }
    // 全屏而不是 sheet：左上角要留给指向系统「◀︎ 返回」的气泡，页面自己带「完成」。
    .fullScreenCover(isPresented: $navigation.opensVoiceSession) { VoiceSessionView() }
  }

  /// 键盘长按空格时还没有语音会话。从别的应用跳过来时盖上语音会话页，告诉用户回去说话；用户正在水杉 App 自己的输入框里打字时（App 本来就在前台）不用离开，也不切标签页，直接开好会话，只有失败时才盖上页面说明原因。
  private func openVoiceSession() {
    guard UIApplication.shared.applicationState == .active else {
      navigation.opensVoiceSession = true
      return
    }
    Task { @MainActor in
      await VoiceSessionService.shared.activate()
      if VoiceSessionService.shared.status == .ready {
        ToastCenter.shared.show("语音已开启，按住空格说话")
      } else {
        navigation.opensVoiceSession = true
      }
    }
  }

  // Both idiom and width class: a Max-size iPhone turned sideways is regular width but stays a phone, and an iPad in a narrow Split View or Slide Over pane gets the phone's stack.
  @ViewBuilder private var settingsTab: some View {
    if UIDevice.current.userInterfaceIdiom == .pad && widthClass == .regular {
      TabletSettingsView().id(navigation.settingsRoot)
    }
    else {
      NavigationStack {
        SettingsView(navigationRoot: navigation.settingsRoot)
          .navigationDestination(isPresented: $navigation.settingsChat) {
            KeyboardTryoutView(title: "AI 对话", focusRequest: navigation.chatFocusRequest)
          }
      }
      .id(navigation.settingsRoot)
    }
  }
}

#if DEBUG && targetEnvironment(simulator)
private struct LaunchScreenPreview: UIViewControllerRepresentable {
  func makeUIViewController(context: Context) -> UIViewController {
    UIStoryboard(name: "LaunchScreen", bundle: .main).instantiateInitialViewController()!
  }
  func updateUIViewController(_ controller: UIViewController, context: Context) {}
}
#endif

#if DEBUG && targetEnvironment(simulator)
private struct ReplyKeyboardPreview: View {
  @StateObject private var model = ReplyKeyboardModel()
  var body: some View {
    ReplyKeyboardView(model: model, paste: { model.setText("你睡了吗") }, generate: { style in
      model.generate(style: style, request: { _, _ in "还没呢，正好想和你聊聊。" }, insert: { _ in true })
    })
  }
}
#endif

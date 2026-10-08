import SwiftUI

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
    // The device's anonymous MSIME account is registered on first launch, before the keyboard needs it. The identity and session live in the app group, so the keyboard reuses them. A saved signed-in or anonymous session, even an expired one, skips the request rather than refreshing it on every launch; a failure is retried on the next launch.
    Task.detached(priority: .utility) {
      if (try? BackendKeychain().load()) != nil { return }
      if (try? BackendAnonymousAccount.sessionStorage().load()) != nil { return }
      _ = try? await BackendAnonymousAccount.ensureSignedIn(session: BackendAnonymousAccount.session, client: BackendAccountClient())
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
    .onChange(of: scenePhase) { phase in
      guard phase == .active else { return }
      // 「水杉四季」跟随月份，所以启动或从后台返回时会重新解析季节；季节变化时发出 `AppThemePalette.didChange`。
      AppThemePalette.refresh()
      applyAppearance()
      // Sends what the keyboard queued, which it cannot send itself without Full Access.
      Task.detached(priority: .utility) { UsageReporting.flush() }
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

private struct MainTabView: View {
  @ObservedObject var navigation: AppNavigation
  @ObservedObject var router: SettingsRouter
  @Environment(\.horizontalSizeClass) private var widthClass
  var body: some View {
    // On iOS 26 and later the system draws this as the floating glass pill; earlier releases keep the classic bar.
    TabView(selection: Binding(get: { navigation.tab }, set: { navigation.select($0, router: router) })) {
      settingsTab.id(navigation.settingsRoot)
        .tabItem { Label("设置", systemImage: "gearshape.fill") }.tag(AppNavigation.Tab.settings)
      NavigationStack { CommunityHomeView() }.id(navigation.communityRoot)
        .tabItem { Label("社区", systemImage: "person.3.fill") }.tag(AppNavigation.Tab.community)
      NavigationStack { TypingStatisticsView() }
        .tabItem { Label("统计", systemImage: "chart.bar.fill") }.tag(AppNavigation.Tab.statistics)
      NavigationStack { AccountSettingsView() }
        .tabItem { Label("我的", systemImage: "person.fill") }.tag(AppNavigation.Tab.account)
    }
    .environmentObject(navigation)
    .environmentObject(router)
    .tint(MetasequoiaTheme.accent)
    // 键盘发来的本版本 URL scheme（`MSIMEAppEdition.urlScheme`，full 是 msime://）：settings[/页面]、feedback、about、voice。不加这一条应用照样会被拉起来,但会停在上次离开的那个标签页 —— 用户是从键盘面板点过来的,落点应该是对应的页面；认不出的链接落在设置。
    .onOpenURL { url in
      guard url.scheme == MSIMEAppEdition.urlScheme else { return }
      navigation.tab = router.handle(url) ?? .settings
      if url.host == "voice" { navigation.recordsVoice = true }
    }
    .sheet(isPresented: $navigation.recordsVoice) {
      NavigationView {
        ServiceSettingsView(kind: .voice)
          .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("完成") { navigation.recordsVoice = false } }
          }
      }.navigationViewStyle(.stack).tint(MetasequoiaTheme.accent)
    }
  }

  // Both idiom and width class: a Max-size iPhone turned sideways is regular width but stays a phone, and an iPad in a narrow Split View or Slide Over pane gets the phone's stack.
  @ViewBuilder private var settingsTab: some View {
    if UIDevice.current.userInterfaceIdiom == .pad && widthClass == .regular { TabletSettingsView() }
    else { NavigationStack { SettingsView() } }
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

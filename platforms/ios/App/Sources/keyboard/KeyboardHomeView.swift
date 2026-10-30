import SwiftUI
import UIKit

/// 手机上的 设置 标签页：搜索框、键盘状态卡片，然后是按移动端设计稿分组卡片排列的各设置页。各行来自 SettingsPage，iPad 侧边栏也读同一份。
struct SettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @EnvironmentObject private var router: SettingsRouter
  @EnvironmentObject private var navigation: AppNavigation
  @State private var scheme = InputSchemePreference.scheme
  @State private var skin = KeyboardTheme.current
  @State private var values: [SettingsPage: String] = [:]
  @State private var query = ""
  @State private var selectedPage: SettingsPage?
  /// The notices not yet dismissed, newest first; the newest is shown.
  @State private var notices: [AppNotice] = []

  private var skinName: String {
    guard let design = skin.design else { return skin.title }
    return CustomSkinLibrary.designs.first { $0.design == design }?.name ?? skin.title
  }

  /// 深链接是待消费的请求。栈上已经推入了页面时先弹回根页面，等根页面重新出现（`onAppear`）再推入要的页面：同一页面已经开着时写入相同的值不会触发导航，用户会停在上次在那里打开的更深一层页面。不靠换栈（`.id`）回到根页面，因为 iOS 18 上换掉一条推入过页面的 NavigationStack 后，新栈的 `navigationDestination` 不再推入任何页面。
  private func consumeRequestedPage() {
    guard let page = router.settingsPage else { return }
    if selectedPage != nil {
      selectedPage = nil
      return
    }
    router.settingsPage = nil
    selectedPage = page
  }

  var body: some View {
    let groups = SettingsPage.matching(query)
    ScrollView {
      VStack(spacing: 20) {
        SettingsSearchPill(query: $query)
        if query.isEmpty {
          if let notice = notices.first {
            NoticeBanner(notice: notice) {
              notices.removeFirst()
              Task.detached(priority: .utility) { AppNotices.dismiss(notice.id) }
            }
          }
          KeyboardStatusCard(scheme: scheme, tryout: navigation.openChat)
        }
        ForEach(Array(groups.enumerated()), id: \.offset) { _, group in
          DesignCard(radius: MetasequoiaTheme.cardRadius) {
            ForEach(group) { page in
              SettingsCardRow(page: page, value: value(for: page), isLast: page == group.last, selection: $selectedPage)
            }
          }
        }
      }
      .padding(.horizontal, 16)
      .padding(.bottom, 20)
    }
    .scrollDismissesKeyboard(.interactively)
    .designCanvasBackground()
    .overlay {
      if groups.isEmpty { ContentUnavailableView.search(text: query) }
    }
    // 「首页」标签的根页以应用名作大标题，与共享设置组件的 `mobilePageTitle("home")` 一致。
    .navigationTitle("水杉输入法")
    .navigationBarTitleDisplayMode(.large)
    // 不按当前选中的标签页过滤：切走时读到 nil 会把推入的页面弹掉，切回来再重新推入，页面里更深一层的导航就丢了。
    .navigationDestination(item: $selectedPage) { $0.destination }
    .onAppear { refresh(); consumeRequestedPage() }
    .onChange(of: router.settingsPage) { _, _ in consumeRequestedPage() }
    // 重选「首页」：收起根页面推入的页面，它上面更深的页面随之一起弹出。
    .onChange(of: navigation.settingsReselection) { _, _ in selectedPage = nil }
    .onChange(of: scenePhase) { _, phase in if phase == .active { refresh(); Task { await loadNotices() } } }
    .task { await loadNotices() }
    .tint(MetasequoiaTheme.accent)
  }

  private func value(for page: SettingsPage) -> String? {
    page == .skin ? skinName : values[page]
  }

  @MainActor private func loadNotices() async {
    notices = await Task.detached(priority: .utility) { AppNotices.load() }.value
  }

  private func refresh() {
    let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
    InputSchemePreference.mirror(preferences)
    TouchShuangpinLayoutPreference.mirror(preferences)
    TouchWubiLayoutPreference.mirror(preferences)
    scheme = InputSchemePreference.scheme
    skin = KeyboardTheme.reload(preferences)
    values = Self.rowValues(scheme: scheme, preferences: preferences)
  }

  /// 根级各行尾部显示的值，从共享文档（键盘的唯一事实来源）读取，读不到时回退到 App Group 镜像。词库 显示学习开关，与 Android 的 记忆新词 一样，因为没有查询词条数的 ABI；开发者选项 不显示值。
  private static func rowValues(scheme: ChineseInputScheme, preferences: [String: Any]?) -> [SettingsPage: String] {
    // `punctuation_lock` 不是跟随时会在键盘里覆盖 `chinese_punctuation`，与 表达 页的显示一致。
    let chinesePunctuation: Bool
    switch preferences?["punctuation_lock"] as? String {
    case "chinese": chinesePunctuation = true
    case "english": chinesePunctuation = false
    default: chinesePunctuation = preferences?["chinese_punctuation"] as? Bool ?? true
    }
    let learning = InputHabitPreference.settings(in: preferences).learning
    let grid = LetterKeyGrid.of(scheme, shuangpinLayout: TouchShuangpinLayoutPreference.layout(in: preferences),
                                wubiLayout: TouchWubiLayoutPreference.layout(in: preferences))
    let keys = scheme.isMandarinNineKey || scheme == .japaneseNineKey ? "9 键" : grid?.title ?? "26 键"
    let height = KeyboardLayoutPreference.sharedHeightAdjustment(preferences?["touch_keyboard_height_adjustment"])
      ?? KeyboardLayoutPreference.heightAdjustment
    let language = VoicePolishSettings(preferences).language
    return [
      .input: scheme.shortLabel,
      .ai: chinesePunctuation ? "中文标点" : "英文标点",
      .dictionary: learning ? "学习常用词 已开" : "学习常用词 已关",
      .layout: keys + (height == 0 ? " · 标准高度" : " · 已调高度"),
      .voice: VoicePolishSettings.languages.first { $0.id == language }?.title ?? "",
      .handwriting: InputSchemePreference.enabledSchemes.contains(.handwriting) ? "已开启" : "未开启",
      .toolbar: TouchToolbarLocalPreference.hidden ? "隐藏" : "输入时显示",
    ]
  }
}

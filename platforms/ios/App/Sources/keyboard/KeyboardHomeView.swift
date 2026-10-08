import SwiftUI
import UIKit

/// 手机上的 设置 标签页：搜索框、键盘状态卡片，然后是按移动端设计稿分组卡片排列的各设置页。各行来自 SettingsPage，iPad 侧边栏也读同一份。
struct SettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @EnvironmentObject private var router: SettingsRouter
  @State private var scheme = InputSchemePreference.scheme
  @State private var skin = KeyboardTheme.current
  @State private var values: [SettingsPage: String] = [:]
  @State private var query = ""
  /// The notices not yet dismissed, newest first; the newest is shown.
  @State private var notices: [AppNotice] = []

  private var skinName: String {
    guard let design = skin.design else { return skin.title }
    return CustomSkinLibrary.designs.first { $0.design == design }?.name ?? skin.title
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
          KeyboardStatusCard(scheme: scheme, tryout: .push)
        }
        ForEach(Array(groups.enumerated()), id: \.offset) { _, group in
          DesignCard(radius: MetasequoiaTheme.cardRadius) {
            ForEach(group) { page in
              SettingsCardRow(page: page, value: value(for: page), isLast: page == group.last)
            }
          }
        }
      }
      .padding(.horizontal, 16)
      .padding(.bottom, 20)
    }
    .scrollDismissesKeyboard(.interactively)
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .overlay {
      if groups.isEmpty { ContentUnavailableView.search(text: query) }
    }
    .navigationTitle("设置")
    .navigationBarTitleDisplayMode(.large)
    .navigationDestination(item: $router.settingsPage) { $0.destination }
    .onAppear { refresh() }
    .onChange(of: scenePhase) { if $0 == .active { refresh(); Task { await loadNotices() } } }
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
    let keys = scheme == .nineKey || scheme == .japaneseNineKey ? "9 键" : "26 键"
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

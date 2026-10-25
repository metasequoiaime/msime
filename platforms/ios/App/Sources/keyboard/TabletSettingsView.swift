import SwiftUI
import UIKit

/// The 设置 tab at regular width (iPad full screen or a wide Split View pane).
///
/// 手机从分组卡片沿一条导航栈逐层进入。在 iPad 上那个列表会横跨整个屏幕，每进一页都会把它替换掉，所以设计稿把它拆开：左边是 320pt 的侧栏，放搜索框、状态卡和与手机相同的分组，选中的页面放在旁边一列，宽度不超过 720pt。侧栏行不显示取值也不显示箭头，图标使用文字颜色。
struct TabletSettingsView: View {
  enum Destination: Hashable {
    case page(SettingsPage)

    var identifier: String {
      switch self {
      case .page(let page): return "tabletSettings.\(page.rawValue)"
      }
    }
  }

  @EnvironmentObject private var router: SettingsRouter
  @EnvironmentObject private var navigation: AppNavigation
  @State private var columns = NavigationSplitViewVisibility.all
  @State private var scheme = InputSchemePreference.scheme
  @State private var query = ""

  /// 侧栏的选中项，保存在 `AppNavigation` 里。
  private var selection: Destination? { navigation.tabletSettingsSelection }

  var body: some View {
    NavigationSplitView(columnVisibility: $columns) {
      sidebar
        .navigationSplitViewColumnWidth(320)
    } detail: {
      // 每个分组重建导航栈，切换后不留下上一分组的子页面。
      NavigationStack {
        Group {
          if navigation.settingsChat {
            KeyboardTryoutView(title: "AI 对话", focusRequest: navigation.chatFocusRequest)
              .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                  Button("返回") { navigation.settingsChat = false }
                }
              }
          } else {
            detail(selection ?? .page(.input))
          }
        }
          .frame(maxWidth: 720)
          .frame(maxWidth: .infinity)
          .designCanvasBackground()
          // 设计稿给详情页一个靠前对齐的大标题，而不是居中的内联标题栏。
          .toolbarTitleDisplayMode(.large)
      }.id(selection)
    }
    .navigationSplitViewStyle(.balanced)
    .tint(MetasequoiaTheme.accent)
    .reloadOnAppearAndActive {
      scheme = InputSchemePreference.scheme
      show(router.settingsPage)
    }
    .onChange(of: navigation.tabletSettingsSelection) { navigation.settingsChat = false }
    .onChange(of: router.settingsPage) { _, page in show(page) }
  }

  /// 选中深链接要求的页面并清掉路由，因为侧栏选中项此后已经记着它。
  private func show(_ page: SettingsPage?) {
    guard let page else { return }
    query = ""
    navigation.tabletSettingsSelection = .page(page)
    router.settingsPage = nil
  }

  private var sidebar: some View {
    let groups = SettingsPage.matching(query)
    return List(selection: $navigation.tabletSettingsSelection) {
      Section {
        SettingsSearchPill(query: $query)
          .listRowInsets(EdgeInsets(top: 0, leading: 0, bottom: 0, trailing: 0))
          .listRowBackground(Color.clear)
          .listRowSeparator(.hidden)
        if query.isEmpty {
          KeyboardStatusCard(scheme: scheme, tryout: navigation.openChat)
            .listRowInsets(EdgeInsets(top: 20, leading: 0, bottom: 0, trailing: 0))
            .listRowBackground(Color.clear)
            .listRowSeparator(.hidden)
        }
      }
      ForEach(Array(groups.enumerated()), id: \.offset) { _, group in
        Section {
          ForEach(group) { page in row(page) }
        }
      }
    }
    .listStyle(.insetGrouped)
    .scrollContentBackground(.hidden)
    .designCanvasBackground()
    .listSectionSpacing(20)
    .environment(\.defaultMinListRowHeight, 44)
    .scrollDismissesKeyboard(.interactively)
    .overlay {
      if groups.isEmpty { ContentUnavailableView.search(text: query) }
    }
    .navigationTitle("水杉输入法")
  }

  private func row(_ page: SettingsPage) -> some View {
    let destination = Destination.page(page)
    // A selection list's cell swallows identifiers set inside its row, so the row is made one element that carries the identifier itself.
    return SettingsNavLabel(title: page.title, symbol: page.symbol, iconColor: .primary)
      .frame(minHeight: 44)
      .accessibilityElement(children: .combine)
      .accessibilityAddTraits(.isButton)
      .accessibilityIdentifier(destination.identifier)
      .listRowBackground(ZStack {
        MetasequoiaTheme.surface
        if selection == destination { MetasequoiaTheme.hoverFill }
      })
      .listRowSeparatorTint(MetasequoiaTheme.hair)
      .tag(destination)
  }

  @ViewBuilder private func detail(_ destination: Destination) -> some View {
    switch destination {
    case .page(let page): page.destination
    }
  }
}

import SwiftUI
import UIKit

/// The 设置 tab at regular width (iPad full screen or a wide Split View pane).
///
/// A phone walks one stack from the grouped list. On an iPad that list would stretch across the whole screen and every page would replace it, so the design splits it: a 320pt sidebar with the status card, search and the same groups as the phone, and the selected page beside it in a column no wider than 720pt.
struct TabletSettingsView: View {
  enum Destination: Hashable {
    case tryout
    case page(SettingsPage)

    var identifier: String {
      switch self {
      case .tryout: return "tabletSettings.tryout"
      case .page(let page): return "tabletSettings.\(page.rawValue)"
      }
    }
  }

  @Environment(\.scenePhase) private var scenePhase
  @State private var selection: Destination? = .page(.skin)
  @State private var columns = NavigationSplitViewVisibility.all
  @State private var scheme = InputSchemePreference.scheme
  @State private var query = ""

  var body: some View {
    NavigationSplitView(columnVisibility: $columns) {
      sidebar
        .navigationSplitViewColumnWidth(320)
    } detail: {
      // A fresh stack per section, so switching sections never leaves a page from the previous one on top.
      NavigationStack {
        detail(selection ?? .page(.skin))
          .frame(maxWidth: 720)
          .frame(maxWidth: .infinity)
          .background(Color(uiColor: .systemGroupedBackground).ignoresSafeArea())
      }.id(selection)
    }
    .navigationSplitViewStyle(.balanced)
    .tint(MetasequoiaTheme.accent)
    .onAppear { scheme = InputSchemePreference.scheme }
    .onChange(of: scenePhase) { if $0 == .active { scheme = InputSchemePreference.scheme } }
  }

  private var sidebar: some View {
    let groups = SettingsPage.matching(query)
    return List(selection: $selection) {
      if query.isEmpty {
        Section {
          KeyboardStatusCard(scheme: scheme)
          row("试用键盘", symbol: "text.cursor", destination: .tryout)
        }
      }
      ForEach(Array(groups.enumerated()), id: \.offset) { _, group in
        Section {
          ForEach(group) { page in row(page.title, symbol: page.symbol, destination: .page(page)) }
        }
      }
    }
    .listStyle(.insetGrouped)
    .environment(\.defaultMinListRowHeight, 44)
    .overlay {
      if groups.isEmpty { ContentUnavailableView.search(text: query) }
    }
    .searchable(text: $query, placement: .sidebar, prompt: "搜索设置")
    .navigationTitle("设置")
  }

  private func row(_ title: String, symbol: String, destination: Destination) -> some View {
    // A selection list's cell swallows identifiers set inside its row, so the row is made one element that carries the identifier itself.
    SettingsNavLabel(title: title, symbol: symbol)
      .accessibilityElement(children: .combine)
      .accessibilityAddTraits(.isButton)
      .accessibilityIdentifier(destination.identifier)
      .tag(destination)
  }

  @ViewBuilder private func detail(_ destination: Destination) -> some View {
    switch destination {
    case .tryout: KeyboardTryoutView(focusOnAppear: true)
    case .page(let page): page.destination
    }
  }
}

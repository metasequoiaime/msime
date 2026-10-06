import SwiftUI
import UIKit

/// The 设置 tab on a phone: the keyboard status card, then the settings pages in the grouped list of the mobile design. The rows come from SettingsPage, which the iPad sidebar reads as well.
struct SettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var scheme = InputSchemePreference.scheme
  @State private var skin = KeyboardTheme.current
  /// The candidate font size the keyboard draws (dc.html NAV_VAL: 候选栏 shows it as "18px").
  @State private var candidateSize = CandidateFontPreference.defaultCandidateSize
  @State private var query = ""
  /// The notices not yet dismissed, newest first; the newest is shown.
  @State private var notices: [AppNotice] = []

  private var skinName: String {
    guard let design = skin.design else { return skin.title }
    return CustomSkinLibrary.designs.first { $0.design == design }?.name ?? skin.title
  }

  var body: some View {
    let groups = SettingsPage.matching(query)
    List {
      if query.isEmpty {
        if let notice = notices.first {
          Section {
            NoticeBanner(notice: notice) {
              notices.removeFirst()
              Task.detached(priority: .utility) { AppNotices.dismiss(notice.id) }
            }
          }
        }
        Section {
          KeyboardStatusCard(scheme: scheme)
          NavigationLink(destination: KeyboardTryoutView(focusOnAppear: true)) {
            SettingsNavLabel(title: "试用键盘", symbol: "text.cursor")
          }.accessibilityIdentifier("keyboardTryoutLink")
        }
      }
      ForEach(Array(groups.enumerated()), id: \.offset) { _, group in
        Section {
          ForEach(group) { page in
            NavigationLink(destination: page.destination) {
              SettingsNavLabel(title: page.title, symbol: page.symbol, value: value(for: page))
            }
            .accessibilityIdentifier(page.linkIdentifier)
            .accessibilityValue(value(for: page) ?? "")
          }
        }
      }
    }
    .listStyle(.insetGrouped)
    .environment(\.defaultMinListRowHeight, 52)
    .overlay {
      if groups.isEmpty { ContentUnavailableView.search(text: query) }
    }
    .navigationTitle("设置")
    .navigationBarTitleDisplayMode(.large)
    .searchable(text: $query, prompt: "搜索设置")
    .onAppear { refresh() }
    .onChange(of: scenePhase) { if $0 == .active { refresh(); Task { await loadNotices() } } }
    .task { await loadNotices() }
    .tint(MetasequoiaTheme.accent)
  }

  private func value(for page: SettingsPage) -> String? {
    switch page {
    case .skin: return skinName
    case .input: return scheme.title
    case .candidate: return "\(candidateSize)px"
    default: return nil
    }
  }

  @MainActor private func loadNotices() async {
    notices = await Task.detached(priority: .utility) { AppNotices.load() }.value
  }

  private func refresh() {
    scheme = InputSchemePreference.scheme
    let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
    skin = KeyboardTheme.reload(preferences)
    candidateSize = CandidateFontPreference.candidateSize(in: preferences, tablet: UIDevice.current.userInterfaceIdiom == .pad)
  }
}

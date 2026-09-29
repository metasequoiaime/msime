import SwiftUI

// Each tab owns its stack. Cross-tab discovery always opens the community root.
final class AppNavigation: ObservableObject {
  enum Tab: Hashable { case settings, community, statistics, account }
  @Published var tab: Tab = .settings
  @Published var communityCategory = 0
  @Published var communityRoot = UUID()
  /// The keyboard's voice panel sends msime://voice: the keyboard cannot record, so the app opens straight onto the recording screen.
  @Published var recordsVoice = false

  func discoverSkins() {
    communityRoot = UUID()
    communityCategory = 0
    tab = .community
  }
}

// Authentication is a task sheet, never another copy of the My tab.
struct AccountLoginSheet: View {
  @Environment(\.dismiss) private var dismiss
  @State private var signedIn = false
  var body: some View {
    NavigationView {
      Form { AppleAccountSection(signedIn: $signedIn) }
        .navigationTitle("登录水杉").navigationBarTitleDisplayMode(.inline)
        .toolbar {
          ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } }
        }
        .onChange(of: signedIn) { if $0 { dismiss() } }
    }.navigationViewStyle(.stack).tint(MetasequoiaTheme.accent)
  }
}

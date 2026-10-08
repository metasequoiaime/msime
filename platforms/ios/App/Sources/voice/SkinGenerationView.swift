import SwiftUI

struct SavedSkinPublishFlow: View {
  let skinID: UUID
  @State private var signedIn = false
  @State private var loading = true
  @Environment(\.dismiss) private var dismiss
  var body: some View {
    Group {
      if loading { ProgressView("正在准备发布…") }
      else if signedIn { CommunityPublishView(onPublished: {}, selectedSkinID: skinID) }
      else {
        NavigationView {
          Form { AppleAccountSection(signedIn: $signedIn) }
            .navigationTitle("登录后发布").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } } }
        }.navigationViewStyle(.stack)
      }
    }.task {
      #if DEBUG && targetEnvironment(simulator)
      if ProcessInfo.processInfo.arguments.contains("-skinGenerationFixture") { signedIn = true; loading = false; return }
      #endif
      signedIn = (try? await SkinCommunityAPI.shared.signedIn()) == true
      loading = false
    }
  }
}

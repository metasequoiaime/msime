import SwiftUI

/// 页面出现和应用回到前台时重新读取本地设置；出现时可先认领请求状态。
struct SettingsScenePhaseReload: ViewModifier {
  @Environment(\.scenePhase) private var scenePhase
  let reload: () -> Void
  let prepare: (() -> Void)?

  func body(content: Content) -> some View {
    content
      .onAppear {
        prepare?()
        reload()
      }
      .onChange(of: scenePhase) { _, phase in
        if phase == .active { reload() }
      }
  }
}

extension View {
  func reloadOnAppearAndActive(_ reload: @escaping () -> Void) -> some View {
    modifier(SettingsScenePhaseReload(reload: reload, prepare: nil))
  }

  func reloadOnAppearAndActive(
    _ reload: @escaping () -> Void,
    onAppear prepare: @escaping () -> Void
  ) -> some View {
    modifier(SettingsScenePhaseReload(reload: reload, prepare: prepare))
  }
}

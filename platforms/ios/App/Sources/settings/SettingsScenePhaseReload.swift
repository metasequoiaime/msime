import SwiftUI

/// 页面出现和应用回到前台时重新读取本地设置。
struct SettingsScenePhaseReload: ViewModifier {
  @Environment(\.scenePhase) private var scenePhase
  let reload: () -> Void

  func body(content: Content) -> some View {
    content
      .onAppear(perform: reload)
      .onChange(of: scenePhase) { _, phase in
        if phase == .active { reload() }
      }
  }
}

extension View {
  func reloadOnAppearAndActive(_ reload: @escaping () -> Void) -> some View {
    modifier(SettingsScenePhaseReload(reload: reload))
  }

  func reloadOnAppearAndActive(
    _ reload: @escaping () -> Void,
    onAppear prepare: @escaping () -> Void
  ) -> some View {
    modifier(SettingsScenePhaseReloadWithPreparation(reload: reload, prepare: prepare))
  }
}

/// 页面出现时先执行请求状态认领，再加载；回到前台时只重新加载。
private struct SettingsScenePhaseReloadWithPreparation: ViewModifier {
  @Environment(\.scenePhase) private var scenePhase
  let reload: () -> Void
  let prepare: () -> Void

  func body(content: Content) -> some View {
    content
      .onAppear {
        prepare()
        reload()
      }
      .onChange(of: scenePhase) { _, phase in
        if phase == .active { reload() }
      }
  }
}

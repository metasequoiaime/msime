import Foundation

@MainActor
enum BackendClipboardEntry {
  /// 配置了自建中继时为中继打开云剪贴板，否则为已登录的账号打开；没有登录时请用户登录，选路失败时同样退回登录窗口（`BackendCloudClipboardRoute.selected` 只在状态目录读不了、偏好读不出来时抛错，正常配置的偏好由设置页先校验）。`route` 是 [`BackendCloudClipboardRoute.selected`]。
  static func open(route: () async throws -> BackendCloudClipboardRoute?,
                   present: (BackendCloudClipboardRoute) -> Void, signIn: () -> Void) async {
    do {
      guard let route = try await route() else { signIn(); return }
      try Task.checkCancellation()
      present(route)
    } catch is CancellationError {
    } catch {
      signIn()
    }
  }
}

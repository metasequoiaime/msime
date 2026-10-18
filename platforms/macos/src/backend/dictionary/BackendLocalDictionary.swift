import Foundation

/// 本进程里的输入会话与本机词库，给 Rust 的快照队列（`snapshot_preview`、`snapshot_enqueue`、`snapshot_status`）用：账号窗口的「云词库同步与备份」和 Tauri 面板的「应用到本机」都经这里。
@MainActor
enum MacLocalDictionary {
  /// 活动输入会话的 HostOptions 文档（`MSIMEClientSession.activeHostOptions`）。独立设置窗口进程和还没打过字的输入法进程没有输入会话，是 `snapshot_unavailable`。
  static func hostOptions() throws -> [String: Any] {
    guard let type = NSClassFromString("MSIMEClientSession") as? NSObject.Type,
          let result = type.perform(NSSelectorFromString("activeHostOptions"))?.takeUnretainedValue() as? [String: Any],
          result["error"] == nil else {
      throw BackendRustClient.Failure(code: "snapshot_unavailable")
    }
    return result
  }

  /// 在 `body` 进行期间每 250 毫秒叫醒一次本进程里握着词库的输入会话（`InputController.mm` 的 `dictionaryMaintenanceWillBegin:`，它只在词库维护租约有效时放手）。Rust 处理队列时写租约、不发通知，而输入法没有获得焦点的输入框时每秒检查租约的计时器不跑，会话就一直握着词库，请求只能留在队列里。
  static func wakingInputSessions<T>(_ body: () async throws -> T) async throws -> T {
    let ticker = Task { @MainActor in
      while !Task.isCancelled {
        NotificationCenter.default.post(name: BackendEdition.notificationName("MSIMEDictionaryMaintenanceWillBeginNotification"), object: nil)
        try? await Task.sleep(nanoseconds: 250_000_000)
      }
    }
    defer { ticker.cancel() }
    return try await body()
  }
}

import Foundation

/// 云剪贴板页的一次刷新经 Rust 的 `msime_client_account_cloud_clipboard`（#8201 迁移计划 4c）：先做页面要的操作，再读列表，云剪贴板开着、而那个操作没有带回设备状态时再读一次设备状态。每次请求都带本机的设备名（P6），只在本机还没登记端到端加密时发给服务端，给其他设备的设备列表看。
///
/// 页面只按第一次读到的 user.id 绑定：C 上拿不到会话代次，同一个账号的令牌轮换和重新登录不作废页面；请求前后都核对，退出或换了号时是 `CancellationError`。
struct IOSCloudClipboard: Sendable {
  let account: BackendRustAccount
  let deviceName: String

  /// 发给 Rust 的一个云剪贴板 action。
  typealias Request = @Sendable ([String: Any]) async throws -> Data

  /// 一次刷新的结果。
  struct Loaded: Sendable {
    let accountID: String
    let page: BackendAccountClient.ClipboardPage
    /// 云剪贴板关着时为 nil。
    let devices: BackendAccountClient.ClipboardDevices?
  }

  /// `accountID` 是页面已经绑定的账号，第一次刷新时为 nil。
  func refresh(boundTo accountID: String?, search: String,
               _ action: (Request) async throws -> BackendAccountClient.ClipboardDevices?) async throws -> Loaded {
    let account = self.account, deviceName = self.deviceName
    let send: Request = { action in try await account.cloudClipboard(action, deviceName: deviceName) }
    guard let current = try await account.status()?.id, accountID == nil || accountID == current else { throw CancellationError() }
    var devices = try await action(send)
    let page = try BackendRustAccount.decode(BackendAccountClient.ClipboardPage.self,
                                             try await send(["operation": "list", "search": search]))
    if page.enabled && devices == nil {
      devices = try BackendRustAccount.decode(BackendAccountClient.ClipboardDevices.self, try await send(["operation": "devices"]))
    }
    guard try await account.status()?.id == current else { throw CancellationError() }
    try Task.checkCancellation()
    return Loaded(accountID: current, page: page, devices: page.enabled ? devices : nil)
  }

  /// 设备管理的 action：批准、拒绝、移除一台设备、重置云剪贴板或读设备状态。列表和添加不在这里。
  static func action(_ operation: BackendAccountClient.ClipboardE2EOperation) -> [String: Any]? {
    switch operation {
    case .approve(let id): return ["operation": "approve", "device_id": id]
    case .reject(let id): return ["operation": "reject", "device_id": id]
    case .remove(let id): return ["operation": "remove", "device_id": id]
    case .reset: return ["operation": "reset"]
    case .status: return ["operation": "devices"]
    case .list, .add: return nil
    }
  }
}

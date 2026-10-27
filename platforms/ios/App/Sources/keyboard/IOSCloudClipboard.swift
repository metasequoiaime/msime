import Foundation

/// 云剪贴板页的一次刷新：先做页面要的操作，再读列表，云剪贴板开着、而那个操作没有带回设备状态时再读一次设备状态。走哪条路以共享层的 `msime_client_cloud_clipboard_source` 为准：偏好里配置了自建中继或 WebDAV 网盘就只经 `IOSCloudClipboardRelay`，出错也不退回水杉账号；否则经 Rust 的 `msime_client_account_cloud_clipboard`（#8201 迁移计划 4c），请求带上偏好目录，Rust 在发出前再确认这时没有改成中继。每次请求都带本机的设备名（P6），只在本机还没登记端到端加密时发出去，给其他设备的设备列表看。
///
/// 页面按第一次读到的那一方绑定：账号路径是 user.id（C 上拿不到会话代次，同一个账号的令牌轮换和重新登录不作废页面），中继路径是中继或网盘的绑定（`IOSCloudClipboardRelayPolicy`，地址和令牌或账号的摘要）。请求前后都核对，退出、换号、换了中继或改了同步方式时是 `CancellationError`。
struct IOSCloudClipboard: Sendable {
  let account: BackendRustAccount
  let deviceName: String
  /// 共享偏好目录；测试给临时目录。
  var preferencesDirectory = IOSCloudClipboardRelay.preferencesDirectory
  /// 中继或网盘的本机目录；测试给临时目录。
  var relayDirectory = IOSCloudClipboardRelay.directory

  /// 走水杉账号而这时没有登录：页面说明登录后能做什么，以及可以改用中继或网盘。页面已经绑定过账号时是 `CancellationError`（退出作废这一页）。
  struct SignedOut: Error {}

  /// 发给 Rust 的一个云剪贴板 action。
  typealias Request = @Sendable ([String: Any]) async throws -> Data

  /// 一次刷新的结果。
  struct Loaded: Sendable {
    /// 页面绑定的那一方：账号的 user.id，或中继、网盘的绑定。
    let accountID: String
    let page: BackendAccountClient.ClipboardPage
    /// 云剪贴板关着时为 nil。
    let devices: BackendAccountClient.ClipboardDevices?
    /// 经中继时是 `.relay` 或 `.webdav`，经水杉账号时为 nil。
    let relay: IOSCloudClipboardRelay.Backend?
  }

  /// `accountID` 是页面已经绑定的那一方，第一次刷新时为 nil。
  func refresh(boundTo accountID: String?, search: String,
               _ action: (Request) async throws -> BackendAccountClient.ClipboardDevices?) async throws -> Loaded {
    if try await IOSCloudClipboardRelay.selected(preferencesDirectory: preferencesDirectory) {
      return try await refreshRelay(boundTo: accountID, search: search, action)
    }
    let account = self.account, deviceName = self.deviceName
    let directory = URL(fileURLWithPath: preferencesDirectory, isDirectory: true)
    let send: Request = { action in try await account.cloudClipboard(action, deviceName: deviceName, preferencesDirectory: directory) }
    guard let current = try await account.status()?.id else {
      if accountID == nil { throw SignedOut() }
      throw CancellationError()
    }
    guard accountID == nil || accountID == current else { throw CancellationError() }
    var devices = try await action(send)
    let page = try BackendRustAccount.decode(BackendAccountClient.ClipboardPage.self,
                                             try await send(["operation": "list", "search": search]))
    if page.enabled && devices == nil {
      devices = try BackendRustAccount.decode(BackendAccountClient.ClipboardDevices.self, try await send(["operation": "devices"]))
    }
    guard try await account.status()?.id == current else { throw CancellationError() }
    try Task.checkCancellation()
    return Loaded(accountID: current, page: page, devices: page.enabled ? devices : nil, relay: nil)
  }

  /// 经自建中继或 WebDAV 网盘刷新，不要求登录水杉账号。中继在操作前后都还是打开时那一个，否则作废这一页。
  private func refreshRelay(boundTo accountID: String?, search: String,
                            _ action: (Request) async throws -> BackendAccountClient.ClipboardDevices?) async throws -> Loaded {
    let relay = try IOSCloudClipboardRelay.open(preferencesDirectory: preferencesDirectory, directory: relayDirectory,
                                                deviceName: deviceName)
    guard accountID == nil || accountID == relay.binding else { throw CancellationError() }
    let send: Request = { action in try await relay.request(action) }
    var devices = try await action(send)
    let page = try BackendAccountClient.clipboardPage(try await send(["operation": "list", "search": search]))
    if page.enabled && devices == nil {
      devices = try BackendRustAccount.decode(BackendAccountClient.ClipboardDevices.self, try await send(["operation": "devices"]))
    }
    guard (try? relay.requireCurrent()) != nil else { throw CancellationError() }
    try Task.checkCancellation()
    return Loaded(accountID: relay.binding, page: page, devices: page.enabled ? devices : nil, relay: relay.backend)
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

import Foundation

/// macOS 云剪贴板的两条路（`docs/cloud-clipboard-relay.md`）：水杉账号，或用户自己部署的中继。偏好 `cloud_clipboard_relay` 里配置了中继就只走中继，不要求登录水杉账号，失败也不退回账号——否则用户以为在用自己的中继，内容却去了官方服务。
///
/// 输入法进程里的三个入口共用这一份选路：输入法菜单启动的 Tauri 云剪贴板面板与表情面板的「发到云剪贴板」（`BackendCloudClipboardProvider`），以及未登录时退回的原生云剪贴板窗口（`MacClipboardModel`）。设置应用里的 Tauri 面板自己按同一份偏好选路（`apps/desktop` 的 `cloud_clipboard_request`），走的是同一个 C 入口。
enum BackendCloudClipboardRoute: Sendable, Equatable {
  /// 水杉账号：绑定打开入口时的账号，每个动作前后都核对账号还在；令牌轮换和重新登录不作废。
  case account(userID: String)
  /// 用户自建的中继：绑定打开时的偏好修订号，不要求登录。`directory` 是输入法状态目录，偏好与中继密钥串 `cloud-clipboard-relay-keystore` 都在那里。
  case relay(directory: URL, revision: UInt64)

  /// 读共享偏好决定走哪条路。配置了中继时是 `.relay`；否则要求登录，没有登录是 nil，由调用方请用户登录。偏好读不了（`storage`）或中继配置不合规（`relay_not_configured`）时抛错：这两种情况都不拿账号路径顶上。
  static func selected(rust: BackendRustAccount) async throws -> BackendCloudClipboardRoute? {
    guard let directory = BackendDesktopSessionFile.standardDirectory else { throw BackendRustClient.Failure(code: "storage") }
    let selection = try await rust.cloudClipboardSelection(preferencesDirectory: directory)
    switch selection.source {
    case "relay":
      return .relay(directory: directory, revision: selection.revision)
    case "account":
      guard let user = try await rust.status() else { return nil }
      return .account(userID: user.id)
    default:
      throw BackendRustClient.Failure.invalidResponse
    }
  }

  /// 缓存身份区分来源、账号或中继修订号，不包含地址与凭据。
  var cacheKey: String {
    switch self {
    case .account(let userID): return "account:" + userID
    case .relay(let directory, let revision): return "relay:" + directory.path + ":" + String(revision)
    }
  }

  /// 每次请求前后核对来源；Rust 在读取实际中继配置的同一快照内复核修订号。
  func request(_ rust: BackendRustAccount, _ action: [String: Any], deviceName: String) async throws -> Data {
    try await requireCurrent(rust)
    let data: Data
    switch self {
    case .account:
      guard let directory = BackendDesktopSessionFile.standardDirectory else { throw BackendRustClient.Failure(code: "storage") }
      data = try await rust.cloudClipboard(action, deviceName: deviceName, preferencesDirectory: directory)
    case .relay(let directory, let revision):
      data = try await rust.relayCloudClipboard(action, preferencesDirectory: directory, keystoreDirectory: directory,
                                               deviceName: deviceName, expectedRevision: revision)
    }
    try await requireCurrent(rust)
    return data
  }

  /// 账号路径保留同账号令牌轮换语义；中继绑定来源和修订号，旧窗口不能操作新空间。
  func requireCurrent(_ rust: BackendRustAccount) async throws {
    try Task.checkCancellation()
    guard let directory = BackendDesktopSessionFile.standardDirectory else { throw BackendRustClient.Failure(code: "storage") }
    let selection = try await rust.cloudClipboardSelection(preferencesDirectory: directory)
    switch self {
    case .account(let userID):
      guard selection.source == "account" else { throw BackendRustClient.Failure(code: "clipboard_source_changed") }
      guard try await rust.status()?.id == userID else { throw BackendRustClient.Failure(code: "account_unauthorized") }
    case .relay(let selectedDirectory, let revision):
      guard directory == selectedDirectory, selection.source == "relay", selection.revision == revision else {
        throw BackendRustClient.Failure(code: "clipboard_source_changed")
      }
    }
    try Task.checkCancellation()
  }
}

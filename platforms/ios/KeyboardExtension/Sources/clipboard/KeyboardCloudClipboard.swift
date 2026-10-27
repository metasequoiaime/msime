import Foundation
import UIKit

/// 面板这一次读取属于谁：水杉账号的登录（user.id 和会话），或者偏好里的自建中继、WebDAV 网盘（`relay` 是 `IOSCloudClipboardRelayPolicy` 的绑定，地址和令牌或账号的摘要，不含凭据）。
struct KeyboardCloudClipboardBinding: Equatable, Sendable {
  let userID: String
  let sessionID: UUID
  /// 经中继或网盘时的绑定；经水杉账号时为 nil。
  var relay: String? = nil
}

/// What the keyboard needs from the account's cloud clipboard. The live one goes through the signed-in session the app shares with the keyboard through the App Group keychain group; tests substitute their own.
protocol KeyboardCloudClipboardService: Sendable {
  /// Captures the current login before a panel request.
  func binding() async throws -> KeyboardCloudClipboardBinding?
  func matches(_ binding: KeyboardCloudClipboardBinding) async -> Bool
  func page(matching binding: KeyboardCloudClipboardBinding) async throws -> BackendAccountClient.ClipboardPage
  func add(_ text: String, matching binding: KeyboardCloudClipboardBinding) async throws
  func uploadIfEnabled(_ text: String, matching binding: KeyboardCloudClipboardBinding) async throws -> BackendAccountClient.ClipboardPage?
}

extension KeyboardCloudClipboardService {
  func uploadIfEnabled(_ text: String, matching binding: KeyboardCloudClipboardBinding) async throws -> BackendAccountClient.ClipboardPage? {
    let before = try await page(matching: binding)
    guard before.enabled else { return nil }
    guard !before.isPending else { throw BackendAccountClient.Failure(status: 0, code: "clipboard_device_pending") }
    try await add(text, matching: binding)
    return try await page(matching: binding)
  }
}

/// 列表和上传经端到端加密驱动（`IOSCloudClipboardDriver`），与 App 用同一份密钥串，所以键盘和 App 是同一台设备。
///
/// 偏好里配置了自建中继或 WebDAV 网盘时（以共享层的 `msime_client_cloud_clipboard_source` 为准）只经 `IOSCloudClipboardRelay`，与 App 用同一份中继密钥串（App Group 访问组的钥匙串项）和同一个本机目录；读不了偏好或配置不合规时按失败处理，从不改走账号。每次请求前都核对这时的同步方式还是读列表时那一种、中继还是那一个，不在配置了中继时经账号读写，也不把一条记录发到刚换掉的中继。
struct BackendKeyboardCloudClipboardService: KeyboardCloudClipboardService {
  var session: BackendAccountSession = .shared
  var client = BackendAccountClient()
  var e2e: BackendClipboardE2E? = IOSCloudClipboardDriver.e2e
  var preferencesDirectory = IOSCloudClipboardRelay.preferencesDirectory
  var relayDirectory = IOSCloudClipboardRelay.directory

  /// 中继路径没有登录会话，绑定里的会话编号固定为这个值。
  private static let relaySession = UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0))

  func binding() async throws -> KeyboardCloudClipboardBinding? {
    if try await IOSCloudClipboardRelay.selected(preferencesDirectory: preferencesDirectory) {
      guard let relay = try IOSCloudClipboardRelay.currentConfiguration(preferencesDirectory: preferencesDirectory) else {
        throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.changed)
      }
      return .init(userID: "", sessionID: Self.relaySession, relay: relay.binding)
    }
    do {
      let identity = try await session.credentials()
      return .init(userID: identity.userID, sessionID: identity.sessionID)
    } catch let failure as BackendAccountClient.Failure where failure.status == 401 {
      return nil
    }
  }
  func matches(_ binding: KeyboardCloudClipboardBinding) async -> Bool {
    if let relay = binding.relay {
      return (try? IOSCloudClipboardRelay.currentConfiguration(preferencesDirectory: preferencesDirectory))?.binding == relay
    }
    // 读列表之后改成了中继或网盘：账号的条目不再插入。
    guard (try? await IOSCloudClipboardRelay.selected(preferencesDirectory: preferencesDirectory)) == false else { return false }
    do {
      try await session.requireSession(matchingUserID: binding.userID, matchingSessionID: binding.sessionID)
      return true
    } catch { return false }
  }
  func page(matching binding: KeyboardCloudClipboardBinding) async throws -> BackendAccountClient.ClipboardPage {
    if binding.relay != nil { return try await relay(matching: binding).page() }
    return try await authorized(matching: binding) { [client] token, e2e in try await client.clipboard(token: token, e2e: e2e) }
  }
  func add(_ text: String, matching binding: KeyboardCloudClipboardBinding) async throws {
    if binding.relay != nil {
      _ = try await relay(matching: binding).add(text)
      return
    }
    _ = try await authorized(matching: binding) { [client] token, e2e in try await client.addClipboard(text, token: token, e2e: e2e) }
  }
  /// 驱动上传前自己会读服务端的开关和本机是否已获批准：开关关着时返回 nil，本机等待批准时以 `clipboard_device_pending` 失败，两种情况都不上传。
  func uploadIfEnabled(_ text: String, matching binding: KeyboardCloudClipboardBinding) async throws -> BackendAccountClient.ClipboardPage? {
    if binding.relay != nil {
      let relay = try await relay(matching: binding)
      do {
        _ = try await relay.add(text)
      } catch let failure as IOSCloudClipboardRelayFailure where failure.code == "clipboard_sync_disabled" {
        return nil
      }
      return try await relay.page()
    }
    try await requireAccountRoute()
    let e2e = e2e?.bound(to: session, userID: binding.userID, sessionID: binding.sessionID)
    func request<T: Sendable>(_ operation: @Sendable (String) async throws -> T) async throws -> T {
      try await session.authenticated(matchingUserID: binding.userID,
                                      matchingSessionID: binding.sessionID, operation).value
    }
    do {
      _ = try await request { [client, e2e] token in try await client.addClipboard(text, token: token, e2e: e2e) }
    } catch let failure as BackendAccountClient.Failure where failure.code == "clipboard_sync_disabled" {
      return nil
    }
    return try await request { [client, e2e] token in try await client.clipboard(token: token, e2e: e2e) }
  }
  /// 偏好里的中继还是读列表时那一个，打开它；换过了、改回了账号或偏好读不了都抛出（`relay_changed`、`storage`）。
  private func relay(matching binding: KeyboardCloudClipboardBinding) async throws -> IOSCloudClipboardRelay {
    guard try await IOSCloudClipboardRelay.selected(preferencesDirectory: preferencesDirectory) else {
      throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.changed)
    }
    let name = await MainActor.run { IOSCloudClipboardDriver.deviceName(UIDevice.current) }
    let relay = try IOSCloudClipboardRelay.open(preferencesDirectory: preferencesDirectory, directory: relayDirectory, deviceName: name)
    guard relay.binding == binding.relay else { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.changed) }
    return relay
  }

  /// 经水杉账号发请求之前确认这时没有改成中继或网盘；读不了偏好也不发。
  private func requireAccountRoute() async throws {
    if try await IOSCloudClipboardRelay.selected(preferencesDirectory: preferencesDirectory) {
      throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.changed)
    }
  }

  /// 将请求及可能发生的令牌刷新绑定到发起时的登录会话。
  private func authorized<T: Sendable>(matching binding: KeyboardCloudClipboardBinding,
                                       _ body: @Sendable (String, BackendClipboardE2E?) async throws -> T) async throws -> T {
    try await requireAccountRoute()
    let e2e = e2e?.bound(to: session, userID: binding.userID, sessionID: binding.sessionID)
    let result = try await session.authenticated(matchingUserID: binding.userID,
                                                 matchingSessionID: binding.sessionID) { token in
      try await body(token, e2e)
    }.value
    try await session.requireSession(matchingUserID: binding.userID, matchingSessionID: binding.sessionID)
    try Task.checkCancellation()
    return result
  }
}

/// The 云端 half of the keyboard clipboard panel. Nothing is read or sent in the background: the list is fetched when the panel opens and when the user asks for a refresh, and text goes up only when the user picks 发到云剪贴板 on a local history item. A fetch that comes back after the panel closed, or after a newer refresh, is dropped.
@MainActor
final class KeyboardCloudClipboard {
  enum State {
    case needsFullAccess
    case loading
    case signedOut
    case disabled
    /// 本机还在等待其他设备批准，带本机的核对码。
    case pending(String)
    case failed(String)
    case loaded([BackendAccountClient.ClipboardItem])
  }

  static let signedOutMessage = "登录水杉账号后可在设备间同步剪贴板"
  static let disabledMessage = "云剪贴板未开启"
  static let needsFullAccessMessage = "请在系统键盘设置中开启「允许完全访问」，再使用云剪贴板。"
  static let offlineMessage = "连接未完成，请检查网络后重试。"
  /// 本机等待批准时的一行提示：去哪里批准，核对什么。
  static func pendingMessage(_ code: String) -> String {
    "本机等待批准 · 核对码 \(code)，请在其他设备的云剪贴板里点「允许」"
  }

  private(set) var state: State
  /// Known only once the panel's first fetch has looked at the keychain; until then 发到云剪贴板 stays disabled.
  private(set) var signedIn = false
  /// One line about the last upload, shown under the local history.
  private(set) var notice: String?
  private(set) var uploading = false
  var onChange: (() -> Void)?
  /// Asked again right before a cloud item is shown or inserted, so a field that became a password field meanwhile gets nothing.
  var fieldAllowsCloud: () -> Bool = { true }
  private let service: any KeyboardCloudClipboardService
  private var fetch: Task<Void, Never>?
  private var binding: KeyboardCloudClipboardBinding?
  private var generation = 0
  private var active = true

  init(hasFullAccess: Bool, service: any KeyboardCloudClipboardService = BackendKeyboardCloudClipboardService()) {
    self.service = service
    state = hasFullAccess ? .loading : .needsFullAccess
  }

  var items: [BackendAccountClient.ClipboardItem] {
    if case .loaded(let items) = state { return items }
    return []
  }
  var isDisabled: Bool {
    if case .disabled = state { return true }
    return false
  }
  var canUpload: Bool {
    guard active && signedIn && !uploading && fieldAllowsCloud() else { return false }
    if case .loaded = state { return true }
    return false
  }
  var message: String {
    switch state {
    case .needsFullAccess: return Self.needsFullAccessMessage
    case .loading: return "正在读取云端内容…"
    case .signedOut: return Self.signedOutMessage
    case .disabled: return Self.disabledMessage
    case .pending(let code): return Self.pendingMessage(code)
    case .failed(let message): return message
    case .loaded(let items):
      return items.isEmpty ? "云端还没有内容。在其他设备发到云剪贴板后点刷新。" : "\(items.count)/50 条 · 点按插入"
    }
  }

  /// Fetch the list. Called when the panel opens and from the refresh button.
  func refresh() {
    guard active else { return }
    if case .needsFullAccess = state { return }
    generation += 1
    let version = generation
    fetch?.cancel()
    binding = nil
    signedIn = false
    notice = nil
    state = .loading
    onChange?()
    let service = service
    fetch = Task { [weak self] in
      var account: KeyboardCloudClipboardBinding?
      let outcome: State
      do {
        account = try await service.binding()
        if let account {
          let page = try await service.page(matching: account)
          if !page.enabled { outcome = .disabled }
          else if page.isPending { outcome = .pending(page.e2e?.code ?? "") }
          else { outcome = .loaded(page.items) }
        } else { outcome = .signedOut }
      } catch { outcome = Self.state(for: error) }
      guard let self, !Task.isCancelled, self.active, self.generation == version else { return }
      if let account, !(await service.matches(account)) {
        self.refresh()
        return
      }
      guard !Task.isCancelled, self.active, self.generation == version else { return }
      self.binding = account
      self.signedIn = account != nil && !outcome.isSignedOut
      self.state = outcome
      self.onChange?()
    }
  }

  /// A displayed row may outlive its account. Recheck the shared session before inserting it.
  func insert(_ item: BackendAccountClient.ClipboardItem, onInsert: @escaping (String) -> Void) {
    guard active, let binding, items.contains(where: { $0.id == item.id && $0.text == item.text }) else { return }
    let version = generation
    let service = service
    Task { [weak self] in
      let stillCurrent = await service.matches(binding)
      guard let self, self.active, self.generation == version else { return }
      guard stillCurrent else { self.refresh(); return }
      guard self.fieldAllowsCloud(), self.items.contains(where: { $0.id == item.id && $0.text == item.text }) else { return }
      onInsert(item.text)
    }
  }

  /// Send one local history item. The server's `enabled` flag is read first, so nothing goes up while the user has the cloud clipboard switched off on another device.
  func upload(_ text: String) {
    guard canUpload, let binding else { return }
    guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, text.utf16.count <= 4000,
          !text.contains("\0") else {
      notice = "这条记录超过 4000 字，不能发到云剪贴板。"
      onChange?()
      return
    }
    uploading = true
    notice = "正在发到云剪贴板…"
    onChange?()
    let service = service
    let uploadGeneration = generation
    Task { [weak self] in
      var refreshed: State?
      var notice: String
      do {
        if let page = try await service.uploadIfEnabled(text, matching: binding) {
          refreshed = page.isPending ? .pending(page.e2e?.code ?? "") : .loaded(page.items)
          notice = "已发到云剪贴板"
        } else {
          refreshed = .disabled
          notice = Self.disabledMessage
        }
      } catch {
        let failed = Self.state(for: error)
        if failed.isSignedOut { refreshed = failed }
        notice = "没能发到云剪贴板：" + (failed.failureText ?? Self.signedOutMessage)
      }
      guard let self, self.active else { return }
      guard await service.matches(binding) else {
        self.uploading = false
        self.refresh()
        return
      }
      self.uploading = false
      self.notice = notice
      if let refreshed {
        // A newer list or a refresh still in flight wins over what this upload saw.
        if self.generation == uploadGeneration && !self.isLoading { self.state = refreshed }
        if refreshed.isSignedOut { self.signedIn = false }
      }
      self.onChange?()
    }
  }

  /// The panel is gone: stop the fetch and drop anything that still comes back. An upload already sent is left to finish, since the user asked for it.
  func close() {
    active = false
    generation += 1
    fetch?.cancel()
    fetch = nil
    onChange = nil
  }

  private var isLoading: Bool {
    if case .loading = state { return true }
    return false
  }

  private static func state(for error: Error) -> State {
    if let failure = error as? BackendAccountClient.Failure {
      return failure.status == 401 ? .signedOut : .failed(failure.localizedDescription)
    }
    // 中继或网盘的失败（令牌被拒、连不上、网盘回退……）各有说法，都不是没登录。
    if let failure = error as? IOSCloudClipboardRelayFailure { return .failed(failure.localizedDescription) }
    return .failed(offlineMessage)
  }
}

private extension KeyboardCloudClipboard.State {
  var isSignedOut: Bool {
    if case .signedOut = self { return true }
    return false
  }
  var failureText: String? {
    if case .failed(let message) = self { return message }
    return nil
  }
}

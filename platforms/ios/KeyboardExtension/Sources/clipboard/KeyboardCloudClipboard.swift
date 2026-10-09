import Foundation

/// What the keyboard needs from the account's cloud clipboard. The live one goes through the signed-in session the app shares with the keyboard through the App Group keychain group; tests substitute their own.
protocol KeyboardCloudClipboardService: Sendable {
  /// Whether an account is signed in on this device. Reads the shared keychain only, no request.
  func isSignedIn() async -> Bool
  func page() async throws -> BackendAccountClient.ClipboardPage
  func add(_ text: String) async throws
  func uploadIfEnabled(_ text: String) async throws -> BackendAccountClient.ClipboardPage?
}

extension KeyboardCloudClipboardService {
  func uploadIfEnabled(_ text: String) async throws -> BackendAccountClient.ClipboardPage? {
    let before = try await page()
    guard before.enabled else { return nil }
    try await add(text)
    return try await page()
  }
}

struct BackendKeyboardCloudClipboardService: KeyboardCloudClipboardService {
  var session: BackendAccountSession = .shared
  var client = BackendAccountClient()

  func isSignedIn() async -> Bool { ((try? await session.user()) ?? nil) != nil }
  func page() async throws -> BackendAccountClient.ClipboardPage {
    try await authorized { [client] token in try await client.clipboard(token: token) }
  }
  func add(_ text: String) async throws {
    _ = try await authorized { [client] token in try await client.addClipboard(text, token: token) }
  }
  func uploadIfEnabled(_ text: String) async throws -> BackendAccountClient.ClipboardPage? {
    let identity = try await session.credentials()
    func request<T: Sendable>(_ operation: @Sendable (String) async throws -> T) async throws -> T {
      try await session.authenticated(matchingUserID: identity.userID,
                                      matchingSessionID: identity.sessionID, operation).value
    }
    let before = try await request { [client] token in try await client.clipboard(token: token) }
    guard before.enabled else { return nil }
    _ = try await request { [client] token in try await client.addClipboard(text, token: token) }
    return try await request { [client] token in try await client.clipboard(token: token) }
  }
  /// 将请求及可能发生的令牌刷新绑定到发起时的登录会话。
  private func authorized<T: Sendable>(_ body: @Sendable (String) async throws -> T) async throws -> T {
    let identity = try await session.credentials()
    let result = try await session.authenticated(matchingUserID: identity.userID,
                                                 matchingSessionID: identity.sessionID) { token in
      try await body(token)
    }.value
    try await session.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
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
    case failed(String)
    case loaded([BackendAccountClient.ClipboardItem])
  }

  static let signedOutMessage = "登录水杉账号后可在设备间同步剪贴板"
  static let disabledMessage = "云剪贴板未开启"
  static let needsFullAccessMessage = "请在系统键盘设置中开启「允许完全访问」，再使用云剪贴板。"
  static let offlineMessage = "连接未完成，请检查网络后重试。"

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
    state = .loading
    onChange?()
    let service = service
    fetch = Task { [weak self] in
      let signedIn = await service.isSignedIn()
      let outcome: State
      if !signedIn { outcome = .signedOut }
      else {
        do {
          let page = try await service.page()
          outcome = page.enabled ? .loaded(page.items) : .disabled
        } catch { outcome = Self.state(for: error) }
      }
      guard let self, !Task.isCancelled, self.active, self.generation == version else { return }
      self.signedIn = signedIn && !(outcome.isSignedOut)
      self.state = outcome
      self.onChange?()
    }
  }

  /// Send one local history item. The server's `enabled` flag is read first, so nothing goes up while the user has the cloud clipboard switched off on another device.
  func upload(_ text: String) {
    guard canUpload else { return }
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
        if let page = try await service.uploadIfEnabled(text) {
          refreshed = .loaded(page.items)
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

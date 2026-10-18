import Foundation

/// 给 Tauri 云剪贴板面板的 IPC 服务端（`DesktopCloudClipboard.mm`）做事的一方：一个面板只能操作打开它时登录的那个账号。动作原样交给 Rust 的 `msime_client_account_cloud_clipboard`（#8201 迁移计划 4e），端到端加密、密钥串和令牌都在 Rust 里，凭据不经过 IPC。
@MainActor @objc(MSIMEBackendCloudClipboardProvider)
final class BackendCloudClipboardProvider: NSObject {
  /// 面板能发的动作，与原来的 Swift provider 相同。Rust 还认 `pin`、`set_retention`、`clear`，共享面板在 macOS 上没有这几个入口，不放行。
  static let operations: Set<String> = ["list", "add", "delete", "set_enabled", "devices", "approve", "reject", "remove", "reset"]
  private let rust: BackendRustAccount
  private let accountID: String
  private let deviceName: String
  init(rust: BackendRustAccount, accountID: String, deviceName: String = MacCloudClipboardDriver.deviceName) {
    self.rust = rust; self.accountID = accountID; self.deviceName = deviceName
  }

  /// 为当前登录的账号准备一个 provider；宿主没有注册会话存储、没有登录或读不了登录状态时是 nil，输入法随后回退到原生窗口。
  @objc static func prepare(completion: @escaping (BackendCloudClipboardProvider?) -> Void) {
    Task {
      guard let rust = BackendRustAccount.shared, let user = (try? await rust.status()) ?? nil else { completion(nil); return }
      completion(BackendCloudClipboardProvider(rust: rust, accountID: user.id))
    }
  }

  /// 打开面板时的账号还登录着才继续；退出或换了号时是 `account_unauthorized`，面板请用户重新登录。C 上拿不到会话代次，只按 user.id 绑定。
  private func requireAccount() async throws {
    guard try await rust.status()?.id == accountID else { throw BackendRustClient.Failure(code: "account_unauthorized") }
    try Task.checkCancellation()
  }

  /// 共享面板的一个动作（`docs/cloud-clipboard-e2e.md` 的「共享面板的动作」），结果是 Rust 给的 JSON 对象，形状原样交给面板。请求之前和之后都核对账号：退出登录以前发出的请求，回答不交给面板。
  func execute(_ request: NSDictionary) async throws -> [String: Any] {
    guard let operation = request["operation"] as? String, Self.operations.contains(operation),
          let action = request as? [String: Any], JSONSerialization.isValidJSONObject(action) else {
      throw BackendRustClient.Failure(code: "invalid_cloud_clipboard")
    }
    try await requireAccount()
    let result = try BackendRustAccount.object(try await rust.cloudClipboard(action, deviceName: deviceName))
    try await requireAccount()
    return result
  }

  /// What became of one local history entry the user explicitly chose to send; `message` is the shared wording the panel shows.
  enum SendOutcome: Equatable {
    case sent, signedOut, disabled, pending, failed
    var message: String {
      switch self {
      case .sent: return "已发到云剪贴板"
      case .signedOut: return "登录水杉账号后可在设备间同步剪贴板"
      case .disabled: return "云剪贴板未开启"
      case .pending: return "这台 Mac 还在等待批准，批准后才能发到云剪贴板"
      case .failed: return "发到云剪贴板失败，请重试"
      }
    }
  }

  /// 上传用户明确选中的一段文字。Rust 的驱动先读服务端的开关和本机是否已获批准，开关关着或本机还在等待批准时什么都不上传；令牌被拒时 Rust 刷新一次再试。
  func send(_ text: String) async -> SendOutcome {
    do {
      _ = try await execute(["operation": "add", "text": text])
      return .sent
    } catch let failure as BackendRustClient.Failure where failure.code == "clipboard_sync_disabled" {
      return .disabled
    } catch let failure as BackendRustClient.Failure where failure.code == "clipboard_device_pending" {
      return .pending
    } catch { return .failed }
  }

  /// 失败时交给面板的错误：Rust 和驱动给出的错误码原样传过去（例如 `clipboard_device_pending`，面板认得的码见 client-services 的 `CLIPBOARD_ERROR_CODES`），动作或参数不合法是 `invalid`，本地造出的失败和其余失败是 `unavailable`。
  static func errorReply(_ error: Error) -> NSDictionary {
    guard let failure = error as? BackendRustClient.Failure, !failure.code.hasPrefix("rust_client_") else { return ["ok": false, "error": "unavailable"] }
    return ["ok": false, "error": failure.code == "invalid_cloud_clipboard" ? "invalid" : failure.code]
  }

  /// `send(_:)` for the signed-in native account, or `.signedOut` when there is none.
  static func send(_ text: String) async -> SendOutcome {
    let provider: BackendCloudClipboardProvider? = await withCheckedContinuation { continuation in
      prepare { continuation.resume(returning: $0) }
    }
    guard let provider else { return .signedOut }
    return await provider.send(text)
  }

  @objc func request(_ request: NSDictionary, completion: @escaping (NSDictionary) -> Void) -> Progress {
    let progress = Progress(totalUnitCount: 1)
    let task = Task {
      // Progress.cancel() sets isCancelled synchronously but dispatches cancellationHandler asynchronously, so the handler's task.cancel() can land after this body has already run past execute's first checkCancellation. A caller that cancels before the work starts would then still see the request sent. Read the flag the caller set synchronously rather than racing the handler; cancellation arriving mid-flight is still the handler's job, and an already-sent request cannot be recalled anyway.
      do {
        guard !progress.isCancelled else { throw CancellationError() }
        completion(["ok": true, "value": try await execute(request)])
      }
      catch { completion(Self.errorReply(error)) }
      progress.completedUnitCount = 1
      progress.cancellationHandler = nil
    }
    progress.cancellationHandler = { task.cancel() }
    return progress
  }
}

import Foundation

protocol DesktopCloudClipboardAPI {
  /// 经端到端加密驱动完成一次操作，返回驱动结果的 JSON 对象（列表、条目或设备状态）。
  func clipboardE2EObject(_ operation: BackendAccountClient.ClipboardE2EOperation, token: String,
                          e2e: BackendClipboardE2E?) async throws -> [String: Any]
  func deleteClipboard(id: String?, token: String) async throws
  func setClipboardEnabled(_ enabled: Bool, token: String) async throws
}
extension BackendAccountClient: DesktopCloudClipboardAPI {}

/// A panel can perform clipboard operations for one native account only. Tokens
/// stay inside the existing account actor; no credential is returned over IPC.
@MainActor @objc(MSIMEBackendCloudClipboardProvider)
final class BackendCloudClipboardProvider: NSObject {
  private let client: any DesktopCloudClipboardAPI
  private let e2e: BackendClipboardE2E?
  private let credentials: () async throws -> String
  private let refreshCredentials: ((String) async throws -> String)?
  /// `e2e` 缺省是输入法交给后端的驱动（`MacCloudClipboardDriver`）；没有驱动时列表和上传都会失败，不退回明文。
  init(client: any DesktopCloudClipboardAPI, e2e: BackendClipboardE2E? = MacCloudClipboardDriver.e2e,
       credentials: @escaping () async throws -> String,
       refreshCredentials: ((String) async throws -> String)? = nil) {
    self.client = client; self.e2e = e2e; self.credentials = credentials; self.refreshCredentials = refreshCredentials
  }

  @objc static func prepare(completion: @escaping (BackendCloudClipboardProvider?) -> Void) {
    Task {
      do {
        let identity = try await BackendAccountSession.shared.credentials()
        completion(BackendCloudClipboardProvider(client: BackendAccountClient(),
          e2e: MacCloudClipboardDriver.e2e?.bound(to: .shared, userID: identity.userID, sessionID: identity.sessionID), credentials: {
          try await BackendAccountSession.shared.credentials(matchingUserID: identity.userID,
                                                              matchingSessionID: identity.sessionID).token
        }, refreshCredentials: { rejected in
          try await BackendAccountSession.shared.credentials(retrying: rejected, matchingUserID: identity.userID,
                                                              matchingSessionID: identity.sessionID).token
        }))
      } catch { completion(nil) }
    }
  }

  /// 共享面板的动作（`docs/cloud-clipboard-e2e.md` 的「共享面板的动作」）。列表、上传和设备相关的五个动作经驱动，删除和开关直接调用原来的接口。
  private enum Action {
    case driver(BackendAccountClient.ClipboardE2EOperation), delete(String), enabled(Bool)
    init(_ value: NSDictionary) throws {
      switch value["operation"] as? String {
      case "list":
        let search = value["search"] as? String ?? ""
        guard search.utf8.count <= 1024, !search.unicodeScalars.contains(where: { $0.properties.generalCategory == .control }) else { throw BackendAccountClient.Failure(status: 400) }
        self = .driver(.list(search: search))
      case "add":
        guard let text = value["text"] as? String, !text.isEmpty, text.utf16.count <= 4000,
              !text.unicodeScalars.contains(where: { $0.properties.generalCategory == .control && ![10, 13, 9].contains($0.value) }) else { throw BackendAccountClient.Failure(status: 400) }
        self = .driver(.add(text: text))
      case "devices": self = .driver(.status)
      case "reset": self = .driver(.reset)
      case "approve", "reject", "remove":
        guard let id = value["device_id"] as? String, BackendAccountClient.validClipboardDeviceID(id) else { throw BackendAccountClient.Failure(status: 400) }
        switch value["operation"] as? String {
        case "approve": self = .driver(.approve(deviceID: id))
        case "reject": self = .driver(.reject(deviceID: id))
        default: self = .driver(.remove(deviceID: id))
        }
      case "delete":
        guard let id = value["id"] as? String, id.utf8.count == 64,
              id.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else { throw BackendAccountClient.Failure(status: 400) }
        self = .delete(id)
      case "set_enabled":
        guard let number = value["enabled"] as? NSNumber,
              CFGetTypeID(number) == CFBooleanGetTypeID() else { throw BackendAccountClient.Failure(status: 400) }
        self = .enabled(number.boolValue)
      default: throw BackendAccountClient.Failure(status: 400)
      }
    }
  }

  func execute(_ request: NSDictionary) async throws -> [String: Any] {
    let action = try Action(request)
    let token = try await credentials()
    try Task.checkCancellation()
    let result: [String: Any]
    do { result = try await executeOnce(action, token: token) }
    catch let failure as BackendAccountClient.Failure where failure.status == 401 {
      guard let refreshCredentials else { throw failure }
      let fresh = try await refreshCredentials(token)
      try Task.checkCancellation()
      result = try await executeOnce(action, token: fresh)
    }
    // A request that succeeded before logout must not publish its old account's data.
    _ = try await credentials()
    try Task.checkCancellation()
    return result
  }

  /// 列表、条目和设备状态都原样转发驱动的结果形状，面板按 `docs/cloud-clipboard-e2e.md` 解析。
  private func executeOnce(_ action: Action, token: String) async throws -> [String: Any] {
    let result: [String: Any]
    switch action {
    case .driver(let operation): result = try await client.clipboardE2EObject(operation, token: token, e2e: e2e)
    case .delete(let id): try await client.deleteClipboard(id: id, token: token); result = [:]
    case .enabled(let enabled): try await client.setClipboardEnabled(enabled, token: token); result = ["enabled":enabled]
    }
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

  /// Uploads one explicitly chosen text. 驱动先读服务端的开关和本机是否已获批准，开关关着或本机还在等待批准时什么都不上传。A rejected token is refreshed once before retrying.
  func send(_ text: String) async -> SendOutcome {
    do {
      _ = try await execute(["operation": "add", "text": text])
      return .sent
    } catch let failure as BackendAccountClient.Failure where failure.code == "clipboard_sync_disabled" {
      return .disabled
    } catch let failure as BackendAccountClient.Failure where failure.code == "clipboard_device_pending" {
      return .pending
    } catch { return .failed }
  }

  /// 失败时交给面板的错误：驱动或服务端给出的错误码原样传过去（例如 `clipboard_device_pending`），本地校验不过是 `invalid`，其余是 `unavailable`。`status` 是驱动给出的状态码，0 表示不是服务端返回的错误。
  static func errorReply(_ error: Error) -> NSDictionary {
    guard let failure = error as? BackendAccountClient.Failure else { return ["ok":false, "error":"unavailable"] }
    let code = failure.code ?? (failure.status == 400 ? "invalid" : "unavailable")
    return ["ok":false, "error":code, "status":failure.status]
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
        completion(["ok":true, "value":try await execute(request)])
      }
      catch { completion(Self.errorReply(error)) }
      progress.completedUnitCount = 1
      progress.cancellationHandler = nil
    }
    progress.cancellationHandler = { task.cancel() }
    return progress
  }
}

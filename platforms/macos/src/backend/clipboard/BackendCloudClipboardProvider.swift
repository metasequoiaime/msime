import Foundation

protocol DesktopCloudClipboardAPI {
  func clipboard(token: String, search: String) async throws -> BackendAccountClient.ClipboardPage
  func addClipboard(_ text: String, token: String) async throws -> BackendAccountClient.ClipboardItem
  func deleteClipboard(id: String?, token: String) async throws
  func setClipboardEnabled(_ enabled: Bool, token: String) async throws
}
extension BackendAccountClient: DesktopCloudClipboardAPI {}

/// A panel can perform clipboard operations for one native account only. Tokens
/// stay inside the existing account actor; no credential is returned over IPC.
@MainActor @objc(MSIMEBackendCloudClipboardProvider)
final class BackendCloudClipboardProvider: NSObject {
  private let client: any DesktopCloudClipboardAPI
  private let credentials: () async throws -> String
  private let refreshCredentials: ((String) async throws -> String)?
  init(client: any DesktopCloudClipboardAPI, credentials: @escaping () async throws -> String,
       refreshCredentials: ((String) async throws -> String)? = nil) {
    self.client = client; self.credentials = credentials; self.refreshCredentials = refreshCredentials
  }

  @objc static func prepare(completion: @escaping (BackendCloudClipboardProvider?) -> Void) {
    Task {
      do {
        let identity = try await BackendAccountSession.shared.credentials()
        completion(BackendCloudClipboardProvider(client: BackendAccountClient(), credentials: {
          try await BackendAccountSession.shared.credentials(matchingUserID: identity.userID,
                                                              matchingSessionID: identity.sessionID).token
        }, refreshCredentials: { rejected in
          try await BackendAccountSession.shared.credentials(retrying: rejected, matchingUserID: identity.userID,
                                                              matchingSessionID: identity.sessionID).token
        }))
      } catch { completion(nil) }
    }
  }

  private enum Action {
    case list(String), add(String), delete(String), enabled(Bool)
    init(_ value: NSDictionary) throws {
      switch value["operation"] as? String {
      case "list":
        let search = value["search"] as? String ?? ""
        guard search.utf8.count <= 1024, !search.unicodeScalars.contains(where: { $0.properties.generalCategory == .control }) else { throw BackendAccountClient.Failure(status: 400) }
        self = .list(search)
      case "add":
        guard let text = value["text"] as? String, !text.isEmpty, text.utf16.count <= 4000,
              !text.unicodeScalars.contains(where: { $0.properties.generalCategory == .control && ![10, 13, 9].contains($0.value) }) else { throw BackendAccountClient.Failure(status: 400) }
        self = .add(text)
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

  private func item(_ value: BackendAccountClient.ClipboardItem) throws -> [String: Any] {
    guard !value.id.isEmpty, value.id.utf8.count <= 256, !value.text.isEmpty,
          value.text.utf16.count <= 4000, !value.text.contains("\0"), value.updated_at.utf8.count <= 128 else { throw BackendAccountClient.Failure(status: 0) }
    return ["id":value.id, "text":value.text, "updated_at":value.updated_at]
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

  private func executeOnce(_ action: Action, token: String) async throws -> [String: Any] {
    let result: [String: Any]
    switch action {
    case .list(let search):
      let page = try await client.clipboard(token: token, search: search)
      guard page.items.count <= 50 else { throw BackendAccountClient.Failure(status: 0) }
      result = ["enabled":page.enabled, "items":try page.items.map(item)]
    case .add(let text): result = try item(await client.addClipboard(text, token: token))
    case .delete(let id): try await client.deleteClipboard(id: id, token: token); result = [:]
    case .enabled(let enabled): try await client.setClipboardEnabled(enabled, token: token); result = ["enabled":enabled]
    }
    return result
  }

  /// What became of one local history entry the user explicitly chose to send; `message` is the shared wording the panel shows.
  enum SendOutcome: Equatable {
    case sent, signedOut, disabled, failed
    var message: String {
      switch self {
      case .sent: return "已发到云剪贴板"
      case .signedOut: return "登录水杉账号后可在设备间同步剪贴板"
      case .disabled: return "云剪贴板未开启"
      case .failed: return "发到云剪贴板失败，请重试"
      }
    }
  }

  /// Uploads one explicitly chosen text. The server's enabled flag is read first, so nothing leaves the device while the account has cloud clipboard switched off. A rejected token is refreshed once before retrying.
  func send(_ text: String) async -> SendOutcome {
    do {
      let page = try await execute(["operation": "list", "search": ""])
      guard page["enabled"] as? Bool == true else { return .disabled }
      _ = try await execute(["operation": "add", "text": text])
      return .sent
    } catch { return .failed }
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
      catch { completion(["ok":false, "error":"unavailable"]) }
      progress.completedUnitCount = 1
      progress.cancellationHandler = nil
    }
    progress.cancellationHandler = { task.cancel() }
    return progress
  }
}

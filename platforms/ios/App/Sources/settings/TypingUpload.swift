import Foundation

/// 打字统计的「上传到云端」，对应 Android 的 `home/TypingUpload`：开关、上次上传的时间和设备标识存在共享层（`msime_client_typing_statistics` 的 `typing-statistics-upload.json`），请求体和十分钟的节流也在那里，这里只带着真实账号的令牌把请求发出去。
///
/// 共享层要求 `upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE` 排在同一个串行队列里，免得关掉以后的删除和还在路上的上传交错，所以每个入口都经过 `serial`。actor 本身在 `await` 处可以重入，挡不住交错，串行靠的是一条依次等待的任务链。应用回到前台（`MetasequoiaImeApp`）和打开统计页时各试一次，节流由共享层判断；只有真实账号（`BackendAccountSession.shared`）上传，匿名账号和没登录时什么都不发。失败静默，下次再试。
actor TypingUpload {
  static let shared = TypingUpload()

  /// 开关、上次上传成功的时间（还没有时为 nil）和这台设备有没有登录真实账号。
  struct Status: Equatable, Sendable {
    let enabled: Bool
    let lastUpload: Date?
    let signedIn: Bool
  }

  /// 打开或关闭的结果：新的状态，以及要告诉用户的一句话。
  struct Change: Equatable, Sendable {
    let status: Status
    let message: String
  }

  private let store: TypingStatisticsStore
  private let session: BackendAccountSession
  private let client: BackendAccountClient
  private let now: @Sendable () -> Date
  private var tail: Task<Void, Never>?

  init(store: TypingStatisticsStore = TypingStatisticsStore(), session: BackendAccountSession = .shared,
       client: BackendAccountClient = BackendAccountClient(), now: @escaping @Sendable () -> Date = { Date() }) {
    self.store = store
    self.session = session
    self.client = client
    self.now = now
  }

  /// 回到前台时调用：开着上传、登录着真实账号时试一次，不到十分钟由共享层跳过。
  func onActive() async {
    _ = await serial { _ = try? await self.upload(force: false) }
  }

  /// 读开关状态，顺带补传一次（与共享设置页挂上时调 `sync` 相同）。补传失败不打扰用户；共享层读不出状态时为 nil。
  func sync() async -> Status? {
    await serial {
      _ = try? await self.upload(force: false)
      return try? await self.status()
    }
  }

  /// 打开或关闭上传。打开要求已登录真实账号，打开后立刻上传一次；关闭先在本机关掉，再删除云端的数据，删除失败时开关仍是关的。共享层读写失败时为 nil。
  func setEnabled(_ enabled: Bool) async -> Change? {
    await serial {
      let login = try? await self.session.login()
      if enabled && login == nil {
        return (try? await self.status()).map { Change(status: $0, message: "先在「我的」页登录水杉账号，再打开上传。") }
      }
      guard (try? self.store.setUploadEnabled(enabled)) != nil else { return nil }
      let message: String
      if enabled {
        do {
          try await self.upload(force: true)
          message = "已打开上传，最近 7 天的每日计数已传到你的水杉账号。"
        } catch {
          message = BackendAccountClient.typingUploadMessage(error, enabling: true)
        }
      } else if let login {
        do {
          let client = self.client
          _ = try await self.session.authenticated(matchingUserID: login.userID, matchingSessionID: login.sessionID) {
            try await client.deleteTypingStatistics(token: $0)
          }
          message = "已关闭上传，云端的打字统计已删除。"
        } catch {
          message = BackendAccountClient.typingUploadMessage(error, enabling: false)
        }
      } else {
        message = "已关闭上传。没有登录，云端的数据没有删掉，登录后打开再关闭一次即可删除。"
      }
      guard let status = try? await self.status() else { return nil }
      return Change(status: status, message: message)
    }
  }

  /// 只读状态，不联网。
  private nonisolated func status() async throws -> Status {
    let state = try store.uploadStatus()
    let signedIn = (try? await session.login()) != nil
    return Status(enabled: state.enabled, lastUpload: state.lastUploadMs.map { Date(timeIntervalSince1970: Double($0) / 1000) },
                  signedIn: signedIn)
  }

  /// 要发时发出去，成功后告诉共享层。没开、不到时候、没有字数或没登录时什么都不做；登录在发的过程中变了时 `authenticated` 抛错，这一次不记。
  private nonisolated func upload(force: Bool) async throws {
    guard let login = try await session.login() else { return }
    guard case let .send(path, device, body) = try store.uploadRequest(day: now(), force: force) else { return }
    let client = client
    _ = try await session.authenticated(matchingUserID: login.userID, matchingSessionID: login.sessionID) {
      try await client.uploadTypingStatistics(path: path, body: body, token: $0)
    }
    _ = try store.uploadSucceeded(device: device)
  }

  /// 等前一件做完再做这一件，返回这一件的结果。`work` 里不能再调 `serial`，否则会等自己。
  private func serial<T: Sendable>(_ work: @escaping @Sendable () async -> T) async -> T {
    let previous = tail
    let task = Task { () -> T in
      await previous?.value
      return await work()
    }
    tail = Task { _ = await task.value }
    return await task.value
  }
}

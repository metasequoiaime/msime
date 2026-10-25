import Foundation

struct DoubaoHandshake {
  enum Failure: Error { case missingAccessKey, missingResourceID }

  let appKey: String
  let accessKey: String
  let resourceID: String
  let requestID: String

  init(appKey: String, accessKey: String, resourceID: String, requestID: String = UUID().uuidString) throws {
    guard !accessKey.isEmpty else { throw Failure.missingAccessKey }
    guard !resourceID.isEmpty else { throw Failure.missingResourceID }
    self.appKey = appKey
    self.accessKey = accessKey
    self.resourceID = resourceID
    self.requestID = requestID
  }

  var headers: [String: String] {
    var result = [
      "X-Api-Resource-Id": resourceID,
      "X-Api-Request-Id": requestID,
    ]
    if appKey.isEmpty {
      result["X-Api-Key"] = accessKey
    } else {
      result["X-Api-App-Key"] = appKey
      result["X-Api-Access-Key"] = accessKey
    }
    return result
  }
}

/// iOS 宿主侧的 WebSocket 生命周期，供注入的 client-providers 语音传输使用。
final class DoubaoWebSocketTransport: NSObject, URLSessionWebSocketDelegate, DoubaoVoiceTransport {
  enum Failure: Error {
    case notConnected, closed
    /// 握手没有升级成 WebSocket，服务端回了这个 HTTP 状态码（密钥无效是 401）。
    case rejected(status: Int)
  }

  private struct Connection {
    let session: URLSession
    let task: URLSessionWebSocketTask
    var isConnected = false
  }

  private let stateLock = NSLock()
  private var connection: Connection?

  var isConnected: Bool {
    stateLock.withLock { connection?.isConnected ?? false }
  }

  func start(endpoint: URL) async throws {
    try await start(endpoint: endpoint, headers: [:])
  }

  func start(endpoint: URL, handshake: DoubaoHandshake) async throws {
    try await start(endpoint: endpoint, headers: handshake.headers)
  }

  func start(endpoint: URL, headers: [String: String] = [:]) async throws {
    guard let current = stateLock.withLock({ () -> Connection? in
      guard connection == nil else { return nil }
      let configuration = URLSessionConfiguration.ephemeral
      let session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
      var request = URLRequest(url: endpoint)
      request.allHTTPHeaderFields = headers
      let task = session.webSocketTask(with: request)
      task.maximumMessageSize = DoubaoVoiceCoordinator.maximumResponseFrameBytes
      let current = Connection(session: session, task: task)
      connection = current
      return current
    }) else { return }
    let session = current.session
    let task = current.task
    task.resume()
    do {
      try await withTaskCancellationHandler(operation: {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
          task.sendPing { error in
            if let error { continuation.resume(throwing: error) } else { continuation.resume() }
          }
        }
      }, onCancel: {
        _ = self.detachConnection(matching: task)
        task.cancel(with: .goingAway, reason: nil)
        session.invalidateAndCancel()
      })
      try Task.checkCancellation()
      let stillCurrent = stateLock.withLock {
        guard connection?.task === task else { return false }
        connection?.isConnected = true
        return true
      }
      guard stillCurrent else { throw Failure.closed }
    } catch {
      // 握手没升级时，系统把服务端的 HTTP 响应留在 `task.response` 上（错误本身只是笼统的 -1011）。只取状态码，正文不读。
      let status = (task.response as? HTTPURLResponse)?.statusCode
      // 握手失败只移除自己的登记，不能清除已经开始的替代连接。
      _ = detachConnection(matching: task)
      task.cancel(with: .goingAway, reason: nil)
      session.invalidateAndCancel()
      if !Task.isCancelled, let status, status != 101 { throw Failure.rejected(status: status) }
      throw error
    }
  }

  func urlSession(_ session: URLSession, task: URLSessionTask,
                  willPerformHTTPRedirection response: HTTPURLResponse,
                  newRequest request: URLRequest,
                  completionHandler: @escaping (URLRequest?) -> Void) {
    completionHandler(nil)
  }

  func send(binary frame: Data) async throws {
    let task = stateLock.withLock { connection?.isConnected == true ? connection?.task : nil }
    guard let task else { throw Failure.notConnected }
    try await task.send(.data(frame))
  }

  func receive() async throws -> Data {
    let task = stateLock.withLock { connection?.isConnected == true ? connection?.task : nil }
    guard let task else { throw Failure.notConnected }
    switch try await task.receive() {
    case .data(let frame): return frame
    case .string: throw Failure.closed
    @unknown default: throw Failure.closed
    }
  }

  func finish() {
    guard let current = detachConnection() else { return }
    current.task.cancel(with: .normalClosure, reason: nil)
    current.session.invalidateAndCancel()
  }

  func cancel() {
    guard let current = detachConnection() else { return }
    current.task.cancel(with: .goingAway, reason: nil)
    current.session.invalidateAndCancel()
  }

  private func detachConnection(matching task: URLSessionWebSocketTask? = nil) -> Connection? {
    stateLock.withLock {
      guard let current = connection, task == nil || current.task === task else { return nil }
      connection = nil
      return current
    }
  }

  func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                  didOpenWithProtocol protocol: String?) {
    // delegate 队列里的旧回调不能重新登记已结束的连接。
    stateLock.withLock {
      guard connection?.task === webSocketTask, connection?.session === session else { return }
      connection?.isConnected = true
    }
  }

  func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                  didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
    guard let current = detachConnection(matching: webSocketTask) else { return }
    // 锁外失效刚取出的会话，释放它持有的 delegate，避免重入状态锁。
    current.session.invalidateAndCancel()
  }
}

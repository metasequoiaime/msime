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

/// iOS host-side WebSocket lifecycle for the injected client-core voice transport.
final class DoubaoWebSocketTransport: NSObject, URLSessionWebSocketDelegate, DoubaoVoiceTransport {
  enum Failure: Error { case notConnected, closed }

  private var session: URLSession?
  private var task: URLSessionWebSocketTask?
  private(set) var isConnected = false

  func start(endpoint: URL) async throws {
    try await start(endpoint: endpoint, headers: [:])
  }

  func start(endpoint: URL, handshake: DoubaoHandshake) async throws {
    try await start(endpoint: endpoint, headers: handshake.headers)
  }

  func start(endpoint: URL, headers: [String: String] = [:]) async throws {
    guard task == nil else { return }
    let configuration = URLSessionConfiguration.ephemeral
    let session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
    var request = URLRequest(url: endpoint)
    request.allHTTPHeaderFields = headers
    let task = session.webSocketTask(with: request)
    task.maximumMessageSize = DoubaoVoiceCoordinator.maximumResponseFrameBytes
    self.session = session
    self.task = task
    task.resume()
    do {
      try await withTaskCancellationHandler(operation: {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
          task.sendPing { error in
            if let error { continuation.resume(throwing: error) } else { continuation.resume() }
          }
        }
      }, onCancel: {
        task.cancel(with: .goingAway, reason: nil)
        session.invalidateAndCancel()
      })
      try Task.checkCancellation()
      isConnected = true
    } catch {
      // Keep a failed or cancelled handshake retryable. The task is stored before
      // sendPing so callbacks can observe it, but leaving it there after an error
      // makes the next start() hit the `task == nil` guard and silently reuse a
      // dead socket forever.
      task.cancel(with: .goingAway, reason: nil)
      session.invalidateAndCancel()
      if self.task === task {
        self.task = nil
        self.session = nil
        self.isConnected = false
      }
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
    guard let task, isConnected else { throw Failure.notConnected }
    try await task.send(.data(frame))
  }

  func receive() async throws -> Data {
    guard let task, isConnected else { throw Failure.notConnected }
    switch try await task.receive() {
    case .data(let frame): return frame
    case .string: throw Failure.closed
    @unknown default: throw Failure.closed
    }
  }

  func finish() {
    task?.cancel(with: .normalClosure, reason: nil)
    task = nil
    session?.invalidateAndCancel()
    session = nil
    isConnected = false
  }

  func cancel() {
    task?.cancel(with: .goingAway, reason: nil)
    task = nil
    session?.invalidateAndCancel()
    session = nil
    isConnected = false
  }

  func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                  didOpenWithProtocol protocol: String?) {
    self.session = session
    self.task = webSocketTask
    isConnected = true
  }

  func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                  didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
    guard self.task === webSocketTask else { return }
    self.task = nil
    self.session = nil
    isConnected = false
  }
}

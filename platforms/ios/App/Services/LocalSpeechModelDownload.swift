import Foundation

/// 每次模型安装独立保存取消意图，防止工作线程启动前和共享层登记前的取消丢失。
final class LocalSpeechModelDownload: @unchecked Sendable {
  private let id: String
  private let lock = NSLock()
  private let cancellationQueue = DispatchQueue(label: "msime.local-model-cancel")
  private var cancelRequested = false
  private var registered = false
  private var finished = false

  init(id: String) { self.id = id }

  func cancel() {
    lock.lock()
    cancelRequested = true
    let shouldForward = registered && !finished
    lock.unlock()
    // 共享取消可能等待其他模型的目录操作，主线程只保存意图。
    if shouldForward { cancellationQueue.async { [self] in forwardCancelIfRunning() } }
  }

  private func forwardCancelIfRunning() {
    lock.lock()
    let shouldForward = registered && !finished
    lock.unlock()
    if shouldForward { LocalSpeechModelStore.cancel(id: id) }
  }

  func install(root: URL, mirror: String, files: [URL]? = nil,
               progress: @escaping (LocalSpeechInstallProgress) -> Void) throws -> URL {
    lock.lock()
    let cancelled = cancelRequested
    lock.unlock()
    defer {
      // 先排空旧取消再允许管理器重试；晚入队的请求看到结束标记后跳过。
      cancellationQueue.sync {
        lock.lock()
        finished = true
        registered = false
        lock.unlock()
      }
    }
    if cancelled { throw LocalSpeechModelStore.HostFailure(code: "local_model_cancelled") }
    return try LocalSpeechModelStore.install(root: root, id: id, mirror: mirror, files: files) { [self] update in
      // C ABI 在登记后、I/O 前同步回调；补发先前保存的取消，安装器与外部回调均在锁外运行。
      lock.lock()
      registered = true
      lock.unlock()
      progress(update)
      // 外部回调也可能请求取消，在返回共享层检查点前一并转发。
      lock.lock()
      let shouldForward = cancelRequested
      lock.unlock()
      if shouldForward { cancellationQueue.sync { forwardCancelIfRunning() } }
    }
  }
}

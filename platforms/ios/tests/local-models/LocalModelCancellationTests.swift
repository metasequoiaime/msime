import Combine
import Foundation

struct LocalSpeechModelInfo {
  let id = "synthetic-model"
  let title = "合成模型"
  let desktopOnly = false
  let installed = false
  let path = "/synthetic/models/synthetic-model"
}
struct LocalSpeechInstallProgress {
  let id: String
  let stage: String
  let downloaded: UInt64
  let total: UInt64
}

final class ModelInstallationFixture: @unchecked Sendable {
  static let shared = ModelInstallationFixture()
  private let condition = NSCondition()
  private var blocked = false
  private var released = false
  private var registered = false
  private var cancelled = false
  private var cancelEntered = false
  private var cancelReleased = false
  private var cancelFinished = false
  private(set) var calls = 0
  var holdRegistration = false
  var holdCompletion = false
  var succeed = false
  var holdCancel = false

  func install(progress: (LocalSpeechInstallProgress) -> Void) throws -> URL {
    condition.lock()
    calls += 1
    if holdRegistration { waitForRelease() }
    registered = true
    condition.unlock()
    progress(LocalSpeechInstallProgress(id: "synthetic-model", stage: "download", downloaded: 0, total: 100))
    condition.lock()
    if holdCompletion { waitForRelease() }
    let stopped = cancelled
    registered = false
    condition.unlock()
    if stopped { throw LocalSpeechModelStore.HostFailure(code: "local_model_cancelled") }
    if succeed { return URL(fileURLWithPath: "/synthetic/models/synthetic-model") }
    throw LocalSpeechModelStore.HostFailure(code: "synthetic_cancel_lost")
  }

  private func waitForRelease() {
    blocked = true
    condition.broadcast()
    while !released { condition.wait() }
  }
  func waitForWorker() {
    condition.lock()
    while !blocked { condition.wait() }
    condition.unlock()
  }
  func releaseWorker() {
    condition.lock()
    released = true
    condition.broadcast()
    condition.unlock()
  }
  func cancel() {
    condition.lock()
    if holdCancel {
      cancelEntered = true
      condition.broadcast()
      while !cancelReleased { condition.wait() }
    }
    if registered { cancelled = true }
    cancelFinished = true
    condition.broadcast()
    condition.unlock()
  }
  func waitForCancel() {
    condition.lock()
    while !cancelEntered { condition.wait() }
    condition.unlock()
  }
  func releaseCancel() {
    condition.lock()
    cancelReleased = true
    condition.broadcast()
    condition.unlock()
  }
  func waitForCancelFinished() {
    condition.lock()
    while !cancelFinished { condition.wait() }
    condition.unlock()
  }
  func prepareRetry() {
    condition.lock()
    holdRegistration = false
    holdCompletion = false
    cancelled = false
    succeed = true
    holdCancel = false
    condition.unlock()
  }
}

enum LocalSpeechModelStore {
  struct Catalog { let models: [LocalSpeechModelInfo] }
  struct HostFailure: LocalizedError {
    let code: String
    var errorDescription: String? { code }
  }
  static func catalog(root: URL) throws -> Catalog { Catalog(models: []) }
  static func install(root: URL, id: String, mirror: String, files: [URL]? = nil,
                      progress: @escaping (LocalSpeechInstallProgress) -> Void) throws -> URL {
    try ModelInstallationFixture.shared.install(progress: progress)
  }
  static func cancel(id: String) { ModelInstallationFixture.shared.cancel() }
  static func remove(root: URL, id: String) throws {}
  static func isValidMirror(_ value: String) -> Bool { true }
  static func message(for code: String) -> String { code }
}
enum LocalSpeechModelLocation {
  static func root() throws -> URL { URL(fileURLWithPath: "/synthetic/models") }
  static func resolve(storedPath: String, root: URL?) -> URL? { nil }
  static func names(_ path: String, model: String) -> Bool { false }
}
@MainActor enum MetasequoiaInputSessionBridge {
  static var writes = 0
  static func loadSharedPreferences() -> [String: Any]? { nil }
  static func updateSharedPreferences(_ change: (inout [String: Any]) -> Void) -> Bool { writes += 1; return true }
}
@MainActor final class SettingsAutosave {
  var hasPending = false
  func flush() {}
  func reject(_ message: String) {}
  func schedule(_ action: @escaping () throws -> Void) {}
}
@MainActor final class LocalSpeechEngine {
  static let shared = LocalSpeechEngine()
  func release() {}
}
struct ServiceFailure: Error { let message: String }

@main struct LocalModelCancellationTests {
  @MainActor static func main() async {
    let fixture = ModelInstallationFixture.shared
    let mode = CommandLine.arguments[2]
    fixture.holdRegistration = ["registration", "progress-blocked"].contains(mode)
    fixture.holdCompletion = ["leave", "blocked-cancel"].contains(mode)
    fixture.holdCancel = ["blocked-cancel", "progress-blocked"].contains(mode)
    let manager = LocalSpeechModelManager()
    let model = LocalSpeechModelInfo()
    let (events, continuation) = AsyncStream<Bool>.makeStream()
    let subscription = manager.$installing.sink { continuation.yield($0.contains(model.id)) }
    defer { subscription.cancel() }
    let start = {
      if CommandLine.arguments[1] == "import" {
        manager.importFiles(model, from: [URL(fileURLWithPath: "/synthetic/source")])
      } else { manager.install(model) }
    }
    start()
    if mode != "early" { await Task.detached { fixture.waitForWorker() }.value }
    if mode == "leave" {
      manager.cancelPending()
      guard manager.installing.contains(model.id) else {
        print("离页时旧安装尚未退出，却已经解除运行标记")
        exit(1)
      }
      start()
    } else { manager.cancel(model) }
    if fixture.holdCancel {
      if mode == "progress-blocked" { fixture.releaseWorker() }
      await Task.detached { fixture.waitForCancel() }.value
      // 共享取消尚未返回时主 actor 仍须能响应再次取消并解除测试屏障。
      manager.cancel(model)
      fixture.releaseCancel()
      await Task.detached { fixture.waitForCancelFinished() }.value
    }
    fixture.releaseWorker()
    var iterator = events.makeAsyncIterator()
    var started = false
    while let running = await iterator.next() {
      if running { started = true }
      if started && !running { break }
    }
    if mode == "early" && fixture.calls != 0 {
      print("启动前取消仍调用安装器")
      exit(1)
    }
    let cancelledStatus = CommandLine.arguments[1] == "import" ? "已取消导入。" : "local_model_cancelled"
    if mode != "leave" && manager.status != cancelledStatus {
      print("取消意图丢失：\(manager.status)")
      exit(1)
    }
    guard MetasequoiaInputSessionBridge.writes == 0 else { fatalError("取消后仍自动选用模型") }
    let calls = fixture.calls
    fixture.prepareRetry()
    start()
    started = false
    while let running = await iterator.next() {
      if running { started = true }
      if started && !running { break }
    }
    guard fixture.calls == calls + 1, manager.selectedPath == model.path else {
      print("重试未成功，可能复用了旧取消状态")
      exit(1)
    }
    print("模型取消及重试回归通过：\(CommandLine.arguments[1]) / \(mode)")
  }
}

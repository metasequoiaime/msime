import Combine
import Foundation

struct LocalSpeechModelInfo {
  var id = "synthetic-model"
  let title = "合成模型"
  let desktopOnly = false
  var installed = false
  var path: String { "/synthetic/models/" + id }
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
  var removalFails = false

  func remove() throws {
    condition.lock()
    waitForRelease()
    condition.unlock()
    if removalFails { throw LocalSpeechModelStore.HostFailure(code: "synthetic_remove_failed") }
  }

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
  static func remove(root: URL, id: String) throws { try ModelInstallationFixture.shared.remove() }
  static func isValidMirror(_ value: String) -> Bool { true }
  static func message(for code: String) -> String { code }
}
enum LocalSpeechModelLocation {
  static func root() throws -> URL { URL(fileURLWithPath: "/synthetic/models") }
  static func resolve(storedPath: String, root: URL?) -> URL? {
    storedPath.isEmpty ? nil : URL(fileURLWithPath: storedPath)
  }
  static func names(_ path: String, model: String) -> Bool {
    !path.isEmpty && URL(fileURLWithPath: path).lastPathComponent == model
  }
}
@MainActor enum MetasequoiaInputSessionBridge {
  static var writes = 0
  static var preferences: [String: Any] = [:]
  static func loadSharedPreferences() -> [String: Any]? { preferences }
  static func updateSharedPreferences(_ change: (inout [String: Any]) -> Void) -> Bool {
    writes += 1
    change(&preferences)
    return true
  }
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
    if CommandLine.arguments[1] == "remove" {
      await verifyRemovalSelection(mode: CommandLine.arguments[2])
      return
    }
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

  @MainActor static func verifyRemovalSelection(mode: String) async {
    let fixture = ModelInstallationFixture.shared
    fixture.removalFails = mode == "failure"
    let removed = LocalSpeechModelInfo(installed: true)
    let other = LocalSpeechModelInfo(id: "synthetic-other", installed: true)
    let manager = LocalSpeechModelManager()
    manager.use(mode == "select-removed" || mode == "unselected" ? other : removed)
    let (events, continuation) = AsyncStream<String>.makeStream()
    let subscription = manager.$status.sink { continuation.yield($0) }
    defer { subscription.cancel() }

    manager.remove(removed)
    await Task.detached { fixture.waitForWorker() }.value
    let previous = manager.selectedPath
    guard previous == (mode == "select-removed" || mode == "unselected" ? other.path : removed.path) else {
      fatalError("删除完成前已清空模型选择")
    }
    if mode == "switch-other" { manager.use(other) }
    if mode == "select-removed" { manager.use(removed) }
    let writesBeforeCompletion = MetasequoiaInputSessionBridge.writes
    fixture.releaseWorker()
    for await status in events {
      if status == "已删除“\(removed.title)”。" || status == "synthetic_remove_failed" { break }
    }
    let expected = mode == "failure" ? removed.path :
      (mode == "switch-other" || mode == "unselected" ? other.path : "")
    guard manager.selectedPath == expected else {
      print("删除完成后覆盖了当前选择：\(mode)，预期 \(expected)，实际 \(manager.selectedPath)")
      exit(1)
    }
    let persisted = (MetasequoiaInputSessionBridge.preferences["voice_input"] as? [String: Any])?["asr_model_path"] as? String
    guard persisted == expected else { fatalError("页面与保存的模型选择不一致") }
    let clearsSelection = expected.isEmpty
    guard MetasequoiaInputSessionBridge.writes == writesBeforeCompletion + (clearsSelection ? 1 : 0) else {
      fatalError("删除完成后写入了无关选择")
    }
    print("模型删除与选择回归通过：\(mode)")
  }
}

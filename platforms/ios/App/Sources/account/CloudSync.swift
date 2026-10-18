import SwiftUI

enum AccountRequestPolicy {
  static func isCancellation(_ error: Error) -> Bool {
    if error is CancellationError { return true }
    if let error = error as? URLError { return error.code == .cancelled }
    return false
  }
}

/// 应用的云同步控制器，对应 Android 的 `home/CloudSync`：设置（含当前自定义键盘皮肤）和无编码常用语。编排都在 Rust 的 `msime_client_cloud_sync`（#8201 迁移计划 4d）：开关、节流、首次选择、三方合并、Retry-After 退避和状态行的措辞，iOS 只在两处动手：交出 App Group 里的键盘设置（`IOSCloudSettings.native`），以及在一轮要写本机时先写 App Group、再让 Rust 提交（`pending_commit`）。
///
/// 只在登录账号且用户打开了同步开关时运行，登录本身不触发。应用每次回到前台（`scenePhase` 变为 `.active`）调用 `onActive`，按节流决定要不要跑。某个分类第一次同步而云端已有数据、或者云端被清空时，先弹选择让用户选，选了才执行。失败静默，下次回到前台再试；最近一次的失败原因在状态行里。个人词库不在自动同步里：用户词库由键盘里的 Engine 持有，应用进程导不出来。
@MainActor
final class CloudSync: ObservableObject {
  static let shared = CloudSync()

  /// 「我的」页显示的状态，每次跑完和每次 `refresh` 后更新。
  @Published private(set) var status = CloudSyncStatus()
  /// 「云端已有同步数据」或「云端数据已被清空」的选择是否正在显示。
  @Published var asksFirstRun = false
  /// 正在问的是「云端被清空」：多一个「关闭同步，保留本机」。
  @Published private(set) var cloudCleared = false

  private let state: CloudSyncState
  private let rust: @Sendable () throws -> BackendRustAccount
  private var running = false
  /// 用户在另一轮还在跑时做出的选择，那一轮结束后马上执行，不丢掉。
  private var pendingChoice: String?

  init(state: CloudSyncState = .shared,
       rust: @escaping @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() }) {
    self.state = state
    self.rust = rust
  }

  /// 回到前台时调用，按节流决定要不要跑。
  func onActive() { request(trigger: "active", choice: nil) }

  /// 用户刚打开同步开关时调用：不等节流，马上比对一次（首次需要选择时会弹出选择）。
  func runNow() { request(trigger: "enable", choice: nil) }

  /// 用户在选择里选了一项：`merge`、`use_cloud`，或者云端被清空时的 `disable`（关闭同步，保留本机）。
  func choose(_ choice: String) { request(trigger: "choice", choice: choice) }

  /// 与当前登录对齐后重新读状态。换了账号或已经退出时，Rust 在这一步关掉开关、清空进度。读不出会话时不对齐。
  func refresh() async {
    guard let status = try? await call(["operation": "status"]) else { return }
    if let decoded = try? CloudSyncStatus(status) { self.status = decoded }
  }

  /// 打开或关闭同步。打开时 Rust 把全部分类标成待上传，没有登录的账号时是 `cloud_sync_signed_out`；打开以后马上比对一次。
  func setEnabled(_ enabled: Bool) async throws {
    status = try CloudSyncStatus(try await call(["operation": "set_enabled", "enabled": enabled]))
    if enabled { runNow() }
  }

  /// 退出登录、注销或清除失效登录之前调用：Rust 清空进度、代次加一，进行中的那一轮因此提交不了。要在本机会话清掉之前调，否则请求带的会话来源已经看不到这个账号。
  func signOut() async {
    if let value = try? await call(["operation": "sign_out"]), let decoded = try? CloudSyncStatus(value) { status = decoded }
  }

  /// 发一个带会话来源的 action。`preferences_directory` 是共享偏好和常用语所在的目录。
  private func call(_ action: [String: Any]) async throws -> Any {
    let account = try rust()
    guard let directory = state.directory else { throw CloudSyncState.Failure.unavailable }
    let data = try await account.client.call(.cloudSync, [
      "session_store": account.store, "state_directory": directory.path,
      "preferences_directory": MetasequoiaInputSessionBridge.sharedStateDirectory, "action": action])
    return try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
  }

  private func request(trigger: String, choice: String?) {
    guard !running else {
      if let choice { pendingChoice = choice }
      return
    }
    running = true
    Task {
      let outcome = await perform(trigger: trigger, choice: choice)
      running = false
      if let next = pendingChoice {
        // 用户已经选过了，不再弹出选择。
        pendingChoice = nil
        request(trigger: "choice", choice: next)
        return
      }
      if case .needsChoice(let cleared) = outcome {
        cloudCleared = cleared
        asksFirstRun = true
      }
    }
  }

  enum Outcome: Equatable { case finished, needsChoice(cloudCleared: Bool) }

  /// 跑一轮：`run` 回答 `pending_commit` 时先把键盘设置写进 App Group，再提交；提交失败时把 App Group 恢复成旧值，这一轮作废，下一轮按三方合并重来。
  func perform(trigger: String, choice: String?) async -> Outcome {
    do {
      var action: [String: Any] = [
        "operation": "run", "trigger": trigger,
        "capabilities": ["settings": true, "skins_library": false, "phrases": true, "dictionary": false],
        // 社区皮肤试用中，共享文档里是用户还没决定保留的皮肤，这一轮不同步设置；待上传标记留着，用户保留或恢复之后再同步。
        "host_inputs": ["ios_native": try IOSCloudSettings.native(),
                        "settings_locked": (try? KeyboardSkinTrialStore())?.hasPendingTrial == true],
      ]
      if let choice { action["choice"] = choice }
      guard let result = try await call(action) as? [String: Any], let outcome = result["outcome"] as? String else {
        return .finished
      }
      if let status = try? CloudSyncStatus(result["status"]) { self.status = status }
      switch outcome {
      case "needs_choice": return .needsChoice(cloudCleared: result["reason"] as? String == "cloud_cleared")
      case "pending_commit":
        guard let runID = result["run_id"] as? String else { return .finished }
        await commit(runID: runID, writes: result["host_writes"] as? [String: Any] ?? [:])
      default: break
      }
    } catch {
      // 失败静默，下次回到前台再试；状态行照 Rust 记下的读一次。
      await refreshPeek()
    }
    return .finished
  }

  private func commit(runID: String, writes: [String: Any]) async {
    var previous: IOSCloudSettings.Feedback?
    var results: [String: Any] = [:]
    if let requested = writes["ios_native"] as? [String: Any] {
      guard let written = try? IOSCloudSettings.write(requested) else { return }
      previous = written.previous
      results["saved_native"] = written.saved
    }
    do {
      status = try CloudSyncStatus(try await call(["operation": "commit", "run_id": runID, "host_results": results]))
      IOSCloudSettings.mirrorDocument()
    } catch {
      // 共享偏好在这一轮读本机以后被别处改过（conflict）或写不进去（storage）：这一轮作废，App Group 回到旧值。
      if let previous { IOSCloudSettings.restore(previous) }
    }
  }

  private func refreshPeek() async {
    if let status = try? state.peek() { self.status = status }
  }
}

extension Binding where Value: Equatable {
  /// 用户经这个绑定改值时记一次设置改动，云同步下一轮上传。别处改了存储（例如同步应用云端设置）不经过绑定，不会被记成本机改动；`onChange` 分不出这两种来源。
  func markingSettingsChange() -> Binding<Value> {
    Binding(get: { wrappedValue }, set: { value in
      let changed = value != wrappedValue
      wrappedValue = value
      if changed { CloudSyncState.shared.markDirty(.settings) }
    })
  }
}

/// 首次同步时「云端已有同步数据」、云端被清空时「云端数据已被清空」的选择，挂在应用根视图上：回到前台时用户在哪个标签页都能看到。
struct CloudSyncFirstRunPrompt: ViewModifier {
  @ObservedObject var sync: CloudSync

  func body(content: Content) -> some View {
    content.confirmationDialog(sync.cloudCleared ? "云端数据已被清空" : "云端已有同步数据", isPresented: $sync.asksFirstRun,
                               titleVisibility: .visible) {
      Button("合并") { sync.choose("merge") }
      Button("使用云端") { sync.choose("use_cloud") }
      if sync.cloudCleared { Button("关闭同步，保留本机") { sync.choose("disable") } }
      Button("取消", role: .cancel) {}
    } message: {
      Text(sync.cloudCleared
           ? "云端的同步数据可能已在其他设备上删除。合并或使用云端都会保留本机的内容并传回云端；也可以关闭这台设备的同步，本机数据保持不变"
           : "合并会保留两边的内容，同一项以本机为准；使用云端会用云端的内容替换本机")
    }
  }
}

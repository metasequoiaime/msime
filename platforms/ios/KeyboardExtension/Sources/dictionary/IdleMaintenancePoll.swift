import Foundation

/// 两秒维护定时器每一轮要读的两份共享文件：个人词库的同步队列和词库快照队列。
///
/// 读取放在一条串行的后台队列上，主线程只在结果回来、并且仍然空闲时才去做真正的同步，绝大多数轮次里主线程什么文件都不碰。两份都只读文件、不取文件锁：键盘被挂起时后台线程即使读到一半，也不会持有共享容器里的锁。真正的同步（`PersonalDictionaryStore.synchronize`、快照激活）会关掉再重建引擎会话，而 `MetasequoiaInputSessionBridge` 的会话句柄只在主线程读写，所以那部分仍在主线程做。
@MainActor
final class IdleMaintenancePoll {
  struct Outcome: Sendable {
    /// 个人词库有没有要同步的内容；读不出来时是那个错误。
    let personalDictionary: Result<Bool, Error>
    let snapshotState: DictionarySnapshotWorker.PrefetchedState
  }
  private let store: PersonalDictionaryStore
  private let queue = DispatchQueue(label: "app.msime.ios.idle-maintenance", qos: .utility)
  private var generation: UInt64 = 0
  /// 上一次判定个人词库无事可做时的文件身份，文件没被替换就不再读取和解码。
  private var idleRevision: PersonalDictionaryStore.FileRevision?
  private(set) var isRunning = false

  init(store: PersonalDictionaryStore = PersonalDictionaryStore()) { self.store = store }

  /// 上一轮还没回来时不开新的一轮，返回 false。`completion` 在主线程调用；`cancel()` 之后才回来的结果直接丢弃。
  @discardableResult
  func start(readSnapshotState: @escaping @Sendable () -> DictionarySnapshotWorker.PrefetchedState,
             completion: @escaping @MainActor (Outcome) -> Void) -> Bool {
    guard !isRunning else { return false }
    isRunning = true
    generation &+= 1
    let generation = generation
    let store = store
    let idle = idleRevision
    queue.async { [weak self] in
      let personal = Result { try store.pollKeyboardSynchronization(idle: idle) }
      let snapshot = readSnapshotState()
      DispatchQueue.main.async {
        MainActor.assumeIsolated {
          guard let self, self.generation == generation else { return }
          self.isRunning = false
          self.idleRevision = (try? personal.get())?.idle
          completion(Outcome(personalDictionary: personal.map { $0.needed }, snapshotState: snapshot))
        }
      }
    }
    return true
  }

  /// 键盘收起时调用：正在后台读的那一轮回来后不再交付。
  func cancel() {
    generation &+= 1
    isRunning = false
  }
}

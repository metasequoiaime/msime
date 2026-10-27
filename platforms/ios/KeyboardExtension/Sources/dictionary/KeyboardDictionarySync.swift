import Foundation

/// 键盘扩展里的词库同步（`docs/dictionary-webdav-sync.md`）：用户在键盘里改了「我的词条」之后（应用了 App 排队的编辑、候选栏「删除词条」），以 `local_change` 调一次 Rust，把改动记成带时间戳的操作，时间戳因此接近改动发生的时刻。要不要联网由 Rust 按防抖决定：通常不联网，发送留给下次打开 App；待发的改动放了十分钟以上时这一次会顺带发出。拉取和写入远端改动只在 App 里做：键盘自己的会话占着词库，Rust 在移动端遇到「忙」就留到下一次。
///
/// 只有这些条件都满足才调（`IOSDictionarySyncPolicy.keyboardMayRecord`）：有完全访问（没有它就写不了 App Group 容器，也连不上网）；App 已经用这个网盘同步过，目录里已经有同步状态和词库密钥串，所以键盘从不生成设备身份、也不替 App 第一次登记；状态文件不超过 `keyboardStateLimit`，`run` 要把整份状态和「我的词条」读进内存，键盘扩展的内存上限远低于 App。不满足时什么也不做：App 下次打开时的那一次同步照样会记下这些改动，只是时间戳是那时的。
///
/// 调用在 `performExpiringActivity` 里做：键盘被收起时系统先等它结束，不会在它还拿着 App Group 里的 `dictionary-sync.lock` 时把扩展挂起（挂起时持有那里的文件锁会被系统结束，0xdead10cc）。系统连这点时间都不给（`expired`）时这一次不调。同时只有一个在跑，跑的时候又来的改动合并成跑完之后的一次。从不在输入线程上等它。
@MainActor
final class KeyboardDictionarySync {
  private let directory: URL
  private var running = false
  /// 跑的时候又来了改动：跑完再记一次。
  private var again = false
  private var request: (options: Data, deviceName: String)?

  init(directory: URL = IOSDictionarySync.directory) { self.directory = directory }

  /// 「我的词条」刚在键盘里变了。`options` 是会话准备好的 HostOptions（`dictionaryStateVersionOptions`），拿不到时不调。
  func recordLocalChange(options: Data?, fullAccess: Bool, deviceName: String) {
    guard fullAccess, let options else { return }
    request = (options, deviceName)
    guard !running else { again = true; return }
    start()
  }

  private func start() {
    guard let request else { return }
    running = true
    again = false
    let directory = self.directory
    let sync = IOSDictionarySync(options: request.options, directory: directory, deviceName: request.deviceName)
    let started = StartFlag()
    ProcessInfo.processInfo.performExpiringActivity(withReason: "app.msime.dictionary-sync.local_change") { [weak self] expired in
      // 系统要挂起扩展时会带着 expired 再调一次这个块：已经在跑的调用照常跑完（这个块返回之前扩展不会被挂起），还没开始的就不开始了。
      guard !expired else {
        if started.claimIfUnclaimed() { DispatchQueue.main.async { MainActor.assumeIsolated { self?.finished(started: false) } } }
        return
      }
      guard started.claimIfUnclaimed() else { return }
      if Self.mayRecord(directory: directory),
         let data = try? sync.encode(["operation": "run", "trigger": IOSDictionarySync.Trigger.localChange.rawValue]) {
        // 结果不显示在键盘里：失败（例如这时连不上网）由 Rust 记进状态，App 打开时会看到。
        _ = try? IOSDictionarySync.perform(data)
      }
      DispatchQueue.main.async { MainActor.assumeIsolated { self?.finished(started: true) } }
    }
  }

  private func finished(started: Bool) {
    guard running else { return }
    running = false
    // 被系统拒绝的那一次不再重试，下一次改词或打开 App 时再记。
    if started && again { start() }
  }

  /// 目录里已经有 App 留下的同步状态和密钥串、状态文件不大。只看文件属性，不读内容。
  nonisolated static func mayRecord(directory: URL) -> Bool {
    let manager = FileManager.default
    let state = (try? manager.attributesOfItem(atPath: directory.appendingPathComponent(IOSDictionarySync.stateFile).path))?[.size] as? NSNumber
    let keystore = manager.fileExists(atPath: directory.appendingPathComponent(IOSDictionarySync.keystoreFile).path)
    return IOSDictionarySyncPolicy.keyboardMayRecord(fullAccess: true, stateBytes: state?.intValue, keystorePresent: keystore)
  }
}

/// `performExpiringActivity` 的块可能被调两次（开始和到期），只有先到的那一次算数。
private final class StartFlag: @unchecked Sendable {
  private let lock = NSLock()
  private var claimed = false

  func claimIfUnclaimed() -> Bool {
    lock.lock()
    defer { lock.unlock() }
    guard !claimed else { return false }
    claimed = true
    return true
  }
}

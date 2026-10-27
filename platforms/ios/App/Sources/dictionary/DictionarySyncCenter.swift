import Foundation
import UIKit

/// 设置 App 一侧的词库同步触发（`docs/dictionary-webdav-sync.md` 的「宿主怎么接入」）：App 回到前台时以 `opened` 同步一次，之后按 Rust 给的 `next_check_ms` 以 `periodic` 再调，App 进了后台就停（iOS 不让 App 在后台常驻，后台的那一段由键盘扩展记录本机改动、下次打开时发出）。「词库同步」页的「立即同步」、对大批删除的选择和设备操作也经这里，结果交给页面显示。
///
/// 要不要联网、额度、防抖和退避全由 Rust 决定，这里只按它说的时间再调。偏好里没有 `dictionary_sync_webdav` 时不调 Rust，也不排定时。每个调用都在专用线程上、期间保持后台执行，挂起时不会还拿着 App Group 里的文件锁。
@MainActor
final class DictionarySyncCenter: ObservableObject {
  static let shared = DictionarySyncCenter()

  /// 最近一次 `run` 的结果；页面据此显示状态和错误。
  @Published private(set) var lastRun: IOSDictionarySync.Run?
  /// 正在跑的同步个数：页面据此禁用按钮，定时到了也不叠着再跑。
  @Published private(set) var running = 0
  private var timer: Task<Void, Never>?
  private var foreground = false

  /// 这个进程的 HostOptions，准备一次就一直用：准备时要校验整套词库资源，与键盘相同，选项在进程活着时都有效。
  private static var preparedOptions: Data?

  /// App 回到前台。
  func onActive() {
    foreground = true
    Task { _ = try? await run(.opened) }
  }

  /// App 进了后台：不再排定时。正在跑的那一次照常跑完（期间保持后台执行）。
  func onBackground() {
    foreground = false
    timer?.cancel()
    timer = nil
  }

  /// 偏好里现在的配置；没配置时为 nil。读一个本机的小文件。
  static func configuration() -> IOSDictionarySyncPolicy.Configuration? {
    IOSDictionarySyncPolicy.Configuration(preferences: MetasequoiaInputSessionBridge.loadSharedPreferences())
  }

  /// 跑一次同步，之后按 `next_check_ms` 重排定时。没有配置时抛出 `dictionary_sync_not_configured`；同步本身失败时不抛出，错误在结果的 `error` 里。
  @discardableResult
  func run(_ trigger: IOSDictionarySync.Trigger, confirmDeletions: Bool = false,
           restoreDeletions: Bool = false) async throws -> IOSDictionarySync.Run {
    guard Self.configuration() != nil else {
      timer?.cancel(); timer = nil
      throw IOSDictionarySyncFailure(code: "dictionary_sync_not_configured")
    }
    let result = try await perform { sync in
      try await sync.run(trigger, confirmDeletions: confirmDeletions, restoreDeletions: restoreDeletions)
    }
    lastRun = result
    schedule(after: result.next_check_ms)
    return result
  }

  /// 设备操作、状态和「测试连接」：与 `run` 用同一个入口。几个同时发起时由 Rust 的 `dictionary-sync.lock` 排队（键盘扩展也拿这把锁），定时到了而这里还有别的在跑，就过几秒再来。
  func perform<T: Sendable>(_ body: @escaping @Sendable (IOSDictionarySync) async throws -> T) async throws -> T {
    running += 1
    defer { running -= 1 }
    let sync = try await Self.open()
    return try await body(sync)
  }

  /// `remove`、`reset` 带回的那一次同步也算最近一次结果，并据此重排定时。
  func record(_ run: IOSDictionarySync.Run) {
    lastRun = run
    schedule(after: run.next_check_ms)
  }

  /// 配置改了（保存了新网盘或停止同步）：旧结果不再属于现在的配置。
  func forget() {
    lastRun = nil
    timer?.cancel()
    timer = nil
  }

  private func schedule(after milliseconds: UInt64) {
    timer?.cancel()
    timer = nil
    guard foreground else { return }
    // Rust 给的间隔至少一秒；这里再封顶 30 分钟（与共享设置页的 `DICTIONARY_SYNC_MAX_DELAY_MS` 相同），早到的那一次由 Rust 决定不联网。
    let delay = min(max(milliseconds, 1_000), 1_800_000)
    timer = Task { [weak self] in
      do { try await Task.sleep(nanoseconds: delay * 1_000_000) } catch { return }
      guard let self, self.foreground else { return }
      guard self.running == 0 else { self.schedule(after: 5_000); return }
      _ = try? await self.run(.periodic)
    }
  }

  /// 这个进程的同步入口。HostOptions 在主线程之外准备；设备名是给其他设备看的名称。
  private static func open() async throws -> IOSDictionarySync {
    let options: Data
    if let preparedOptions {
      options = preparedOptions
    } else {
      let prepared = await Task.detached(priority: .utility) { try? MetasequoiaInputSessionBridge.preparedHostOptions() }.value
      guard let prepared else { throw IOSDictionarySyncFailure(code: "unavailable") }
      preparedOptions = prepared
      options = prepared
    }
    return IOSDictionarySync(options: options, deviceName: IOSCloudClipboardDriver.deviceName(UIDevice.current),
                             activity: IOSRustAccount.backgroundActivity)
  }
}

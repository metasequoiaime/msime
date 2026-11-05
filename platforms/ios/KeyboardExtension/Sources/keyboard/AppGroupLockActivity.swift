import Foundation

/// 键盘持着 App Group 里的文件锁期间开着的一段 `ProcessInfo.performExpiringActivity`（#10910）。
///
/// 宿主应用在键盘显示时被结束，扩展收不到任何 UIKit 回调，iPhone XR（iOS 18.7.11）上约 40 ms 后就被挂起，手里还拿着词库访问租约，于是被系统以 0xdead10cc 结束（10 次里 10 次）。定时器轮询来不及；expiring activity 是系统给扩展的「挂起之前先告诉我」：活动开着时系统不挂起进程，要挂起之前在另一条线程上以 `expired == true` 再调一次同一个块，块返回之后才挂起。所以锁一取到就开着它，到期时在主线程上同步放锁再返回。
///
/// `performExpiringActivity` 的活动持续到第一次调用的块返回为止，所以那个块停在信号量上等 `end()`。到期调用和它是两条线程上并发的两次调用；系统一开始就不给活动（进程正要被挂起）时只有一次 `expired == true` 的调用，同样按到期处理。
final class AppGroupLockActivity: @unchecked Sendable {
  typealias Perform = (_ reason: String, _ block: @escaping @Sendable (_ expired: Bool) -> Void) -> Void

  /// 系统的入口；单测换成自己调块的假实现，模拟系统给活动、到期。
  nonisolated(unsafe) static var perform: Perform = { reason, block in
    ProcessInfo.processInfo.performExpiringActivity(withReason: reason, using: block)
  }

  /// 到期时等主线程放锁的上限。主线程这时通常空闲（宿主已经没了）；万一被别的同步调用占着，也不能让系统线程无限等下去。
  static let expiryWait: DispatchTimeInterval = .seconds(3)

  private final class Token: @unchecked Sendable {
    let finished = DispatchSemaphore(value: 0)
  }

  private let lock = NSLock()
  private var token: Token?
  private let reason: String

  init(reason: String) {
    self.reason = reason
  }

  deinit {
    token?.finished.signal()
  }

  /// 活动是否开着（`begin` 之后、`end` 或到期之前）。
  var isActive: Bool {
    lock.lock()
    defer { lock.unlock() }
    return token != nil
  }

  /// 开一段活动；已经开着时什么也不做。`expire` 在系统要挂起进程时于主线程上执行，必须在返回前放掉 App Group 里的全部锁；它返回之后（或等满 `expiryWait`）这段活动结束。
  func begin(expire: @escaping @MainActor () -> Void) {
    lock.lock()
    guard token == nil else {
      lock.unlock()
      return
    }
    let current = Token()
    token = current
    lock.unlock()
    Self.perform(reason) { [weak self] expired in
      guard expired else {
        current.finished.wait()
        return
      }
      Self.onMainAndWait(expire)
      self?.finish(current)
    }
  }

  /// 锁都放掉了，结束活动，让系统可以照常挂起。没开着时什么也不做。
  func end() {
    lock.lock()
    let current = token
    token = nil
    lock.unlock()
    current?.finished.signal()
  }

  private func finish(_ current: Token) {
    lock.lock()
    if token === current { token = nil }
    lock.unlock()
    current.finished.signal()
  }

  private static func onMainAndWait(_ work: @escaping @MainActor () -> Void) {
    if Thread.isMainThread {
      MainActor.assumeIsolated(work)
      return
    }
    let done = DispatchSemaphore(value: 0)
    DispatchQueue.main.async {
      MainActor.assumeIsolated(work)
      done.signal()
    }
    _ = done.wait(timeout: .now() + expiryWait)
  }
}

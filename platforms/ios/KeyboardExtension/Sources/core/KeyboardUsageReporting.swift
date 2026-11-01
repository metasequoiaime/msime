import Foundation
import os

@_silgen_name("msime_ios_crash_handlers_install")
private func msimeIOSCrashHandlersInstall()

/// 每次键盘展示管理自己的采集身份，网络队列不参与同意观察和会话收尾。
enum KeyboardUsageReporting {
  private static let queue = DispatchQueue(label: "app.msime.ios.keyboard.usage-reporting", qos: .utility)
  /// 只在网络队列上读写。
  private static var lastFlush: Date?
  private static var exceptionHandlerInstalled = false
  private static let previousExceptionHandler = OSAllocatedUnfairLock<(@convention(c) (NSException) -> Void)?>(initialState: nil)
  /// C 回调不能捕获上下文：只有字面闭包或全局函数能转成 C 函数指针，静态方法的引用不行，闭包里写全类型名调用它，不经隐式的 `self`。
  private static func handleUncaughtException(_ exception: NSException) {
    // 按自己的会话 id 写记录：标记可能已经属于另一个键盘进程刚开始的会话。
    if UsageReporting.exceptionConsent.isEnabled,
       let session = UsageReporting.keyboardCapture.current {
      UsageReporting.recordCrash(session: session,
                                 message: "Uncaught \(exception.name.rawValue)",
                                 stack: UsageReporting.frames(fromReturnAddresses: exception.callStackReturnAddresses))
    }
    // 关闭采集时也保留原处理器的处置；故障路径不能等待可能已被故障线程持有的锁。
    let previous = previousExceptionHandler.withLockIfAvailable({ $0 }) ?? nil
    previous?(exception)
  }

  /// 高频展示仅每十五分钟尝试发送一次，失败的队列由 App 或后续展示继续发送。
  private static let flushInterval: TimeInterval = 15 * 60

  /// viewWillAppear：发送需要完全访问权限，未授权时由 App 后续发送。
  static func presented(fullAccess: Bool) {
    msimeIOSCrashHandlersInstall()
    if !exceptionHandlerInstalled {
      exceptionHandlerInstalled = true
      previousExceptionHandler.withLock { $0 = NSGetUncaughtExceptionHandler() }
      NSSetUncaughtExceptionHandler { KeyboardUsageReporting.handleUncaughtException($0) }
    }
    UsageReporting.keyboardCapture.start {
      queue.async {
        guard fullAccess, lastFlush.map({ Date().timeIntervalSince($0) >= flushInterval }) ?? true else { return }
        lastFlush = Date()
        UsageReporting.flush()
      }
    }
  }

  /// viewWillDisappear：正常收起，不等待可能阻塞的网络发送。收起后扩展随时可能被挂起，而最后一拍轮询和会话收尾都会取 App Group 偏好与队列的文件锁，挂起时还持着锁会以 0xdead10cc 被杀：收起总是排入，再在过期活动里等已排入的生命周期操作做完才放手；系统不给时间（`expired`）时不等。块可能被调两次（开始和到期），等待只在没过期的那一次里做。
  static func dismissed() {
    UsageReporting.keyboardCapture.stop()
    ProcessInfo.processInfo.performExpiringActivity(withReason: "app.msime.crash-consent.stop") { expired in
      guard !expired else { return }
      UsageReporting.keyboardCapture.waitForPendingOperations()
    }
  }
}

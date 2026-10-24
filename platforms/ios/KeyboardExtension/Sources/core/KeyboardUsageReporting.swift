import Foundation
import os

@_silgen_name("msime_ios_crash_record_path_set")
private func msimeIOSCrashRecordPathSet(_ path: UnsafePointer<CChar>?)
@_silgen_name("msime_ios_crash_handlers_install")
private func msimeIOSCrashHandlersInstall()

/// One usage-reporting session per keyboard presentation (UsageReporting). The file work runs on a utility queue so it never delays the keyboard appearing.
enum KeyboardUsageReporting {
  private static let queue = DispatchQueue(label: "app.msime.ios.keyboard.usage-reporting", qos: .utility)
  /// Read and written on `queue` only.
  private static var lastFlush: Date?
  private static var exceptionHandlerInstalled = false
  /// 正在进行的会话 id。在 `queue` 上写；未捕获异常处理函数可能在任何线程读它，所以放在锁里。
  private static let currentSession = OSAllocatedUnfairLock<String?>(initialState: nil)
  /// C 回调不能捕获上下文：只有字面闭包或全局函数能转成 C 函数指针，静态方法的引用不行，闭包里写全类型名调用它，不经隐式的 `self`。
  private static func handleUncaughtException(_ exception: NSException) {
    // 按自己的会话 id 写记录：标记可能已经属于另一个键盘进程刚开始的会话。
    guard let session = currentSession.withLock({ $0 }) else { return }
    UsageReporting.recordCrash(session: session,
                               message: "Uncaught \(exception.name.rawValue): \(exception.reason ?? "")",
                               stack: exception.callStackSymbols.joined(separator: "\n"))
  }

  /// A keyboard is shown many times a day; its events wait in the App Group queue between sends.
  private static let flushInterval: TimeInterval = 15 * 60

  /// viewWillAppear. Sending needs Full Access; without it the app sends the queue the next time it opens.
  static func presented(fullAccess: Bool) {
    msimeIOSCrashHandlersInstall()
    if !exceptionHandlerInstalled {
      exceptionHandlerInstalled = true
      NSSetUncaughtExceptionHandler { KeyboardUsageReporting.handleUncaughtException($0) }
    }
    queue.async {
      let session = UsageReporting.begin()
      currentSession.withLock { $0 = session?.id }
      guard let session else { msimeIOSCrashRecordPathSet(nil); return }
      session.crashRecordPath.withCString { msimeIOSCrashRecordPathSet($0) }
      guard fullAccess, lastFlush.map({ Date().timeIntervalSince($0) >= flushInterval }) ?? true else { return }
      lastFlush = Date()
      UsageReporting.flush()
    }
  }

  /// viewWillDisappear: the presentation ended normally.
  static func dismissed() {
    queue.async {
      msimeIOSCrashRecordPathSet(nil)
      let session = currentSession.withLock { current in
        defer { current = nil }
        return current
      }
      // 只结束本进程自己开始的会话；上报关着时没有会话，也就什么都不做。
      if let session { UsageReporting.end(session: session) }
    }
  }
}

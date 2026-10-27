import Foundation
import MetricKit
import os

/// 系统的崩溃报告来自 MetricKit：它在 App 之后某次启动时送来 App 和键盘扩展两者的崩溃（含 ObjC 异常、Swift trap 和系统强制结束）。每一条经 `UsageReporting.recordDiagnosedCrash` 写成崩溃记录，由键盘下次启动时排队：键盘扩展的真崩溃按进程号对回它当时未结束的会话，补报那个会话的 `session_crash`，键盘自己的处理程序已经记过的不再记第二份；系统强制结束和 App 自身的崩溃不属于任何会话，只报 `crash`。
///
/// App 主线程上未捕获的 ObjC 异常经 CFRunLoop 重新抛出后才 abort，MetricKit 的线程栈里只剩 `main` 和系统帧，常常也不带异常名。所以 App 另装一个未捕获异常处理器，带着异常名和抛出点的栈先记一份；MetricKit 事后送来同一进程的诊断时跳过。
final class CrashDiagnostics: NSObject, MXMetricManagerSubscriber {
  static let shared = CrashDiagnostics()

  private struct Recording {
    let directory: URL
    let handledProcesses: URL
    let previous: (@convention(c) (NSException) -> Void)?
  }

  /// 处理器可能在任何线程上运行，而它在工具队列上安装：放在锁里。上报关着时为 nil，处理器什么都不写。
  private static let recording = OSAllocatedUnfairLock<Recording?>(initialState: nil)

  /// 主 App 处理器记下过崩溃的进程号，App 私有目录。
  private static var handledProcesses: URL? {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
      .appendingPathComponent("uncaught-exception-pids")
  }

  func start() {
    MXMetricManager.shared.add(self)
    // 上报开关在共享偏好里，读它要经 Rust 读盘，不放在启动的主线程上；安装之前极短的窗口里抛出的异常只有 MetricKit 的诊断。
    DispatchQueue.global(qos: .utility).async {
      guard UsageReporting.isEnabled, let directory = UsageReporting.directory,
            let handledProcesses = Self.handledProcesses else { return }
      try? FileManager.default.createDirectory(at: handledProcesses.deletingLastPathComponent(),
                                               withIntermediateDirectories: true)
      Self.recording.withLock {
        $0 = Recording(directory: directory, handledProcesses: handledProcesses,
                       previous: NSGetUncaughtExceptionHandler())
      }
      NSSetUncaughtExceptionHandler { CrashDiagnostics.handleUncaughtException($0) }
    }
  }

  /// C 回调不能捕获上下文：闭包里写全类型名调用它，不经隐式的 `self`。
  private static func handleUncaughtException(_ exception: NSException) {
    guard let recording = recording.withLock({ $0 }) else { return }
    UsageReporting.recordUncaughtException(name: exception.name.rawValue,
                                           returnAddresses: exception.callStackReturnAddresses, pid: getpid(),
                                           in: recording.directory, handledProcesses: recording.handledProcesses)
    recording.previous?(exception)
  }

  func didReceive(_ payloads: [MXDiagnosticPayload]) {
    guard UsageReporting.isEnabled, let directory = UsageReporting.directory else { return }
    for payload in payloads {
      for crash in payload.crashDiagnostics ?? [] {
        // App 自己的处理器已经带着抛出点的栈记下了这次崩溃。
        if let handledProcesses = Self.handledProcesses,
           UsageReporting.takeHandledProcess(crash.metaData.pid, in: handledProcesses) { continue }
        let summary = UsageReporting.diagnosticSummary(exceptionType: crash.exceptionType, exceptionCode: crash.exceptionCode,
                                                       signal: crash.signal, exceptionName: crash.exceptionReason?.exceptionName,
                                                       terminationReason: crash.terminationReason)
        UsageReporting.recordDiagnosedCrash(summary,
                                            stack: UsageReporting.frames(fromCallStackTree: crash.callStackTree.jsonRepresentation()),
                                            pid: crash.metaData.pid, in: directory)
      }
    }
  }
}

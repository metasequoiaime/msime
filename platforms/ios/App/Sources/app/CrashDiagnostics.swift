import Foundation
import MetricKit
import os
import UIKit

@_silgen_name("msime_ios_crash_handlers_install")
private func installCrashSignals()

/// 系统的崩溃报告来自 MetricKit：它在 App 之后某次启动时送来 App 和键盘扩展两者的崩溃（含 ObjC 异常、Swift trap 和系统强制结束）。每一条经 `UsageReporting.recordDiagnosedCrash` 写成崩溃记录，由键盘下次启动时排队：键盘扩展的真崩溃按进程号对回它当时未结束的会话，补报那个会话的 `session_crash`，键盘自己的处理程序已经记过的不再记第二份；系统强制结束和 App 自身的崩溃不属于任何会话，只报 `crash`。
///
/// App 主线程上未捕获的 ObjC 异常经 CFRunLoop 重新抛出后才 abort，MetricKit 的线程栈里只剩 `main` 和系统帧，常常也不带异常名。所以 App 另装一个未捕获异常处理器，带着异常名和抛出点的栈先记一份；MetricKit 事后送来同一进程的诊断时跳过。
///
/// App 的信号处理函数按准备时的捕获 UUID 写现场记录。MetricKit 事后送来同一进程的崩溃时按进程号取回那个身份补录，与现场记录共用同一个 UUID，不另报一份。
final class CrashDiagnostics: NSObject, MXMetricManagerSubscriber {
  static let shared = CrashDiagnostics()

  private struct Recording {
    let directory: URL
    let handledProcesses: URL
    let previous: (@convention(c) (NSException) -> Void)?
  }

  /// 处理器可能在任何线程上运行，而它在工具队列上安装：放在锁里，权限由 `UsageReporting.exceptionConsent` 单独维护。
  private static let recording = OSAllocatedUnfairLock<Recording?>(initialState: nil)

  /// 主 App 处理器记下过崩溃的进程号，App 私有目录。
  private static var handledProcesses: URL? {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
      .appendingPathComponent("uncaught-exception-pids")
  }

  func start() {
    installCrashSignals()
    MXMetricManager.shared.add(self)
    observeBackground()
    // 上报开关在共享偏好里，读它要经 Rust 读盘，不放在启动的主线程上；安装之前极短的窗口里抛出的异常只有 MetricKit 的诊断。
    DispatchQueue.global(qos: .utility).async {
      UsageReporting.applicationCapture.start()
      guard let directory = UsageReporting.directory,
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

  /// 授权轮询每一拍都持 App Group 偏好的文件锁，App 被挂起时还持着它会以 0xdead10cc 被杀：进后台时停下轮询，后台任务覆盖到已在进行的那一拍结束；回前台再恢复。身份不清，后台里的故障仍按原身份记录。
  private func observeBackground() {
    let center = NotificationCenter.default
    center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: nil) { _ in
      let end = IOSRustAccount.backgroundActivity("crash-consent-pause")
      UsageReporting.applicationCapture.pause { end() }
    }
    center.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: nil) { _ in
      UsageReporting.applicationCapture.resume()
    }
  }

  /// C 回调不能捕获上下文：闭包里写全类型名调用它，不经隐式的 `self`。
  private static func handleUncaughtException(_ exception: NSException) {
    guard let recording = recording.withLockIfAvailable({ $0 }) ?? nil else { return }
    if UsageReporting.exceptionConsent.isEnabled, let capture = UsageReporting.applicationCapture.current {
      UsageReporting.recordUncaughtException(name: exception.name.rawValue,
                                           returnAddresses: exception.callStackReturnAddresses, pid: getpid(),
                                           in: recording.directory, handledProcesses: recording.handledProcesses, capture: capture)
    }
    recording.previous?(exception)
  }

  func didReceive(_ payloads: [MXDiagnosticPayload]) {
    guard case let .enabled(authorization)? = UsageReporting.captureConsent(),
          let directory = UsageReporting.directory else { return }
    for payload in payloads {
      guard UsageReporting.diagnosticAllowed(begin: payload.timeStampBegin, end: payload.timeStampEnd, consentSince: authorization.since) else { continue }
      for hang in payload.hangDiagnostics ?? [] {
        // 卡死诊断不代表进程崩溃，也不能计入键盘的 session_crash。
        UsageReporting.recordDiagnosedCrash(.init(message: "Hang main thread", killed: true),
          stack: UsageReporting.frames(fromCallStackTree: hang.callStackTree.jsonRepresentation()),
          pid: 0, version: hang.applicationVersion,
          diagnosticID: UsageReporting.diagnosticID(kind: "hang", data: hang.jsonRepresentation()), in: directory, authorization: authorization, begin: payload.timeStampBegin, end: payload.timeStampEnd)
      }
      for crash in payload.crashDiagnostics ?? [] {
        // 取出即删，异常处理器已记下的进程也不留下这条身份。
        let capture = Self.applicationCapture(of: crash.metaData, begin: payload.timeStampBegin, end: payload.timeStampEnd)
        // App 自己的处理器已经带着抛出点的栈记下了这次崩溃。
        if let handledProcesses = Self.handledProcesses,
           UsageReporting.takeHandledProcess(crash.metaData.pid, in: handledProcesses) { continue }
        let summary = UsageReporting.diagnosticSummary(exceptionType: crash.exceptionType, exceptionCode: crash.exceptionCode,
                                                       signal: crash.signal, exceptionName: crash.exceptionReason?.exceptionName,
                                                       terminationReason: crash.terminationReason)
        let stack = UsageReporting.frames(fromCallStackTree: crash.callStackTree.jsonRepresentation())
        if let capture, !summary.killed {
          // 信号处理函数可能已按这个身份写过现场记录；共用 UUID，一次崩溃只报一次。系统强制结束收不到信号，仍走下面的单独记录。
          UsageReporting.recordDiagnosedCapture(summary, stack: stack, capture: capture, in: directory,
                                                authorization: authorization, begin: payload.timeStampBegin, end: payload.timeStampEnd)
          continue
        }
        UsageReporting.recordDiagnosedCrash(summary,
                                            stack: stack,
                                            pid: crash.metaData.pid, version: crash.applicationVersion,
                                            diagnosticID: UsageReporting.diagnosticID(kind: "crash", data: crash.jsonRepresentation()), in: directory, authorization: authorization, begin: payload.timeStampBegin, end: payload.timeStampEnd)
      }
    }
  }

  /// App 进程准备捕获时记下的身份。键盘扩展的诊断也由 App 收到，系统给出包标识时只认 App 自己的；没有包标识时由 `takeApplicationCapture` 按诊断时间窗只认窗口前最近一次启动的 App 进程，进程号被扩展复用时不会错认旧身份。
  private static func applicationCapture(of metaData: MXMetaData, begin: Date, end: Date) -> UsageReporting.Session? {
    guard let identities = UsageReporting.applicationCaptureIdentities else { return nil }
    if #available(iOS 26.0, *), !metaData.bundleIdentifier.isEmpty,
       metaData.bundleIdentifier != Bundle.main.bundleIdentifier { return nil }
    return UsageReporting.takeApplicationCapture(metaData.pid, begin: begin, end: end, in: identities)
  }
}

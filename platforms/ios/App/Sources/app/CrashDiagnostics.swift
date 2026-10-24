import Foundation
import MetricKit

/// 系统的崩溃报告来自 MetricKit：它在 App 之后某次启动时送来 App 和键盘扩展两者的崩溃（含 ObjC 异常、Swift trap 和系统强制结束）。每一条经 `UsageReporting.recordDiagnosedCrash` 写成崩溃记录，由键盘下次启动时排队：键盘扩展的真崩溃按进程号对回它当时未结束的会话，补报那个会话的 `session_crash`，键盘自己的处理程序已经记过的不再记第二份；系统强制结束和 App 自身的崩溃不属于任何会话，只报 `crash`。
final class CrashDiagnostics: NSObject, MXMetricManagerSubscriber {
  static let shared = CrashDiagnostics()

  func start() { MXMetricManager.shared.add(self) }

  func didReceive(_ payloads: [MXDiagnosticPayload]) {
    guard UsageReporting.isEnabled, let directory = UsageReporting.directory else { return }
    for payload in payloads {
      for crash in payload.crashDiagnostics ?? [] {
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

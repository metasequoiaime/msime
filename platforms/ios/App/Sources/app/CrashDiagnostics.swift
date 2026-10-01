import Foundation
import MetricKit

/// The app's crash reports come from MetricKit, which delivers a crash on a later launch, ObjC exceptions and Swift traps included. Each becomes a crash record (UsageReporting.storeCrashDiagnostic) that the keyboard's next start queues; the app is not the input method, so it has no session to mark as crashed.
final class CrashDiagnostics: NSObject, MXMetricManagerSubscriber {
  static let shared = CrashDiagnostics()

  func start() { MXMetricManager.shared.add(self) }

  func didReceive(_ payloads: [MXDiagnosticPayload]) {
    guard UsageReporting.isEnabled, let directory = UsageReporting.directory else { return }
    for payload in payloads {
      for crash in payload.crashDiagnostics ?? [] {
        UsageReporting.storeCrashDiagnostic(message: Self.summary(crash),
                                            stack: UsageReporting.frames(fromCallStackTree: crash.callStackTree.jsonRepresentation()),
                                            in: directory)
      }
    }
  }

  /// The exception type, signal and the system's termination reason: numbers and system text only. The ObjC exception's composed message is left out because it can quote app data.
  private static func summary(_ crash: MXCrashDiagnostic) -> String {
    var parts = ["Crash"]
    if let type = crash.exceptionType { parts.append("exception \(type)") }
    if let code = crash.exceptionCode { parts.append("code \(code)") }
    if let signal = crash.signal { parts.append("signal \(signal)") }
    if let name = crash.exceptionReason?.exceptionName, !name.isEmpty { parts.append(name) }
    if let reason = crash.terminationReason, !reason.isEmpty { parts.append(reason) }
    return parts.joined(separator: " ").replacingOccurrences(of: "\n", with: " ")
  }
}

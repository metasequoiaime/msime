import Foundation

enum VerificationCodePolicy {
  static func remainingSeconds(until deadline: Date, now: Date) -> Int {
    max(0, Int(ceil(deadline.timeIntervalSince(now))))
  }
}

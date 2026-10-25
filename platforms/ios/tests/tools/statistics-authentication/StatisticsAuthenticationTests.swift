import Foundation
import LocalAuthentication

private final class AuthenticationContext: StatisticsAuthenticationContext {
  var available = true
  var unavailableError: NSError?
  var reply: (@Sendable (Bool, Error?) -> Void)?
  var invalidated = false
  var evaluations = 0

  func canEvaluatePolicy(_ policy: LAPolicy, error: NSErrorPointer) -> Bool {
    precondition(policy == .deviceOwnerAuthentication)
    error?.pointee = unavailableError
    return available
  }
  func evaluatePolicy(_ policy: LAPolicy, localizedReason: String, reply: @escaping @Sendable (Bool, Error?) -> Void) {
    evaluations += 1
    self.reply = reply
  }
  func invalidate() { invalidated = true }
}

@main
struct StatisticsAuthenticationTests {
  @MainActor
  static func main() async {
    for code in [LAError.passcodeNotSet, .biometryNotAvailable, .biometryLockout, .systemCancel] {
      let context = AuthenticationContext()
      context.available = false
      context.unavailableError = NSError(domain: LAError.errorDomain, code: code.rawValue)
      let gate = StatisticsAuthentication(makeContext: { context })
      gate.authenticate(automatically: true)
      precondition(gate.authorized == (code == .passcodeNotSet))
      precondition(!gate.busy && context.evaluations == 0)
    }
    let first = AuthenticationContext()
    let second = AuthenticationContext()
    var contexts = [first, second]
    let gate = StatisticsAuthentication(makeContext: { contexts.removeFirst() })
    gate.authenticate(automatically: true)
    gate.authenticate(automatically: true)
    precondition(first.evaluations == 1 && gate.busy && !gate.authorized)
    gate.lock()
    precondition(first.invalidated && !gate.authorized)
    gate.authenticate(automatically: true)
    first.reply?(true, nil)
    await Task.yield()
    precondition(!gate.authorized && gate.busy)
    second.reply?(false, nil)
    while gate.busy { await Task.yield() }
    precondition(!gate.authorized)
    gate.authenticate(automatically: true)
    precondition(second.evaluations == 1)
    let retry = AuthenticationContext()
    contexts.append(retry)
    gate.authenticate()
    retry.reply?(true, nil)
    while gate.busy { await Task.yield() }
    precondition(gate.authorized)
    gate.lock()
    precondition(!gate.authorized)
    let disabled = AuthenticationContext()
    let enabledAgain = AuthenticationContext()
    var choices = [disabled, enabledAgain]
    let configurable = StatisticsAuthentication(makeContext: { choices.removeFirst() })
    configurable.authenticate()
    configurable.authenticate(enabled: false)
    precondition(configurable.authorized && disabled.invalidated && !configurable.busy)
    disabled.reply?(true, nil)
    await Task.yield()
    configurable.lock()
    configurable.authenticate(enabled: true)
    precondition(!configurable.authorized && enabledAgain.evaluations == 1)
    configurable.lock()
    configurable.authenticate(enabled: false)
    precondition(configurable.authorized && !configurable.busy)
    print("iOS 统计认证：无密码、失败、重试、后台和过期回调检查通过")
  }
}

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
      let gate = StatisticsAuthentication(makeContext: { context }, cooldown: StatisticsAuthenticationCooldown())
      gate.authenticate(automatically: true)
      precondition(gate.authorized == (code == .passcodeNotSet))
      precondition(!gate.busy && context.evaluations == 0)
    }
    let first = AuthenticationContext()
    let second = AuthenticationContext()
    var contexts = [first, second]
    let gate = StatisticsAuthentication(makeContext: { contexts.removeFirst() }, cooldown: StatisticsAuthenticationCooldown())
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
    let configurable = StatisticsAuthentication(makeContext: { choices.removeFirst() }, cooldown: StatisticsAuthenticationCooldown())
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
    // 冷却：认证成功后、或已认证时离开之后五分钟内再进入不弹认证，满五分钟再要求；关闭开关或未设密码得到的授权不算。
    let start = ContinuousClock.now
    var elapsed = Duration.zero
    let cooldown = StatisticsAuthenticationCooldown(now: { start + elapsed })
    let verifiedOnce = AuthenticationContext()
    let afterCooldown = AuthenticationContext()
    var cooled = [verifiedOnce, afterCooldown]
    let timed = StatisticsAuthentication(makeContext: { cooled.removeFirst() }, cooldown: cooldown)
    timed.authenticate(automatically: true)
    verifiedOnce.reply?(true, nil)
    while timed.busy { await Task.yield() }
    precondition(timed.authorized)
    elapsed = .seconds(4 * 60)
    timed.lock()
    elapsed = .seconds(8 * 60)
    timed.authenticate(automatically: true)
    precondition(timed.authorized && !timed.busy && cooled.count == 1, "离开后四分钟内回来不弹认证，冷却从离开时算起")
    let another = StatisticsAuthentication(makeContext: { cooled.removeFirst() }, cooldown: cooldown)
    another.authenticate(automatically: true)
    precondition(another.authorized && cooled.count == 1, "冷却跨页面实例共用")
    elapsed = .seconds(9 * 60)
    another.lock()
    timed.lock()
    elapsed = .seconds(14 * 60)
    timed.authenticate(automatically: true)
    precondition(!timed.authorized && verifiedOnce.evaluations == 1 && afterCooldown.evaluations == 1, "满五分钟重新认证")
    timed.lock()
    let passcodeless = AuthenticationContext()
    passcodeless.available = false
    passcodeless.unavailableError = NSError(domain: LAError.errorDomain, code: LAError.passcodeNotSet.rawValue)
    let fresh = StatisticsAuthenticationCooldown(now: { start + elapsed })
    let unverified = StatisticsAuthentication(makeContext: { passcodeless }, cooldown: fresh)
    unverified.authenticate(enabled: false)
    unverified.lock()
    unverified.authenticate()
    precondition(unverified.authorized)
    unverified.lock()
    precondition(!fresh.isActive, "关闭开关或未设密码的授权不开启冷却")
    print("iOS 统计认证：无密码、失败、重试、后台、过期回调和冷却检查通过")
  }
}

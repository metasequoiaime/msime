import Foundation
import LocalAuthentication

private final class AuthenticationContext: ClipboardDeleteAuthenticationContext {
  var available = true
  var reply: (@Sendable (Bool, Error?) -> Void)?
  var invalidated = false
  var evaluations = 0
  var reason = ""

  func canEvaluatePolicy(_ policy: LAPolicy, error: NSErrorPointer) -> Bool {
    precondition(policy == .deviceOwnerAuthentication)
    if !available { error?.pointee = NSError(domain: LAError.errorDomain, code: LAError.passcodeNotSet.rawValue) }
    return available
  }
  func evaluatePolicy(_ policy: LAPolicy, localizedReason: String, reply: @escaping @Sendable (Bool, Error?) -> Void) {
    precondition(policy == .deviceOwnerAuthentication)
    evaluations += 1
    reason = localizedReason
    self.reply = reply
  }
  func invalidate() { invalidated = true }
}

/// #10408：「剪贴板删除数据验证」的切换规则、删除前的判断、只在本设备的存储，以及认证器的一次性、取消和迟到回答。
@main
struct ClipboardDeleteAuthenticationTests {
  @MainActor
  static func main() async {
    typealias Policy = ClipboardDeleteAuthenticationPolicy
    // 值不变什么也不做；打开和关闭都要认证，第一次打开也一样；没有锁屏密码时两个方向都拒绝。
    for enabled in [false, true] {
      for available in [false, true] {
        precondition(Policy.decide(enabled: enabled, next: enabled, authenticationAvailable: available) == .unchanged)
      }
    }
    precondition(Policy.decide(enabled: false, next: true, authenticationAvailable: true) == .authenticate)
    precondition(Policy.decide(enabled: true, next: false, authenticationAvailable: true) == .authenticate)
    precondition(Policy.decide(enabled: false, next: true, authenticationAvailable: false) == .unavailable)
    precondition(Policy.decide(enabled: true, next: false, authenticationAvailable: false) == .unavailable)
    // 删除：关着直接删，开着先认证，开着而没有锁屏密码时不删。
    precondition(Policy.gate(enabled: false, authenticationAvailable: false) == .delete)
    precondition(Policy.gate(enabled: false, authenticationAvailable: true) == .delete)
    precondition(Policy.gate(enabled: true, authenticationAvailable: true) == .authenticate)
    precondition(Policy.gate(enabled: true, authenticationAvailable: false) == .unavailable)
    precondition(Policy.refusal(.unavailable, next: true) == Policy.unavailableMessage)
    precondition(Policy.unavailableMessage.contains("锁屏密码"))
    precondition(Policy.refusal(.failed, next: true).contains("保持关闭"))
    precondition(Policy.refusal(.failed, next: false).contains("保持开启"))

    // 存储：默认关，存进去的值在重启后读回。
    let suite = "app.msime.tests.clipboard-delete-authentication.\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suite)!
    defer { defaults.removePersistentDomain(forName: suite) }
    let store = ClipboardDeleteAuthenticationStore(defaults: defaults)
    precondition(!store.enabled)
    store.save(true)
    precondition(ClipboardDeleteAuthenticationStore(defaults: defaults).enabled)
    store.save(false)
    precondition(!ClipboardDeleteAuthenticationStore(defaults: defaults).enabled)

    // 没有锁屏密码：不弹认证，直接回答不可用。
    let missing = AuthenticationContext()
    missing.available = false
    let refusing = ClipboardDeleteAuthenticator(makeContext: { missing })
    precondition(!refusing.available())
    var outcomes: [ClipboardDeleteAuthenticator.Outcome] = []
    refusing.authenticate(reason: "删除剪贴板历史") { outcomes.append($0) }
    precondition(outcomes == [.unavailable] && missing.evaluations == 0 && !refusing.busy)

    // 认证进行中再请求不叠第二个认证框；通过回答 authenticated，失败回答 failed。
    let first = AuthenticationContext()
    let second = AuthenticationContext()
    var contexts = [first, second]
    let authenticator = ClipboardDeleteAuthenticator(makeContext: { contexts.removeFirst() })
    outcomes = []
    authenticator.authenticate(reason: "删除剪贴板历史") { outcomes.append($0) }
    precondition(authenticator.busy && first.evaluations == 1 && first.reason == "删除剪贴板历史")
    authenticator.authenticate(reason: "删除剪贴板历史") { _ in preconditionFailure("a second prompt must not start") }
    precondition(contexts.count == 1)
    first.reply?(true, nil)
    while authenticator.busy { await Task.yield() }
    precondition(outcomes == [.authenticated])
    authenticator.authenticate(reason: "关闭剪贴板删除数据验证") { outcomes.append($0) }
    second.reply?(false, NSError(domain: LAError.errorDomain, code: LAError.userCancel.rawValue))
    while authenticator.busy { await Task.yield() }
    precondition(outcomes == [.authenticated, .failed])

    // 面板收起时取消：进行中的认证被作废，迟到的通过不再回答，不会删任何东西。
    let late = AuthenticationContext()
    let cancelling = ClipboardDeleteAuthenticator(makeContext: { late })
    var lateOutcomes: [ClipboardDeleteAuthenticator.Outcome] = []
    cancelling.authenticate(reason: "删除剪贴板历史") { lateOutcomes.append($0) }
    cancelling.cancel()
    precondition(late.invalidated && !cancelling.busy)
    late.reply?(true, nil)
    for _ in 0..<20 { await Task.yield() }
    precondition(lateOutcomes.isEmpty)
    print("ClipboardDeleteAuthenticationTests ok")
  }
}

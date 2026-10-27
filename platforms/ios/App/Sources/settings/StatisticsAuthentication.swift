import Combine
import LocalAuthentication

protocol StatisticsAuthenticationContext: AnyObject {
  func canEvaluatePolicy(_ policy: LAPolicy, error: NSErrorPointer) -> Bool
  func evaluatePolicy(_ policy: LAPolicy, localizedReason: String, reply: @escaping @Sendable (Bool, Error?) -> Void)
  func invalidate()
}

extension LAContext: StatisticsAuthenticationContext {}

/// 认证成功、或在已认证状态下离开统计页（含退到后台）时记下时刻，之后 `duration` 之内再进入不再要求认证。只在内存里，用含睡眠时间的单调时钟，App 被结束或重启设备后重新认证，改系统时间也绕不过去。
@MainActor
final class StatisticsAuthenticationCooldown {
  static let shared = StatisticsAuthenticationCooldown()
  static let duration: Duration = .seconds(5 * 60)
  private let now: () -> ContinuousClock.Instant
  private var lastAuthorized: ContinuousClock.Instant?

  init(now: @escaping () -> ContinuousClock.Instant = { ContinuousClock.now }) {
    self.now = now
  }

  func mark() { lastAuthorized = now() }

  var isActive: Bool {
    guard let lastAuthorized else { return false }
    let elapsed = lastAuthorized.duration(to: now())
    return elapsed >= .zero && elapsed < Self.duration
  }
}

/// 仅系统明确表示未配置密码时免认证；临时故障和取消不能变成通行凭据。
@MainActor
final class StatisticsAuthentication: ObservableObject {
  static let preferenceKey = "statisticsAuthenticationEnabled"
  /// 默认关闭：每次看统计都要验证太打扰，需要的人在「我的」里打开。
  static let enabledByDefault = false
  @Published private(set) var authorized = false
  @Published private(set) var busy = false
  @Published private(set) var message = ""
  private var context: StatisticsAuthenticationContext?
  private let makeContext: () -> StatisticsAuthenticationContext
  private let cooldown: StatisticsAuthenticationCooldown
  private var generation = 0
  private var attempted = false
  /// 本次授权来自系统认证或冷却期，而不是关闭开关或未设密码：只有这种授权才延长冷却。
  private var verified = false

  init(makeContext: @escaping () -> StatisticsAuthenticationContext = { LAContext() },
       cooldown: StatisticsAuthenticationCooldown? = nil) {
    self.makeContext = makeContext
    self.cooldown = cooldown ?? .shared
  }

  func authenticate(automatically: Bool = false, enabled: Bool = true) {
    if !enabled {
      lock()
      authorized = true
      return
    }
    guard !busy, !authorized else { return }
    if cooldown.isActive {
      attempted = true
      verified = true
      authorized = true
      message = ""
      return
    }
    guard !automatically || !attempted else { return }
    attempted = true
    generation += 1
    let request = generation
    let context = makeContext()
    self.context = context
    busy = true
    message = ""
    var error: NSError?
    guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
      busy = false
      self.context = nil
      if error?.domain == LAError.errorDomain && error?.code == LAError.passcodeNotSet.rawValue {
        authorized = true
      } else {
        message = "系统认证暂不可用，请重试。"
      }
      return
    }
    context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: "查看打字统计") { [weak self] success, _ in
      Task { @MainActor [weak self] in
        guard let self, self.generation == request else { return }
        self.context = nil
        self.busy = false
        self.authorized = success
        self.verified = success
        if success { self.cooldown.mark() }
        self.message = success ? "" : "未完成身份验证，请重试。"
      }
    }
  }

  func lock() {
    if verified && authorized { cooldown.mark() }
    verified = false
    generation += 1
    context?.invalidate()
    context = nil
    authorized = false
    attempted = false
    busy = false
    message = ""
  }
}

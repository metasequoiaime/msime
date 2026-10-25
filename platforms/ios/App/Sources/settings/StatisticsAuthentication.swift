import Combine
import LocalAuthentication

protocol StatisticsAuthenticationContext: AnyObject {
  func canEvaluatePolicy(_ policy: LAPolicy, error: NSErrorPointer) -> Bool
  func evaluatePolicy(_ policy: LAPolicy, localizedReason: String, reply: @escaping @Sendable (Bool, Error?) -> Void)
  func invalidate()
}

extension LAContext: StatisticsAuthenticationContext {}

/// 仅系统明确表示未配置密码时免认证；临时故障和取消不能变成通行凭据。
@MainActor
final class StatisticsAuthentication: ObservableObject {
  static let preferenceKey = "statisticsAuthenticationEnabled"
  @Published private(set) var authorized = false
  @Published private(set) var busy = false
  @Published private(set) var message = ""
  private var context: StatisticsAuthenticationContext?
  private let makeContext: () -> StatisticsAuthenticationContext
  private var generation = 0
  private var attempted = false

  init(makeContext: @escaping () -> StatisticsAuthenticationContext = { LAContext() }) {
    self.makeContext = makeContext
  }

  func authenticate(automatically: Bool = false, enabled: Bool = true) {
    if !enabled {
      lock()
      authorized = true
      return
    }
    guard !busy, !authorized, !automatically || !attempted else { return }
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
        self.message = success ? "" : "未完成身份验证，请重试。"
      }
    }
  }

  func lock() {
    generation += 1
    context?.invalidate()
    context = nil
    authorized = false
    attempted = false
    busy = false
    message = ""
  }
}

import Foundation
import LocalAuthentication

/// 「剪贴板删除数据验证」（#10408）的切换规则与删除前的判断，与 Android 的 `ClipboardDeleteAuthenticationPolicy`、鸿蒙的同名策略逐条相同。
///
/// 开关默认关闭。打开后，键盘剪贴板面板里删除本机记录（「管理记录」菜单的「删除」和确认后的「清空」）都要先通过系统认证；App 的「剪贴板历史」页同样受它约束，否则开关形同虚设。打开和关闭都要认证，第一次打开也不例外。设备没设锁屏密码时一律不许切换；打开以后锁屏密码被移除，删除和关闭都做不了，只有重新设锁屏密码才能解开，与 Android 的「防止截屏」先例相同。iOS 键盘没有访客剪贴板，也没有删除撤销，所以只看开关。
enum ClipboardDeleteAuthenticationPolicy {
  /// 系统认证不可用时给用户的提示：设置页拒绝切换和键盘里拒绝删除都用这一句。
  static let unavailableMessage = "请先在系统设置中开启锁屏密码（可搭配面容 ID 或触控 ID）后再使用"

  enum Decision: Equatable {
    /// 下一值与当前值相同，什么也不做。
    case unchanged
    /// 先通过系统认证再保存。
    case authenticate
    /// 设备没有可用的系统认证，拒绝切换并保持原值。
    case unavailable
  }

  enum Gate: Equatable {
    /// 开关关着，直接删。
    case delete
    /// 先通过系统认证再删。
    case authenticate
    /// 开关开着但设备没有可用的系统认证：不删，提示先开启锁屏密码。
    case unavailable
  }

  static func decide(enabled: Bool, next: Bool, authenticationAvailable: Bool) -> Decision {
    if next == enabled { return .unchanged }
    return authenticationAvailable ? .authenticate : .unavailable
  }

  static func gate(enabled: Bool, authenticationAvailable: Bool) -> Gate {
    if !enabled { return .delete }
    return authenticationAvailable ? .authenticate : .unavailable
  }

  /// 切换没成功时设置页给的提示；`next` 是用户想切到的值。
  static func refusal(_ outcome: ClipboardDeleteAuthenticator.Outcome, next: Bool) -> String {
    if outcome == .unavailable { return unavailableMessage }
    return next ? "未通过系统认证，剪贴板删除数据验证保持关闭" : "未通过系统认证，剪贴板删除数据验证保持开启"
  }
}

/// 开关存在 App Group 的 UserDefaults 里，和隐私模式（`KeyboardPrivacyPreference`）一样不在共享偏好文档里，所以不随账号同步，只留在本设备上。键盘只读不写。
struct ClipboardDeleteAuthenticationStore {
  static let key = "keyboard.clipboard.deleteAuthentication"
  let defaults: UserDefaults

  var enabled: Bool { defaults.bool(forKey: Self.key) }

  func save(_ enabled: Bool) { defaults.set(enabled, forKey: Self.key) }
}

protocol ClipboardDeleteAuthenticationContext: AnyObject {
  func canEvaluatePolicy(_ policy: LAPolicy, error: NSErrorPointer) -> Bool
  func evaluatePolicy(_ policy: LAPolicy, localizedReason: String, reply: @escaping @Sendable (Bool, Error?) -> Void)
  func invalidate()
}

extension LAContext: ClipboardDeleteAuthenticationContext {}

/// 一次系统认证（锁屏密码，可用时由面容 ID 或触控 ID 代替），不带冷却，每次都弹。设置 App 和键盘扩展共用：`LocalAuthentication` 在头文件里没有标成扩展不可用，键盘扩展里同样调用；拉不起认证界面时系统回答失败，按未通过处理，绝不放行。
@MainActor
final class ClipboardDeleteAuthenticator {
  enum Outcome: Equatable {
    case authenticated
    /// 取消、失败、被系统打断，或者认证界面拉不起来。
    case failed
    /// 设备此刻没有可用的系统认证（没设锁屏密码）。
    case unavailable
  }

  private let makeContext: () -> ClipboardDeleteAuthenticationContext
  private var context: ClipboardDeleteAuthenticationContext?
  private var generation = 0
  /// 认证界面开着时为真：再次请求直接忽略，不叠第二个认证框。
  private(set) var busy = false

  init(makeContext: @escaping () -> ClipboardDeleteAuthenticationContext = { LAContext() }) {
    self.makeContext = makeContext
  }

  /// 系统认证此刻是否可用：设了锁屏密码才算。
  func available() -> Bool {
    var error: NSError?
    return makeContext().canEvaluatePolicy(.deviceOwnerAuthentication, error: &error)
  }

  /// 弹出系统认证，结束后在主线程回答；进行中再调用时什么也不做。
  func authenticate(reason: String, completion: @escaping @MainActor (Outcome) -> Void) {
    guard !busy else { return }
    let context = makeContext()
    var error: NSError?
    guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
      completion(.unavailable)
      return
    }
    generation += 1
    let request = generation
    self.context = context
    busy = true
    context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason) { [weak self] success, _ in
      Task { @MainActor [weak self] in
        guard let self, self.generation == request else { return }
        self.context = nil
        self.busy = false
        completion(success ? .authenticated : .failed)
      }
    }
  }

  /// 面板收起或页面离开时结束进行中的认证；迟到的回答被丢掉，不会再删任何东西。
  func cancel() {
    generation += 1
    context?.invalidate()
    context = nil
    busy = false
  }
}

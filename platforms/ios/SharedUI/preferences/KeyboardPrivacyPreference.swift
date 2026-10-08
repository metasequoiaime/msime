import Foundation

/// 隐私模式，键盘功能菜单里用于不留痕迹输入的开关：打开期间词库不从输入内容中学习，也不记录输入统计、按键、剪贴板历史和诊断日志，不碰云剪贴板，具体由 `KeyboardPrivacyGate` 判断。默认关闭。
///
/// 它和键盘的声音、振动开关一起放在 App Group 里，而不是共享偏好文档里，所以不会同步，只留在本设备上，与 Android 的做法一致。没有完全访问权限时键盘无法写入 App Group，开关只保持到键盘下次出现为止。
enum KeyboardPrivacyPreference {
  static let incognitoKey = "keyboard.privacy.incognito"

  static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }

  static var incognito: Bool {
    get { defaults.bool(forKey: incognitoKey) }
    set { defaults.set(newValue, forKey: incognitoKey) }
  }
}

/// 「是否记录」的统一判断，对应 Android 的 `ImePrivacyGate`：打字统计、按键计数、语音时长、剪贴板历史、云剪贴板和诊断日志都先问这里。
///
/// 两种情况下一律不记：隐私模式开着，或当前是密码、新密码、一次性验证码这类凭据输入框（`KeyboardViewController.isCredentialField`）。Android 的第三种情况是带 `IME_FLAG_NO_PERSONALIZED_LEARNING` 的输入框，iOS 的输入框没有对应的标记。除此之外各项仍按各自的开关判断（统计开关、剪贴板历史、云剪贴板的服务端开关），这里不管。Android 的输入事件日志和语音贡献 iOS 没有，所以不在 `Record` 里。
struct KeyboardPrivacyGate: Equatable {
  /// 受隐私规则约束的各类记录。
  enum Record: CaseIterable {
    /// 上屏字数的打字统计，以及与它同口径的换装统计（Android 的 `recordsTyping`）。
    case typing
    /// 按键热力图。
    case keys
    /// 语音输入时长。
    case voiceDuration
    /// 把系统剪贴板存进剪贴板历史。
    case clipboardHistory
    /// 云剪贴板：读取、插入和发送都算。
    case cloudClipboard
    /// 「诊断日志」（`diagnostic_log.server`）。
    case diagnosticLog
  }

  /// 隐私模式（`KeyboardPrivacyPreference.incognito`）是否开着。
  let incognito: Bool
  /// 当前输入框是否是凭据输入框。
  let credentialField: Bool

  /// 是否处在不记任何东西的情况下。
  var suppressed: Bool { incognito || credentialField }

  /// 某一类记录在当前状态下是否允许：两种隐私情况下全部为假，其他情况下全部为真。
  func allows(_ record: Record) -> Bool { !suppressed }

  /// 是否经 `msime_client_set_private_session` 把会话标成隐私会话，与 Android 的 `learningSuppressed` 同口径：只看隐私模式（Android 另有不允许学习的输入框，iOS 没有）。用户在设置里关掉学习不算，凭据输入框也不算，与 Android 的密码框一致。
  var privateSession: Bool { incognito }
}

import UIKit

/// 只声明公开方法的 Objective-C 签名，动态调用保留系统的参数与 block 桥接。
@objc private protocol KeyboardURLOpening {
  @objc(openURL:options:completionHandler:)
  func openKeyboardURL(_ url: URL, options: NSDictionary, completionHandler: ((Bool) -> Void)?)
}

/// 从键盘扩展的响应链请求打开本应用，不直接访问扩展不可用的 UIApplication.shared。
/// 使用带 options 与 completionHandler 的公开方法，避免已废弃 openURL: 的无响应。
enum KeyboardAppLauncher {
  /// 键盘要打开的目标。scheme 是本版本的 `MSIMEAppEdition.urlScheme`,在 `platforms/ios/project.yml` 的 `CFBundleURLTypes` 里注册;按版本区分,装了多个版本时才不会拉起另一个版本的 App。
  static let settingsURL = URL(string: "\(MSIMEAppEdition.urlScheme)://settings")!
  /// 语音录音页。iOS 键盘扩展拿不到麦克风,录音只能在应用里做,识别结果再经语音交接回到键盘。
  static let voiceURL = URL(string: "\(MSIMEAppEdition.urlScheme)://voice")!
  /// 功能菜单里的「词库」：打开应用的词库页。
  static let dictionaryURL = URL(string: "\(MSIMEAppEdition.urlScheme)://settings/dictionary")!
  /// 功能菜单里的「设置」和方案选择里的「添加语言」：打开应用的「输入」页。
  static let inputSettingsURL = URL(string: "\(MSIMEAppEdition.urlScheme)://settings/input")!
  /// 常用语面板里的「添加常用语」：打开应用的常用语页，iOS 上只能在那里添加常用语。
  static let phrasesURL = URL(string: "\(MSIMEAppEdition.urlScheme)://settings/phrases")!
  /// 长按工具栏上的任何按钮：打开应用的「键盘工具栏」页，在那里选择工具栏显示哪些按钮。
  static let toolbarSettingsURL = URL(string: "\(MSIMEAppEdition.urlScheme)://settings/toolbar")!
  /// 功能菜单里的「反馈」：打开「我的」下的「帮助与反馈」。
  static let feedbackURL = URL(string: "\(MSIMEAppEdition.urlScheme)://feedback")!
  /// 功能菜单里的「关于」：打开「我的」下的关于页。
  static let aboutURL = URL(string: "\(MSIMEAppEdition.urlScheme)://about")!

  /// 返回值表示是否已发出请求；系统实际打开结果由 completion 返回。
  @discardableResult
  static func open(_ url: URL, from responder: UIResponder, completion: ((Bool) -> Void)? = nil) -> Bool {
    let selector = sel_registerName("openURL:options:completionHandler:")
    var current: UIResponder? = responder
    while let candidate = current {
      // UIScene 与 UIApplication 的选择器相同，但 options 类型不同，必须走各自签名。
      if let scene = candidate as? UIScene {
        scene.open(url, options: nil, completionHandler: completion)
        return true
      }
      if candidate.responds(to: selector) {
        (candidate as AnyObject).openKeyboardURL?(url, options: [:], completionHandler: completion)
        return true
      }
      current = candidate.next
    }
    completion?(false)
    return false
  }
}

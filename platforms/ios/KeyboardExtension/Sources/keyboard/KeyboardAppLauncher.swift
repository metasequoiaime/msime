import UIKit

/// 从键盘扩展里打开本应用。
///
/// 扩展编译时 `UIApplication.shared` 不可用,而 `NSExtensionContext.open(_:)` 在输入法这一类扩展上不生效 —— 它只对今日小组件和 iMessage 应用有效。剩下的办法是沿响应链往上找:键盘视图最终挂在宿主应用的窗口上,链上那个应用对象照样应答 `openURL:`。调的是它公开的那个方法,只是拿不到通常那个引用。
///
/// 独立成一个类型而不是写在控制器里,是为了能测:响应链可以由测试自己搭一条假的,而控制器要跑起来得有整个输入法环境。
enum KeyboardAppLauncher {
  /// 键盘要打开的目标。scheme 是本版本的 `MSIMEAppEdition.urlScheme`,在 `platforms/ios/project.yml` 的 `CFBundleURLTypes` 里注册;按版本区分,装了多个版本时才不会拉起另一个版本的 App。
  static let settingsURL = URL(string: "\(MSIMEAppEdition.urlScheme)://settings")!
  /// 语音录音页。iOS 键盘扩展拿不到麦克风,录音只能在应用里做,识别结果再经语音交接回到键盘。
  static let voiceURL = URL(string: "\(MSIMEAppEdition.urlScheme)://voice")!

  /// 返回值是「找到了能打开的对象」,不是「应用已经到前台」—— 后者由系统决定,扩展这边看不到结果。
  @discardableResult
  static func open(_ url: URL, from responder: UIResponder) -> Bool {
    let selector = sel_registerName("openURL:")
    var current: UIResponder? = responder
    while let candidate = current {
      if candidate.responds(to: selector) {
        candidate.perform(selector, with: url)
        return true
      }
      current = candidate.next
    }
    return false
  }
}

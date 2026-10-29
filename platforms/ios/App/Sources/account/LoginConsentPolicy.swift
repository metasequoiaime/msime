import Foundation

/// 登录前的明示同意，规则与共享设置页的 `useLoginConsent`（`packages/ui/src/account/login-consent.tsx`）和 Android 的 `LoginConsentPolicy` 相同：勾选框默认不勾，用户自己勾上之前，发送验证码、验证码登录、Google 和 Apple 登录都不发出任何请求。
///
/// 没勾就点了这些操作时 `confirm()` 返回 false 并进入提示状态，界面显示 `prompt`；勾上后提示消失。状态只活在这一次打开的登录界面里，不落盘：每次打开登录都要重新勾。AppGallery 审核按《个人信息保护法》的「明示同意」驳回过「登录即表示同意」的写法，见 `.agents/notes/implemented/bug-fix/2026-10-27-login-privacy-consent.md`。
struct LoginConsentPolicy: Equatable {
  static let termsTitle = "《用户协议》"
  static let privacyTitle = "《隐私政策》"
  static let text = "我已阅读并同意\(termsTitle)和\(privacyTitle)"
  /// 没勾选就去登录时的提示。
  static let prompt = "请先阅读并勾选同意\(termsTitle)和\(privacyTitle)"
  static let termsURL = URL(string: "https://msime.app/terms/")!
  static let privacyURL = URL(string: "https://msime.app/privacy/")!

  private(set) var agreed = false
  /// 是否正在显示 `prompt`。
  private(set) var prompting = false

  mutating func setAgreed(_ value: Bool) {
    agreed = value
    if value { prompting = false }
  }

  /// 登录操作发请求前调用：已勾选返回 true；没勾选时进入提示状态并返回 false，调用方不发请求。
  mutating func confirm() -> Bool {
    if agreed { return true }
    prompting = true
    return false
  }
}

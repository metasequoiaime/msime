import XCTest

/// 登录面板的明示同意：默认不勾，没勾时每个登录操作都被挡下并显示提示，勾上后提示消失、操作放行，取消勾选后再次挡下。
final class LoginConsentPolicyTests: XCTestCase {
  func testStartsUncheckedAndRefusesUntilChecked() {
    var consent = LoginConsentPolicy()
    XCTAssertFalse(consent.agreed)
    XCTAssertFalse(consent.prompting)

    XCTAssertFalse(consent.confirm())
    XCTAssertTrue(consent.prompting)
    XCTAssertFalse(consent.confirm())

    consent.setAgreed(true)
    XCTAssertFalse(consent.prompting)
    XCTAssertTrue(consent.confirm())
    XCTAssertFalse(consent.prompting)

    consent.setAgreed(false)
    XCTAssertFalse(consent.prompting)
    XCTAssertFalse(consent.confirm())
    XCTAssertTrue(consent.prompting)
  }

  func testWordingAndDocumentsMatchTheSharedLoginConsent() {
    XCTAssertEqual(LoginConsentPolicy.text, "我已阅读并同意《用户协议》和《隐私政策》")
    XCTAssertEqual(LoginConsentPolicy.prompt, "请先阅读并勾选同意《用户协议》和《隐私政策》")
    XCTAssertEqual(LoginConsentPolicy.termsURL.absoluteString, "https://msime.app/terms/")
    XCTAssertEqual(LoginConsentPolicy.privacyURL.absoluteString, "https://msime.app/privacy/")
  }
}

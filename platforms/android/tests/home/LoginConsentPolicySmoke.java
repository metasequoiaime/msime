import app.msime.android.home.LoginConsentPolicy;

/** 登录面板的明示同意：默认不勾，没勾时每个登录操作都被挡下并显示提示，勾上后提示消失、操作放行，取消勾选后再次挡下。面板怎么画、哪些操作先问它由 check-host.sh 钉住接线。 */
public final class LoginConsentPolicySmoke {
    public static void main(String[] arguments) {
        LoginConsentPolicy consent = new LoginConsentPolicy();
        check(!consent.agreed(), "the box starts unchecked");
        check(!consent.prompting(), "no prompt before any attempt");

        check(!consent.confirm(), "an attempt without consent is refused");
        check(consent.prompting(), "a refused attempt shows the prompt");
        check(!consent.confirm(), "a second attempt is still refused");

        consent.setAgreed(true);
        check(!consent.prompting(), "checking the box clears the prompt");
        check(consent.confirm(), "an attempt after checking proceeds");
        check(!consent.prompting(), "a permitted attempt shows no prompt");

        consent.setAgreed(false);
        check(!consent.prompting(), "unchecking alone does not prompt");
        check(!consent.confirm(), "unchecking refuses the next attempt");
        check(consent.prompting(), "and prompts again");

        check(!new LoginConsentPolicy().confirm(), "every new sheet starts without consent");

        check(LoginConsentPolicy.TEXT.equals("我已阅读并同意《用户协议》和《隐私政策》"), "consent text");
        check(LoginConsentPolicy.PROMPT.equals("请先阅读并勾选同意《用户协议》和《隐私政策》"), "prompt text");
        check(LoginConsentPolicy.TEXT.contains(LoginConsentPolicy.TERMS_TITLE)
            && LoginConsentPolicy.TEXT.contains(LoginConsentPolicy.PRIVACY_TITLE), "both titles are in the line");
        check(LoginConsentPolicy.TERMS_URL.equals("https://msime.app/terms/"), "terms url");
        check(LoginConsentPolicy.PRIVACY_URL.equals("https://msime.app/privacy/"), "privacy url");
        System.out.println("Android login consent policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

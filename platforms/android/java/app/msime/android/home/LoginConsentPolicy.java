package app.msime.android.home;

/**
 * 登录前的明示同意，规则与共享设置页的 `useLoginConsent`（`packages/ui/src/account/login-consent.tsx`）相同：勾选框默认不勾，用户自己勾上之前，发送验证码、验证码登录、Google 和 Apple 登录都不发出任何请求。
 *
 * <p>没勾就点了这些操作时 {@link #confirm} 返回 false 并进入提示状态，界面显示 {@link #PROMPT}；勾上后提示消失。状态只活在这一次打开的登录面板里，不落盘：每次打开登录都要重新勾。AppGallery 审核按《个人信息保护法》的「明示同意」驳回过「登录即表示同意」的写法，见 `.agents/notes/implemented/bug-fix/2026-10-27-login-privacy-consent.md`。
 */
public final class LoginConsentPolicy {
    public static final String TERMS_TITLE = "《用户协议》";
    public static final String PRIVACY_TITLE = "《隐私政策》";
    public static final String TEXT = "我已阅读并同意" + TERMS_TITLE + "和" + PRIVACY_TITLE;
    /** 没勾选就去登录时的提示。 */
    public static final String PROMPT = "请先阅读并勾选同意" + TERMS_TITLE + "和" + PRIVACY_TITLE;
    public static final String TERMS_URL = "https://msime.app/terms/";
    public static final String PRIVACY_URL = "https://msime.app/privacy/";

    private boolean agreed;
    private boolean prompting;

    public boolean agreed() {
        return agreed;
    }

    /** 是否正在显示 {@link #PROMPT}。 */
    public boolean prompting() {
        return prompting;
    }

    public void setAgreed(boolean value) {
        agreed = value;
        if (value) prompting = false;
    }

    /** 登录操作发请求前调用：已勾选返回 true；没勾选时进入提示状态并返回 false，调用方不发请求。 */
    public boolean confirm() {
        if (agreed) return true;
        prompting = true;
        return false;
    }
}

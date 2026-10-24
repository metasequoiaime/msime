package app.msime.android;

/** #9606：关闭剪贴板历史会清空本机记录，必须先通过系统设备认证。 */
public final class ClipboardHistoryAuthenticationPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(ClipboardHistoryAuthenticationPolicy.requiresAuthentication(true, false),
            "turning clipboard history off must require device authentication");
        check(!ClipboardHistoryAuthenticationPolicy.requiresAuthentication(false, false),
            "turning an already disabled history off needs no authentication");
        check(!ClipboardHistoryAuthenticationPolicy.requiresAuthentication(false, true),
            "turning clipboard history on needs no authentication");
        check(!ClipboardHistoryAuthenticationPolicy.requiresAuthentication(true, true),
            "leaving clipboard history enabled needs no authentication");
        System.out.println("ClipboardHistoryAuthenticationPolicySmoke ok");
    }
}

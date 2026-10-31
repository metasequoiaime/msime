import app.msime.android.ClipboardDeleteAuthenticationPolicy;

public final class ClipboardDeleteAuthenticationPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(!ClipboardDeleteAuthenticationPolicy.requiresAuthentication(false, false),
            "主模式默认不要求认证");
        check(ClipboardDeleteAuthenticationPolicy.requiresAuthentication(true, false),
            "主模式开启门禁后要求认证");
        check(ClipboardDeleteAuthenticationPolicy.requiresAuthentication(false, true),
            "访客模式始终要求认证");
        check(ClipboardDeleteAuthenticationPolicy.requiresAuthentication(true, true),
            "访客模式开启门禁仍要求认证");
        System.out.println("ClipboardDeleteAuthenticationPolicySmoke ok");
    }
}

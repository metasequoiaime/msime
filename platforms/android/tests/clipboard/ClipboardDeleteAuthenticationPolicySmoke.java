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
        check(ClipboardDeleteAuthenticationPolicy.allowsWithoutAuthenticator(false),
            "没有锁屏凭据时，仅来自访客模式的门禁照旧直接删除");
        check(!ClipboardDeleteAuthenticationPolicy.allowsWithoutAuthenticator(true),
            "用户打开开关后，没有锁屏凭据时仍拒绝删除");
        check(ClipboardDeleteAuthenticationPolicy.togglesWithoutAuthenticator(false),
            "没有锁屏凭据时可以不经认证关闭开关，开关不能变成关不掉的陷阱");
        check(!ClipboardDeleteAuthenticationPolicy.togglesWithoutAuthenticator(true),
            "没有锁屏凭据时不能打开开关");
        System.out.println("ClipboardDeleteAuthenticationPolicySmoke ok");
    }
}

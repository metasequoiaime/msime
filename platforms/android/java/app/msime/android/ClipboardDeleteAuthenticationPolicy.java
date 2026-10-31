package app.msime.android;

/** 剪贴板单条或多选删除的系统认证门禁；访客剪贴板无条件要求认证。 */
public final class ClipboardDeleteAuthenticationPolicy {
    private ClipboardDeleteAuthenticationPolicy() {}

    public static boolean requiresAuthentication(boolean enabled, boolean visitorMode) {
        return enabled || visitorMode;
    }
}

package app.msime.android;

/** 剪贴板单条或多选删除的系统认证门禁；访客剪贴板无条件要求认证。 */
public final class ClipboardDeleteAuthenticationPolicy {
    private ClipboardDeleteAuthenticationPolicy() {}

    public static boolean requiresAuthentication(boolean enabled, boolean visitorMode) {
        return enabled || visitorMode;
    }

    /**
     * 设备没有任何可用的系统认证（没有锁屏凭据）时是否照旧直接删除：只有用户没打开开关、门禁仅来自访客模式时才放行。没有锁屏的设备谁都能操作，这时拦下访客删除保护不了数据，只会让访客剪贴板删不掉；用户自己打开过开关则按开关拒绝，提示先设置锁屏凭据。
     */
    public static boolean allowsWithoutAuthenticator(boolean enabled) {
        return !enabled;
    }

    /**
     * 设备没有可用的系统认证时，切换开关能否不经认证直接生效：只放行关闭。打开开关后又移除了锁屏凭据的用户，认证永远满足不了，关闭也要认证就再也关不掉；没有锁屏的设备认证本来保护不了任何东西。打开开关仍要求先设置锁屏凭据，否则设置页显示「已开启」而删除全部被拒。
     */
    public static boolean togglesWithoutAuthenticator(boolean next) {
        return !next;
    }
}

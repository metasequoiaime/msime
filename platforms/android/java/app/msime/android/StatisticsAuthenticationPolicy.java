package app.msime.android;

/** 统计页在已配置系统认证时要求认证；未设置锁屏凭据时直接查看。 */
public final class StatisticsAuthenticationPolicy {
    private StatisticsAuthenticationPolicy() {}

    public static boolean requiresAuthentication(boolean authorized, boolean deviceSecure) {
        return deviceSecure && !authorized;
    }
}

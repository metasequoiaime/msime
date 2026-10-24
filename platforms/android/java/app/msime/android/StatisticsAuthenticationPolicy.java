package app.msime.android;

/** 统计页每次进入都要认证，离开页面后授权失效。 */
public final class StatisticsAuthenticationPolicy {
    private StatisticsAuthenticationPolicy() {}

    public static boolean requiresAuthentication(boolean authorized) { return !authorized; }
}

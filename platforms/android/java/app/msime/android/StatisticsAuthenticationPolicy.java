package app.msime.android;

/** 统计页在已配置系统认证时要求认证；未设置锁屏凭据时直接查看。 */
public final class StatisticsAuthenticationPolicy {
    /** 认证成功、或在已认证状态下离开统计页之后，这么久之内再进入（含从后台回来）不用重新认证。 */
    public static final long COOLDOWN_MS = 5 * 60_000L;

    private StatisticsAuthenticationPolicy() {}

    /**
     * {@code lastAuthorizedMs} 是最后一次处于已认证状态的时刻，从未认证过为 0；两个时刻都取 {@code SystemClock.elapsedRealtime()}（含深度睡眠的开机毫秒数），只放在内存里，进程结束或重启设备后重新认证，改系统时间也绕不过去。
     */
    public static boolean withinCooldown(long lastAuthorizedMs, long nowMs) {
        return lastAuthorizedMs > 0 && nowMs >= lastAuthorizedMs && nowMs - lastAuthorizedMs < COOLDOWN_MS;
    }

    public static boolean requiresAuthentication(boolean authorized, boolean deviceSecure) {
        return deviceSecure && !authorized;
    }
}

package app.msime.android;

/** 统计页无锁屏凭据可直接查看；已配置时取消、失败和再次进入仍需认证。 */
public final class StatisticsAuthenticationPolicySmoke {
    public static void main(String[] args) {
        check(!StatisticsAuthenticationPolicy.requiresAuthentication(false, false),
            "未配置锁屏凭据时不能挡住统计页");
        check(!StatisticsAuthenticationPolicy.requiresAuthentication(true, false),
            "移除锁屏凭据后仍可查看统计");
        check(StatisticsAuthenticationPolicy.requiresAuthentication(false, true),
            "已配置凭据时首次进入、取消、失败和重新进入都需认证");
        check(!StatisticsAuthenticationPolicy.requiresAuthentication(true, true),
            "本次认证成功后可以读取统计");
        long minute = 60_000L;
        check(!StatisticsAuthenticationPolicy.withinCooldown(0, 10 * minute), "从未认证过时没有冷却");
        check(StatisticsAuthenticationPolicy.withinCooldown(10 * minute, 10 * minute), "刚认证完再进入不用重新认证");
        check(StatisticsAuthenticationPolicy.withinCooldown(10 * minute, 15 * minute - 1), "五分钟之内不用重新认证");
        check(!StatisticsAuthenticationPolicy.withinCooldown(10 * minute, 15 * minute), "满五分钟要重新认证");
        check(!StatisticsAuthenticationPolicy.withinCooldown(10 * minute, 9 * minute), "时钟回到记录之前（例如重启设备）要重新认证");
        System.out.println("StatisticsAuthenticationPolicySmoke ok");
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}

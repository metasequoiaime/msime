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
        System.out.println("StatisticsAuthenticationPolicySmoke ok");
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}

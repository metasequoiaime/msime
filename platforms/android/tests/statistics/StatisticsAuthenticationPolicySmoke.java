package app.msime.android;

/** #9609：统计页离开后再次进入必须重新认证。 */
public final class StatisticsAuthenticationPolicySmoke {
    public static void main(String[] args) {
        if (!StatisticsAuthenticationPolicy.requiresAuthentication(false)
                || StatisticsAuthenticationPolicy.requiresAuthentication(true)) {
            throw new AssertionError("statistics authentication must follow page authorization state");
        }
        System.out.println("StatisticsAuthenticationPolicySmoke ok");
    }
}

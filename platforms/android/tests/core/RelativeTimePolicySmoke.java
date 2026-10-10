import app.msime.android.RelativeTimePolicy;

public final class RelativeTimePolicySmoke {
    public static void main(String[] arguments) {
        check("刚刚".equals(RelativeTimePolicy.minutesAgo(0)), "less than one minute");
        check("59 分钟前".equals(RelativeTimePolicy.minutesAgo(59)), "last minute before hour");
        check("1 小时前".equals(RelativeTimePolicy.minutesAgo(60)), "first hour");
        check("23 小时前".equals(RelativeTimePolicy.minutesAgo(1439)), "last hour before day");
        check("1 天前".equals(RelativeTimePolicy.minutesAgo(1440)), "first day");
        check("2 天前".equals(RelativeTimePolicy.minutesAgo(2880)), "multiple days");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

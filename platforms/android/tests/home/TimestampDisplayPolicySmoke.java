package app.msime.android;

/** Regression checks for RFC 3339 values outside java.time's local date range. */
public final class TimestampDisplayPolicySmoke {
    public static void main(String[] arguments) {
        check("".equals(TimestampDisplayPolicy.formatLocal("-1000000000-01-01T00:00:00Z")),
            "extreme backup timestamps are blank");
        // 取年中而不是 1 月 1 日零点：西半球时区里后者换成本地时间是 999999999 年 12 月 31 日，
        // 仍在 LocalDateTime 范围内，结果会随运行机器的时区变化；年中在 UTC±18 小时内都越界。
        check(TimestampDisplayPolicy.parse("+1000000000-06-01T00:00:00Z") == null,
            "extreme developer timestamps are ignored");
        System.out.println("Android timestamp display policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

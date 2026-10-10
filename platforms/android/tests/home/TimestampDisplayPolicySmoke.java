package app.msime.android;

/** Regression checks for RFC 3339 values outside java.time's local date range. */
public final class TimestampDisplayPolicySmoke {
    public static void main(String[] arguments) {
        check("".equals(TimestampDisplayPolicy.formatLocal("-1000000000-01-01T00:00:00Z")),
            "extreme backup timestamps are blank");
        check(TimestampDisplayPolicy.parse("+1000000000-01-01T00:00:00Z") == null,
            "extreme developer timestamps are ignored");
        System.out.println("Android timestamp display policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

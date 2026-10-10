package app.msime.android;

/** Regression checks for hostile but syntactically valid cloud timestamps. */
public final class CloudClipboardPageTimeSmoke {
    public static void main(String[] arguments) {
        check("".equals(CloudClipboardTimePolicy.relative("not-a-timestamp")),
            "malformed timestamps are blank");
        check("".equals(CloudClipboardTimePolicy.relative("-1000000000-01-01T00:00:00Z")),
            "timestamps outside the local date range are blank");
        System.out.println("Android cloud clipboard timestamp passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

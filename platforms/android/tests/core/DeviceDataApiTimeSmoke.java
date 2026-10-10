package app.msime.android;

/** Regression checks for hostile but syntactically valid account timestamps. */
public final class DeviceDataApiTimeSmoke {
    public static void main(String[] arguments) {
        check(DeviceDataApi.instant("+1000000000-01-01T00:00:00Z") == 0L,
            "positive epoch-millisecond overflow is ignored");
        check(DeviceDataApi.instant("-1000000000-01-01T00:00:00Z") == 0L,
            "negative epoch-millisecond overflow is ignored");
        check(DeviceDataApi.instant("2026-10-05T00:00:00Z") > 0L,
            "normal timestamp remains supported");
        System.out.println("Android device data timestamp passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

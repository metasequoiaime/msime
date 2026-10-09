import app.msime.android.JsonPolicy;

public final class NoticeBannerSmoke {
    public static void main(String[] arguments) throws Exception {
        check("synthetic".equals(JsonPolicy.strictString("synthetic")),
            "notice fields accept JSON strings");
        check(JsonPolicy.strictString(Integer.valueOf(7)) == null,
            "notice fields reject numbers instead of coercing them");
        check(JsonPolicy.strictString(Boolean.TRUE) == null,
            "notice fields reject booleans instead of coercing them");
        check(JsonPolicy.strictString(null) == null,
            "notice fields reject missing values");
        System.out.println("Android notice field policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

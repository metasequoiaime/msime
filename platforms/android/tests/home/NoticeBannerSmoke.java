public final class NoticeBannerSmoke {
    public static void main(String[] arguments) throws Exception {
        Class<?> policy = Class.forName("app.msime.android.core.NoticeFieldPolicy");
        java.lang.reflect.Method strictString = policy.getDeclaredMethod("strictString", Object.class);
        check("synthetic".equals(strictString.invoke(null, "synthetic")),
            "notice fields accept JSON strings");
        check(strictString.invoke(null, Integer.valueOf(7)) == null,
            "notice fields reject numbers instead of coercing them");
        check(strictString.invoke(null, Boolean.TRUE) == null,
            "notice fields reject booleans instead of coercing them");
        check(strictString.invoke(null, (Object) null) == null,
            "notice fields reject missing values");
        System.out.println("Android notice field policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

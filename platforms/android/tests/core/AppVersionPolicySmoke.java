import app.msime.android.AppVersionPolicy;

public final class AppVersionPolicySmoke {
    public static void main(String[] arguments) {
        check("0.4.2（2026-10-19 发布）".equals(AppVersionPolicy.displayVersion("0.4.2", "2026-10-19")), "version with release date");
        check("0.4.2".equals(AppVersionPolicy.displayVersion("0.4.2", null)), "version without release date");
        check("0.4.2".equals(AppVersionPolicy.displayVersion("0.4.2", "")), "empty release date");
        // JVM 冒烟没有 Gradle 生成的 BuildConfig，读不到日期时不报错。
        check(AppVersionPolicy.releaseDate() == null, "release date without BuildConfig");
        check(AppVersionPolicy.buildTime() == null, "build time without BuildConfig");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

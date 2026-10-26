import app.msime.android.WindowInsetsPolicy;

public final class WindowInsetsPolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    public static void main(String[] args) {
        check(WindowInsetsPolicy.bottomContentInset(12, 80, 0, 24) == 116);
        check(WindowInsetsPolicy.bottomContentInset(12, 80, 300, 24) == 324);
        check(WindowInsetsPolicy.bottomContentInset(12, 0, 0, 0) == 12);
        check(WindowInsetsPolicy.bottomContentInsetAboveIme(12, 80, 0, 24) == 116);
        check(WindowInsetsPolicy.bottomContentInsetAboveIme(12, 80, 300, 24) == 404);
        check(!WindowInsetsPolicy.shouldShowHomeTabs(true, false));
        check(!WindowInsetsPolicy.shouldShowHomeTabs(true, true));
        check(WindowInsetsPolicy.shouldShowHomeTabs(false, false));
        check(!WindowInsetsPolicy.shouldShowHomeTabs(false, true));
        System.out.println("Android 窗口底部内容避让策略通过");
    }
}

import app.msime.android.ScrollTopButtonPolicy;

public final class ScrollTopButtonPolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    public static void main(String[] args) {
        check(!ScrollTopButtonPolicy.visible(0));
        check(!ScrollTopButtonPolicy.visible(-1));
        check(ScrollTopButtonPolicy.visible(1));
        check(ScrollTopButtonPolicy.visible(240));
        System.out.println("Android 回到顶部按钮显示策略通过");
    }
}

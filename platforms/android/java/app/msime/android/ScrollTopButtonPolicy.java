package app.msime.android;

/** 决定列表滚动后是否显示回到顶部按钮。 */
public final class ScrollTopButtonPolicy {
    private ScrollTopButtonPolicy() {}

    public static boolean visible(int scrollY) {
        return scrollY > 0;
    }
}

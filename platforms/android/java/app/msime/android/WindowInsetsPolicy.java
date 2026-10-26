package app.msime.android;

/** Android 窗口遮挡区域共用的内容避让策略。 */
public final class WindowInsetsPolicy {
    private WindowInsetsPolicy() {}

    /** 计算页面避让系统栏或输入法后的底部内容内边距。 */
    public static int bottomContentInset(int systemBottom, int tabs, int imeBottom, int base) {
        return BoundsPolicy.atLeast(systemBottom + tabs, imeBottom) + base;
    }

    /** 计算输入法显示时仍需避开的底部导航和系统栏高度。 */
    public static int bottomContentInsetAboveIme(int systemBottom, int tabs, int imeBottom,
                                                 int base) {
        return BoundsPolicy.atLeast(systemBottom, imeBottom) + tabs + base;
    }

    /** 输入法弹出时收起主页底部导航；对话页无论是否弹出输入法都不显示。 */
    public static boolean shouldShowHomeTabs(boolean imeVisible, boolean chatVisible) {
        return !imeVisible && !chatVisible;
    }
}

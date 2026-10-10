package app.msime.android;

/** Android 窗口遮挡区域共用的内容避让策略。 */
public final class WindowInsetsPolicy {
    private WindowInsetsPolicy() {}

    /** 计算页面避让系统栏或输入法后的底部内容内边距。 */
    public static int bottomContentInset(int systemBottom, int tabs, int imeBottom, int base) {
        return BoundsPolicy.atLeast(systemBottom + tabs, imeBottom) + base;
    }
}

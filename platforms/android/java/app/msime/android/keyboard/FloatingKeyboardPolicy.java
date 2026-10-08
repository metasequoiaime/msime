package app.msime.android;

/**
 * 浮动键盘（#5621）的几何：键盘缩窄成一块可拖动的面板悬在应用上面，输入法窗口铺满屏幕但只有面板接收触摸，应用不再被顶起或压缩。
 *
 * <p>位置按「可移动范围里的千分比」存（{@link #MAX_FRACTION} 表示最右 / 最下），旋转、分屏或键盘变高后换算成像素时自动落回窗口里，不会跑到屏幕外。这里只做换算，不依赖 Android 运行时，供 JVM 冒烟直接调用。
 */
public final class FloatingKeyboardPolicy {
    /** 浮动键盘占当前窗口宽度的百分比。 */
    public static final int WIDTH_PERCENT = 80;
    /** 浮动键盘的最大宽度（dp）：平板上不必铺到 80%，与手机上的手感接近即可。 */
    public static final int MAX_WIDTH_DP = 480;
    /** 浮动键盘的最小宽度（dp）：再窄 26 键就点不准了；窗口本身更窄时取窗口宽度。 */
    public static final int MIN_WIDTH_DP = 240;
    /** 面板顶部拖动条的高度（dp）。 */
    public static final int BAR_HEIGHT_DP = 22;
    /** 浮动时面板的圆角（dp）。 */
    public static final int CORNER_RADIUS_DP = 12;
    public static final int MAX_FRACTION = 1000;
    /** 第一次浮动时的位置：水平居中、贴着窗口底部，和原来停靠的键盘在同一处，只是变窄。 */
    public static final int DEFAULT_X_FRACTION = 500;
    public static final int DEFAULT_Y_FRACTION = MAX_FRACTION;

    private FloatingKeyboardPolicy() { }

    /** 浮动键盘此刻是否生效：开关打开，且没有处在外接键盘的候选条模式（{@link HardwareKeyboardModePolicy}）——候选条停在底部。 */
    public static boolean active(boolean enabled, boolean hardwareKeyboardMode) {
        return enabled && !hardwareKeyboardMode;
    }

    /** 当前窗口宽度（dp）下浮动键盘的宽度（dp）：窗口的 80%，夹在 240–480 之间，且不超过窗口本身；窗口宽度未知时取 {@link #MIN_WIDTH_DP}。 */
    public static int widthDp(int windowWidthDp) {
        if (windowWidthDp <= 0) return MIN_WIDTH_DP;
        int preferred = (int) ((long) windowWidthDp * WIDTH_PERCENT / 100);
        int bounded = Math.max(MIN_WIDTH_DP, Math.min(MAX_WIDTH_DP, preferred));
        return Math.min(bounded, windowWidthDp);
    }

    /** 千分比钳到 0–1000。 */
    public static int fraction(int value) {
        return Math.max(0, Math.min(MAX_FRACTION, value));
    }

    /**
     * 千分比对应的偏移：{@code free} 是可移动的像素范围（可用区域减去面板大小），不是正数时面板放不下，贴在起点（偏移 0）。
     */
    public static int offset(int fraction, int free) {
        if (free <= 0) return 0;
        return (int) Math.round((double) fraction(fraction) * free / MAX_FRACTION);
    }

    /** 偏移对应的千分比，{@link #offset} 的反函数；范围外先钳回来，{@code free} 不是正数时为居中的 500。 */
    public static int fractionOf(float offset, int free) {
        if (free <= 0 || !Float.isFinite(offset)) return MAX_FRACTION / 2;
        return fraction((int) Math.round((double) offset * MAX_FRACTION / free));
    }

    /** 拖动时的新偏移：起点加位移后钳进 0…{@code free}；位移不是有限数时停在起点。 */
    public static float dragged(float start, float delta, int free) {
        float limit = Math.max(0, free);
        float next = Float.isFinite(delta) ? start + delta : start;
        return Math.max(0f, Math.min(limit, next));
    }
}

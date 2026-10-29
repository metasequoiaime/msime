package app.msime.android;

/** Android window-size policy for the native touch keyboard surface. */
public final class KeyboardFormFactorPolicy {
    /** Android's conventional boundary between handset and large-screen layouts. */
    public static final int EXPANDED_SMALLEST_WIDTH_DP = 600;
    /** Keep a convertible's key travel comfortable instead of stretching ten keys edge to edge. */
    public static final int EXPANDED_SURFACE_MAX_WIDTH_DP = 720;
    /** 大屏单手模式的键区最宽多少：与手机上单手时的键区差不多大，拇指够得着（#10202）。 */
    public static final int ONE_HANDED_KEYS_MAX_WIDTH_DP = 360;

    private KeyboardFormFactorPolicy() {}

    public static boolean expanded(int smallestWidthDp) {
        return smallestWidthDp >= EXPANDED_SMALLEST_WIDTH_DP;
    }

    /** 手机不提供只对大屏有意义的分离式键盘和物理键盘候选栏设置。 */
    public static boolean showLargeScreenOptions(int smallestWidthDp) {
        return expanded(smallestWidthDp);
    }

    /**
     * Width of the keyboard surface, in dp. A zero result means fill the handset window.
     *
     * <p>The decision uses {@code smallestScreenWidthDp}, not the current width: rotating a phone
     * must not turn it into the convertible layout just because its landscape width crosses 600dp.
     */
    public static int surfaceWidthDp(int smallestWidthDp, int screenWidthDp) {
        return surfaceWidthDp(smallestWidthDp, screenWidthDp, false);
    }

    /**
     * 同上，另外考虑分离式键盘：分离式键盘画着的时候（{@link SplitKeyboardPolicy#drawn}）不受 720 dp 上限约束，整套键盘表面铺满可用宽度，左右两半才能贴到两侧，留给双手拇指。结果为 0 表示铺满。
     */
    public static int surfaceWidthDp(int smallestWidthDp, int screenWidthDp, boolean splitKeyboard) {
        return surfaceWidthDp(smallestWidthDp, screenWidthDp, splitKeyboard, false);
    }

    /**
     * 同上，另外考虑单手模式：大屏上单手模式生效时（分离式、浮动时它不生效，由调用方先排除）键盘表面也铺满窗口，侧栏与键区才能贴到屏幕边上，见 {@link #oneHandedBlockWidthDp}。
     */
    public static int surfaceWidthDp(int smallestWidthDp, int screenWidthDp, boolean splitKeyboard, boolean oneHanded) {
        if (!expanded(smallestWidthDp) || splitKeyboard || oneHanded) return 0;
        if (screenWidthDp <= 0) return EXPANDED_SURFACE_MAX_WIDTH_DP;
        return BoundsPolicy.atMost(screenWidthDp, EXPANDED_SURFACE_MAX_WIDTH_DP);
    }

    /**
     * 单手模式下「侧栏 + 键区」这一块有多宽，单位 dp。结果为 0 表示手机：照旧铺满整行，按 {@link OneHandGutterView#GUTTER_FRACTION} 切出侧栏。
     *
     * <p>大屏上沿用手机的比例会让键区有六七百 dp 宽，单手够不着（#10202）。所以大屏上这一块按键区 {@link #ONE_HANDED_KEYS_MAX_WIDTH_DP} 加同样比例的侧栏算，再贴到所选的那一侧，另一侧空着；窗口窄到放不下时整行都给它。工具栏与候选条不在这一块里，保持全宽。
     */
    public static int oneHandedBlockWidthDp(int smallestWidthDp, int screenWidthDp) {
        if (!expanded(smallestWidthDp)) return 0;
        int block = Math.round(ONE_HANDED_KEYS_MAX_WIDTH_DP / (1f - OneHandGutterView.GUTTER_FRACTION));
        return screenWidthDp <= 0 ? block : BoundsPolicy.atMost(screenWidthDp, block);
    }
}

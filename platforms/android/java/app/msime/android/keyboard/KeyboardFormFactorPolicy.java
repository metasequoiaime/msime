package app.msime.android;

/** Android window-size policy for the native touch keyboard surface. */
public final class KeyboardFormFactorPolicy {
    /** Android's conventional boundary between handset and large-screen layouts. */
    public static final int EXPANDED_SMALLEST_WIDTH_DP = 600;
    /** Keep a convertible's key travel comfortable instead of stretching ten keys edge to edge. */
    public static final int EXPANDED_SURFACE_MAX_WIDTH_DP = 720;
    /** 大屏单手模式的键区最宽多少：与手机上单手时的键区差不多大，拇指够得着（#10202）。 */
    public static final int ONE_HANDED_KEYS_MAX_WIDTH_DP = 360;
    /** 大屏单手模式空出来那一侧的侧边数字键盘最宽多少（#10203）：三列键每列约 100 dp，和九键数字层的键差不多大；空出来的再宽也只取这么多，多出来的是空白。 */
    public static final int ONE_HANDED_SIDE_PAD_MAX_WIDTH_DP = 300;
    /** 空出来的宽度小于它时不画侧边数字键盘：三列键每列至少约 44 dp 才按得准。 */
    public static final int ONE_HANDED_SIDE_PAD_MIN_WIDTH_DP = 150;

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

    /**
     * 大屏单手模式空出来那一侧的侧边数字键盘有多宽，单位 dp（#10203）。结果为 0 表示不画：手机（{@link #oneHandedBlockWidthDp} 为 0）、窗口宽度未知、或空出来的宽度不到 {@link #ONE_HANDED_SIDE_PAD_MIN_WIDTH_DP}。否则取空出来的宽度，最多 {@link #ONE_HANDED_SIDE_PAD_MAX_WIDTH_DP}；面板贴屏幕边，它和侧栏之间剩下的是空白。
     */
    public static int oneHandedSidePadWidthDp(int smallestWidthDp, int screenWidthDp) {
        int block = oneHandedBlockWidthDp(smallestWidthDp, screenWidthDp);
        if (block <= 0 || screenWidthDp <= 0) return 0;
        int side = screenWidthDp - block;
        if (side < ONE_HANDED_SIDE_PAD_MIN_WIDTH_DP) return 0;
        return BoundsPolicy.atMost(side, ONE_HANDED_SIDE_PAD_MAX_WIDTH_DP);
    }
}

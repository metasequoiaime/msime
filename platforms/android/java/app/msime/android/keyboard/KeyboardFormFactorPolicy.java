package app.msime.android;

/** Android window-size policy for the native touch keyboard surface. */
public final class KeyboardFormFactorPolicy {
    /** Android's conventional boundary between handset and large-screen layouts. */
    public static final int EXPANDED_SMALLEST_WIDTH_DP = 600;
    /** Keep a convertible's key travel comfortable instead of stretching ten keys edge to edge. */
    public static final int EXPANDED_SURFACE_MAX_WIDTH_DP = 720;

    private KeyboardFormFactorPolicy() {}

    public static boolean expanded(int smallestWidthDp) {
        return smallestWidthDp >= EXPANDED_SMALLEST_WIDTH_DP;
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
        if (!expanded(smallestWidthDp) || splitKeyboard) return 0;
        if (screenWidthDp <= 0) return EXPANDED_SURFACE_MAX_WIDTH_DP;
        return BoundsPolicy.atMost(screenWidthDp, EXPANDED_SURFACE_MAX_WIDTH_DP);
    }
}

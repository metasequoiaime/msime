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
        if (!expanded(smallestWidthDp)) return 0;
        if (screenWidthDp <= 0) return EXPANDED_SURFACE_MAX_WIDTH_DP;
        return Math.min(screenWidthDp, EXPANDED_SURFACE_MAX_WIDTH_DP);
    }
}

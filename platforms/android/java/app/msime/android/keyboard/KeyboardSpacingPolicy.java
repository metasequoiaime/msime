package app.msime.android;

/** Shared touch-key spacing and gap conversions for Android keyboard layouts. */
public final class KeyboardSpacingPolicy {
    private KeyboardSpacingPolicy() { }

    /** 默认键距 6 dp、行距 7 dp，与共享偏好 client-core 的默认值一致。 */
    public static final int DEFAULT_KEY_SPACING_TENTHS = 60;
    public static final int DEFAULT_ROW_SPACING_TENTHS = 70;
    public static final int MIN_KEY_SPACING_TENTHS = 30;
    public static final int MAX_KEY_SPACING_TENTHS = 60;
    public static final int MIN_ROW_SPACING_TENTHS = 40;
    public static final int MAX_ROW_SPACING_TENTHS = 100;

    public static int keySpacing(int value) {
        return clamp(value, MIN_KEY_SPACING_TENTHS, MAX_KEY_SPACING_TENTHS,
            DEFAULT_KEY_SPACING_TENTHS);
    }

    /** Widest key spacing the Dachen keyboard takes. */
    public static final int ZHUYIN_MAX_KEY_SPACING_TENTHS = 40;

    /** The displayed layout caps Dachen spacing while preserving narrower user settings. */
    public static int layoutKeySpacing(int value, int touchLayout) {
        int spacing = keySpacing(value);
        return touchLayout == KeyboardLayout.ZHUYIN_LAYOUT
            ? BoundsPolicy.atMost(spacing, ZHUYIN_MAX_KEY_SPACING_TENTHS) : spacing;
    }

    public static int rowSpacing(int value) {
        return clamp(value, MIN_ROW_SPACING_TENTHS, MAX_ROW_SPACING_TENTHS,
            DEFAULT_ROW_SPACING_TENTHS);
    }

    public static String display(int tenths) {
        return NumberPolicy.decimal1(tenths / 10.0);
    }

    public static int halfGapPixels(int tenths, float density) {
        if (!Float.isFinite(density) || density <= 0) return 0;
        return BoundsPolicy.nonNegative(Math.round(tenths * density / 20f));
    }

    private static int clamp(int value, int minimum, int maximum, int fallback) {
        if (value < 0) return fallback;
        return BoundsPolicy.bounded(value, minimum, maximum);
    }
}

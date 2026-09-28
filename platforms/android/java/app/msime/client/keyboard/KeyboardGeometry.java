package app.msime.client;

import java.util.Locale;

/** Apple-compatible touch-keyboard spacing contract; input algorithms remain in Engine. */
public final class KeyboardGeometry {
    public static final int DEFAULT_HEIGHT_ADJUSTMENT_DP = 0;
    public static final int MIN_HEIGHT_ADJUSTMENT_DP = -12;
    public static final int MAX_HEIGHT_ADJUSTMENT_DP = 48;
    public static final int STANDARD_ROW_HEIGHT_DP = 48;
    /** Fixed candidate/shortcut row; swapping its contents must not move the key rows. */
    public static final int CANDIDATE_ROW_HEIGHT_DP = 48;
    public static final int NINE_KEY_HEIGHT_DP = 180;
    public static final int HANDWRITING_BODY_HEIGHT_DP = 220;
    public static final int DEFAULT_KEY_SPACING_TENTHS = 60;
    public static final int DEFAULT_ROW_SPACING_TENTHS = 70;
    public static final int MIN_KEY_SPACING_TENTHS = 30;
    public static final int MAX_KEY_SPACING_TENTHS = 60;
    public static final int MIN_ROW_SPACING_TENTHS = 40;
    public static final int MAX_ROW_SPACING_TENTHS = 100;

    private KeyboardGeometry() { }

    public static int keySpacing(int value) {
        return clamp(value, MIN_KEY_SPACING_TENTHS, MAX_KEY_SPACING_TENTHS,
            DEFAULT_KEY_SPACING_TENTHS);
    }

    public static int rowSpacing(int value) {
        return clamp(value, MIN_ROW_SPACING_TENTHS, MAX_ROW_SPACING_TENTHS,
            DEFAULT_ROW_SPACING_TENTHS);
    }

    public static int heightAdjustment(int value) {
        if (value == Integer.MIN_VALUE) return DEFAULT_HEIGHT_ADJUSTMENT_DP;
        return Math.max(MIN_HEIGHT_ADJUSTMENT_DP, Math.min(value, MAX_HEIGHT_ADJUSTMENT_DP));
    }

    /** Divide the total adjustment across rows without losing a density-independent pixel. */
    public static int adjustedRowHeight(int baseHeight, int adjustment, int rowCount, int rowIndex) {
        if (baseHeight <= 0 || rowCount <= 0 || rowIndex < 0 || rowIndex >= rowCount)
            throw new IllegalArgumentException("Invalid keyboard height geometry");
        int total = baseHeight * rowCount + heightAdjustment(adjustment);
        return total / rowCount + (rowIndex < total % rowCount ? 1 : 0);
    }

    public static String display(int tenths) {
        return String.format(Locale.ROOT, "%.1f", tenths / 10.0);
    }

    public static String displayHeight(int adjustment) {
        int value = heightAdjustment(adjustment);
        return (value > 0 ? "+" : "") + value;
    }

    public static int halfGapPixels(int tenths, float density) {
        if (!Float.isFinite(density) || density <= 0) return 0;
        return Math.max(0, Math.round(tenths * density / 20f));
    }

    public static int bounded(int value, int minimum, int maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    private static int clamp(int value, int minimum, int maximum, int fallback) {
        if (value < 0) return fallback;
        return bounded(value, minimum, maximum);
    }
}

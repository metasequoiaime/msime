package app.msime.android;

import java.util.Locale;
import org.json.JSONObject;

/** Apple-compatible touch-keyboard spacing contract; input algorithms remain in Engine. */
public final class KeyboardGeometry {
    public static final int DEFAULT_HEIGHT_ADJUSTMENT_DP = 0;
    public static final int MIN_HEIGHT_ADJUSTMENT_DP = -12;
    public static final int MAX_HEIGHT_ADJUSTMENT_DP = 48;
    public static final int STANDARD_ROW_HEIGHT_DP = 46;
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
        return bounded(value, MIN_HEIGHT_ADJUSTMENT_DP, MAX_HEIGHT_ADJUSTMENT_DP);
    }

    /** 读取整数值，拒绝 JSONObject 的小数截断、布尔转换和非有限数。 */
    public static int strictInt(JSONObject object, String key, int fallback) {
        return strictInt(object == null ? null : object.opt(key), fallback);
    }

    public static int strictInt(Object raw, int fallback) {
        if (!(raw instanceof Number) || raw instanceof Boolean) return fallback;
        double value = ((Number) raw).doubleValue();
        if (!Double.isFinite(value) || value != Math.rint(value)
                || value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) return fallback;
        return (int) value;
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

    public static double bounded(double value, double minimum, double maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    public static double bounded(double value, double minimum, double maximum, double fallback) {
        return Double.isFinite(value) ? bounded(value, minimum, maximum) : fallback;
    }

    public static float bounded(float value, float minimum, float maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    private static int clamp(int value, int minimum, int maximum, int fallback) {
        if (value < 0) return fallback;
        return bounded(value, minimum, maximum);
    }
}

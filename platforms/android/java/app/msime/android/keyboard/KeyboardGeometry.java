package app.msime.android;

import java.util.Locale;
import java.math.BigDecimal;
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
    /** 新设计的默认键距 5 dp、行距 8 dp（plan P26：Android 新安装的默认值）。 */
    public static final int DEFAULT_KEY_SPACING_TENTHS = 50;
    public static final int DEFAULT_ROW_SPACING_TENTHS = 80;
    public static final int MIN_KEY_SPACING_TENTHS = 30;
    public static final int MAX_KEY_SPACING_TENTHS = 60;
    public static final int MIN_ROW_SPACING_TENTHS = 40;
    public static final int MAX_ROW_SPACING_TENTHS = 100;

    // ---- 新设计的键盘几何（N/design-tokens.md §5） ----

    /** 新设计的标准键高（100% 时）。 */
    public static final int DESIGN_KEY_HEIGHT_DP = 46;
    /** 新设计的工具栏 / 候选行高度。 */
    public static final int DESIGN_TOOLBAR_ROW_HEIGHT_DP = 50;
    /** 新设计键盘外边距：上 8、左右 6、下 6。 */
    public static final int DESIGN_PADDING_TOP_DP = 8;
    public static final int DESIGN_PADDING_HORIZONTAL_DP = 6;
    public static final int DESIGN_PADDING_BOTTOM_DP = 6;
    /** 新设计的键距 5 dp、行距 8 dp。 */
    public static final int DESIGN_KEY_GAP_DP = 5;
    public static final int DESIGN_ROW_GAP_DP = 8;
    /** 键盘高度百分比的范围与默认值（内联高度条 75%–130%）。 */
    public static final int MIN_HEIGHT_PERCENT = 75;
    public static final int MAX_HEIGHT_PERCENT = 130;
    public static final int DEFAULT_HEIGHT_PERCENT = 100;
    /** 百分比换算的基准：四行标准键高 4 × 46 dp，与 Rust `TOUCH_KEYBOARD_HEIGHT_BASE_DP` 相同。 */
    public static final int HEIGHT_PERCENT_BASE_DP = 184;
    /** 触屏键盘高度调整在新设计下的范围（dp）：75%–130% 换算为 −46…55，与 Rust `TOUCH_KEYBOARD_HEIGHT_ADJUSTMENT_RANGE` 相同。 */
    public static final int MIN_DESIGN_HEIGHT_ADJUSTMENT_DP = -46;
    public static final int MAX_DESIGN_HEIGHT_ADJUSTMENT_DP = 55;

    private KeyboardGeometry() { }

    /** 键盘高度百分比对应的高度调整 dp：`round(184 × (p − 100) / 100)`，范围外先钳到 75–130，与 Rust `height_percent_to_adjustment` 同式（向远离零的方向取整）。 */
    public static int heightPercentToAdjustment(int percent) {
        int clamped = bounded(percent, MIN_HEIGHT_PERCENT, MAX_HEIGHT_PERCENT);
        int scaled = HEIGHT_PERCENT_BASE_DP * (clamped - DEFAULT_HEIGHT_PERCENT);
        return (scaled + Integer.signum(scaled) * 50) / 100;
    }

    /** 高度调整 dp 对应的百分比：`round(100 + adjustment × 100 / 184)`，与 Rust `height_adjustment_to_percent` 同式；调整先钳到 −46…55。 */
    public static int heightAdjustmentToPercent(int adjustment) {
        int clamped = designHeightAdjustment(adjustment);
        int scaled = clamped * 100;
        return DEFAULT_HEIGHT_PERCENT
            + (scaled + Integer.signum(scaled) * HEIGHT_PERCENT_BASE_DP / 2) / HEIGHT_PERCENT_BASE_DP;
    }

    /** 新设计下的高度调整：缺省（{@link Integer#MIN_VALUE}）为 0，其余钳到 −46…55。 */
    public static int designHeightAdjustment(int value) {
        if (value == Integer.MIN_VALUE) return DEFAULT_HEIGHT_ADJUSTMENT_DP;
        return bounded(value, MIN_DESIGN_HEIGHT_ADJUSTMENT_DP, MAX_DESIGN_HEIGHT_ADJUSTMENT_DP);
    }

    /** 某个高度百分比下的键高：`round(46 × p / 100)`。 */
    public static int designKeyHeight(int percent) {
        int clamped = bounded(percent, MIN_HEIGHT_PERCENT, MAX_HEIGHT_PERCENT);
        return (DESIGN_KEY_HEIGHT_DP * clamped + 50) / 100;
    }

    /** 内联高度条上显示的百分比文字，如「100%」。 */
    public static String displayPercent(int percent) {
        return bounded(percent, MIN_HEIGHT_PERCENT, MAX_HEIGHT_PERCENT) + "%";
    }

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
        if (raw instanceof Double || raw instanceof Float) return fallback;
        try {
            return new BigDecimal(raw.toString()).intValueExact();
        } catch (NumberFormatException | ArithmeticException error) {
            return fallback;
        }
    }

    public static long strictLong(Object raw, long fallback) {
        if (!(raw instanceof Number) || raw instanceof Boolean) return fallback;
        if (raw instanceof Double || raw instanceof Float) return fallback;
        try {
            return new BigDecimal(raw.toString()).longValueExact();
        } catch (NumberFormatException | ArithmeticException error) {
            return fallback;
        }
    }

    /** Read a finite JSON number without accepting numeric strings or booleans. */
    public static double strictDouble(Object raw, double fallback) {
        if (!(raw instanceof Number) || raw instanceof Boolean) return fallback;
        double value = ((Number) raw).doubleValue();
        return Double.isFinite(value) ? value : fallback;
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

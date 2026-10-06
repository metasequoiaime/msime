package app.msime.android;

import android.content.Context;
import android.content.res.Configuration;
import android.util.TypedValue;
import java.util.Locale;
import java.math.BigDecimal;
import org.json.JSONObject;

/** Apple-compatible touch-keyboard spacing contract; input algorithms remain in Engine. */
public final class KeyboardGeometry {
    public static final int DEFAULT_HEIGHT_ADJUSTMENT_DP = 0;
    public static final int MIN_HEIGHT_ADJUSTMENT_DP = -12;
    public static final int MAX_HEIGHT_ADJUSTMENT_DP = 48;
    /** 底栏（123、中/英、空格、换行那一行）的高度，也是高度百分比换算的那个 46 dp 设计键高。 */
    public static final int STANDARD_ROW_HEIGHT_DP = 46;
    /** 所有布局的键行高（26 键字母行、九键网格、笔画、手写区、大千四行整块……）：按 46 dp 排九宫格的键像横条，所有布局一起加到 56 dp，彼此切换时键盘总高不变。底栏仍是 {@link #STANDARD_ROW_HEIGHT_DP}。 */
    public static final int KEY_ROW_HEIGHT_DP = 56;
    /** Fixed candidate/shortcut row; swapping its contents must not move the key rows. */
    public static final int CANDIDATE_ROW_HEIGHT_DP = 48;
    public static final int NINE_KEY_HEIGHT_DP = 180;
    public static final int HANDWRITING_BODY_HEIGHT_DP = 220;
    /** 默认键距 6 dp、行距 7 dp，与共享偏好 client-core 的 `touch_key_spacing_tenths` / `touch_row_spacing_tenths` 默认值一致：共享层总会把这两个值写进偏好，这里另取一套会让新安装和「恢复默认」先画一种间距再跳回共享的那种。要改默认值得在共享层改，各平台一起变。 */
    public static final int DEFAULT_KEY_SPACING_TENTHS = 60;
    public static final int DEFAULT_ROW_SPACING_TENTHS = 70;
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

    /** Return the current display width in physical pixels. */
    public static int screenWidthPixels(Context context) {
        return context.getResources().getDisplayMetrics().widthPixels;
    }

    /** Return whether the supplied context currently uses the system night configuration. */
    public static boolean isNight(Context context) {
        return (context.getResources().getConfiguration().uiMode
            & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
    }

    /** Read the display density used by keyboard geometry calculations. */
    public static float density(Context context) {
        return context.getResources().getDisplayMetrics().density;
    }

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

    /** Widest key spacing the Dachen keyboard takes: eleven columns leave each key a tenth less width than the 26-key rows, and a 6 dp gap would take that out of the keycap itself. */
    public static final int ZHUYIN_MAX_KEY_SPACING_TENTHS = 40;

    /** The key spacing a touch layout draws with; the Dachen rows cap the setting rather than replace it, so a narrower gap the user picked still applies. */
    public static int layoutKeySpacing(int value, int touchLayout) {
        int spacing = keySpacing(value);
        return touchLayout == KeyboardLayout.ZHUYIN_LAYOUT
            ? Math.min(spacing, ZHUYIN_MAX_KEY_SPACING_TENTHS) : spacing;
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

    /** Convert a size to pixels while guaranteeing at least one physical pixel. */
    public static int atLeastOnePixel(Context context, float dp) {
        return Math.max(1, pixels(context, dp));
    }

    /** Convert an integer density-independent size to pixels using Android's rounding rule. */
    public static int pixels(int dp, float density) {
        return Math.round(dp * density);
    }

    /** Convert a fractional density-independent size to rounded pixels. */
    public static int pixels(float dp, float density) {
        return Math.round(dp * density);
    }

    /** Convert a density-independent size to rounded pixels using the context's density. */
    public static int pixels(Context context, float dp) {
        return pixels(dp, density(context));
    }

    /** Convert a fractional density-independent size to pixels without rounding. */
    public static float floatPixels(double dp, float density) {
        return (float) dp * density;
    }

    /** Convert a fractional density-independent size to pixels using the context's density. */
    public static float floatPixels(Context context, double dp) {
        return floatPixels(dp, density(context));
    }

    /** Convert pixels back to density-independent units using the context's density. */
    public static float fromPixels(Context context, float pixels) {
        float density = density(context);
        return density <= 0 ? pixels : pixels / density;
    }

    /** Convert scalable text units using the view context's display metrics. */
    public static float sp(Context context, float value) {
        return TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, value,
            context.getResources().getDisplayMetrics());
    }

    /** 键盘里的文字最多跟随系统字体放大到这个倍数。 */
    public static final float MAX_KEYBOARD_FONT_SCALE = 1.15f;

    /**
     * 键盘里文字实际用的字体缩放：系统设置调小时照样跟随，调大时封顶在 {@link #MAX_KEYBOARD_FONT_SCALE}。
     *
     * <p>键高、候选行高和工具栏都是固定 dp，而国产机出厂常把字体设成「大」甚至「超大」（1.3–2.0 倍）。不封顶时 22 sp 的字母在 56 dp 的键里放不下：文字超出内边距框时 TextView 不再居中，而是从上内边距处往下排，字母被挤到键底被裁掉，`123` 折成两行，「中」只剩顶上一截。系统键盘（Gboard、iOS）的键面同样不随系统字号无限放大。
     */
    public static float keyboardFontScale(float systemFontScale) {
        if (!(systemFontScale > 0) || Float.isInfinite(systemFontScale)) return 1f;
        return Math.min(systemFontScale, MAX_KEYBOARD_FONT_SCALE);
    }

    /** 键盘文字的 sp 换算成像素，字体缩放按 {@link #keyboardFontScale} 封顶。 */
    public static float keySp(Context context, float value) {
        android.content.res.Resources resources = context.getResources();
        return value * resources.getDisplayMetrics().density
            * keyboardFontScale(resources.getConfiguration().fontScale);
    }

    /** 以 {@link #keySp} 设置键盘里控件的字号。 */
    public static void setKeyTextSize(android.widget.TextView view, float sp) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_PX, keySp(view.getContext(), sp));
    }

    /** 键帽左右各留的内边距（dp）：只防字形贴住圆角，键宽几乎全部留给文字。 */
    public static final int KEY_CAP_HORIZONTAL_PADDING_DP = 2;

    /**
     * 给键帽定下与系统主题无关的内边距和最小尺寸。
     *
     * <p>键帽一直沿用按钮样式自带的内边距，那份内边距来自系统主题的按钮背景：原生 Material 是左右 12 dp、上下 10 dp，各厂商的 `DeviceDefault` 主题又各不相同。TextView 把文字裁在内边距框里，36 dp 宽的字母键扣掉两侧内边距后经常放不下一个字母，于是同一个键盘在不同手机上有的正常、有的字母整排消失。键帽的文字本来就由 gravity 居中，提示、数字和图标由各自的子类在需要时另加内边距，所以这里统一归零上下、左右只留 {@link #KEY_CAP_HORIZONTAL_PADDING_DP}，并去掉字体留白和最小宽高。
     */
    public static void normalizeKeyCap(android.widget.TextView key) {
        int horizontal = pixels(key.getContext(), KEY_CAP_HORIZONTAL_PADDING_DP);
        key.setPadding(horizontal, 0, horizontal, 0);
        key.setIncludeFontPadding(false);
        key.setMinWidth(0);
        key.setMinimumWidth(0);
        key.setMinHeight(0);
        key.setMinimumHeight(0);
    }

    public static int bounded(int value, int minimum, int maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    public static long bounded(long value, long minimum, long maximum) {
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

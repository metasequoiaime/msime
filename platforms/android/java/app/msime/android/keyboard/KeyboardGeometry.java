package app.msime.android;

import android.content.Context;
import android.content.res.Configuration;
import android.util.TypedValue;
import android.view.View;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.view.ViewGroup;
import java.math.BigDecimal;
import org.json.JSONObject;

/** Apple-compatible touch-keyboard spacing contract; input algorithms remain in Engine. */
public final class KeyboardGeometry {
    public static final int DEFAULT_HEIGHT_ADJUSTMENT_DP = 0;
    public static final int MIN_HEIGHT_ADJUSTMENT_DP = -12;
    public static final int MAX_HEIGHT_ADJUSTMENT_DP = 48;
    /** 所有布局的每一行键高，包括底行（123、中/英、空格、换行那一行）：26 键字母行、九键网格、笔画、手写区、大千四行整块和底行都按它排，每行另加一份行距，彼此切换时键盘总高不变。默认行距下四行共 4 × (52 + 7) = 236 dp，与原来三行 56 dp 键行加 46 dp 不带行距的底栏（235 dp）差 1 dp；底行和上面的键一样高，与 iOS、HarmonyOS 相同（#6354、#6472）。 */
    public static final int KEY_ROW_HEIGHT_DP = 52;
    /** 键区的行数：三行键（或一块占三行高的键块）加底行。键盘高度调整由这四行均分，见 {@link #bottomRowAdjustment}。 */
    public static final int KEYBOARD_ROW_COUNT = 4;

    /**
     * 底行分到的键盘高度调整（dp）：底行和上面三行一起均分调整量，余数按 {@link #adjustedRowHeight} 的规则给前面的行，底行是第四行，所以是 `floorDiv(调整量, 4)`。调整量先按 {@link #designHeightAdjustment} 钳制。
     *
     * <p>和 {@link #keyRowsAdjustment} 相加恰好是整份调整量，键盘总高与百分比的换算不变。
     */
    public static int bottomRowAdjustment(int adjustment) {
        return Math.floorDiv(designHeightAdjustment(adjustment), KEYBOARD_ROW_COUNT);
    }

    /** 底行以上的键行（三行字母、数字层的前三行、九键 / 笔画 / 手写这类三行高的键块）合起来分到的高度调整：整份调整量减去底行那一份（{@link #bottomRowAdjustment}）。 */
    public static int keyRowsAdjustment(int adjustment) {
        int value = designHeightAdjustment(adjustment);
        return value - bottomRowAdjustment(value);
    }

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
    /** 键盘高度百分比的范围与默认值（内联高度条与设置页滑块 75%–160%）。上限原是 130%，竖屏手机上整块键盘只到屏幕的四成多，单手握持时拇指够不舒服（#5564），放宽到 160%，竖屏手机能调到屏幕一半以上；超出 130% 的部分另受窗口高度约束，见 {@link #windowHeightAdjustment}。 */
    public static final int MIN_HEIGHT_PERCENT = 75;
    public static final int MAX_HEIGHT_PERCENT = 160;
    public static final int DEFAULT_HEIGHT_PERCENT = 100;
    /** 百分比换算的基准：四行设计键高 4 × 46 dp。这是换算用的常数，不是此刻画出的键区高度（键行是 {@link #KEY_ROW_HEIGHT_DP} 另加行距）；调整量按它换算，所以 1% 仍是 1.84 dp。 */
    public static final int HEIGHT_PERCENT_BASE_DP = 184;
    /** 触屏键盘高度调整在新设计下的范围（dp）：75%–160% 换算为 −46…110。存在 Android 本地设置里，共享偏好的 `touch_keyboard_height_adjustment` 仍是 −12…48。 */
    public static final int MIN_DESIGN_HEIGHT_ADJUSTMENT_DP = -46;
    public static final int MAX_DESIGN_HEIGHT_ADJUSTMENT_DP = 110;
    /** 放宽前的上限（130% 对应的 55 dp）：不超过它的调整量在任何窗口里都照画，与放宽前相同。 */
    public static final int LEGACY_MAX_DESIGN_HEIGHT_ADJUSTMENT_DP = 55;
    /** 超出 {@link #LEGACY_MAX_DESIGN_HEIGHT_ADJUSTMENT_DP} 的调整量最多占窗口高度的这个百分比：竖屏手机（约 800 dp 高）能用满 160%，横屏手机（约 400 dp 高）止于 130%，不会把工具栏顶出窗口。 */
    public static final int WINDOW_HEIGHT_ADJUSTMENT_PERCENT = 14;

    private KeyboardGeometry() { }

    /** Return the shorter of two dimensions for proportional control sizing. */
    public static float shorterSide(float width, float height) {
        return BoundsPolicy.atMost(width, height);
    }

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

    /** Return a view's non-negative width after horizontal padding. */
    public static int contentWidth(View view) {
        return BoundsPolicy.nonNegative(view.getWidth() - view.getPaddingLeft() - view.getPaddingRight());
    }

    /** Return a view's non-negative height after vertical padding. */
    public static int contentHeight(View view) {
        return BoundsPolicy.nonNegative(view.getHeight() - view.getPaddingTop() - view.getPaddingBottom());
    }

    /** 键盘高度百分比对应的高度调整 dp：`round(184 × (p − 100) / 100)`，范围外先钳到 75–160（向远离零的方向取整）。 */
    public static int heightPercentToAdjustment(int percent) {
        int clamped = bounded(percent, MIN_HEIGHT_PERCENT, MAX_HEIGHT_PERCENT);
        int scaled = HEIGHT_PERCENT_BASE_DP * (clamped - DEFAULT_HEIGHT_PERCENT);
        return (scaled + Integer.signum(scaled) * 50) / 100;
    }

    /** 高度调整 dp 对应的百分比：`round(100 + adjustment × 100 / 184)`；调整先钳到 −46…110。 */
    public static int heightAdjustmentToPercent(int adjustment) {
        int clamped = designHeightAdjustment(adjustment);
        int scaled = clamped * 100;
        return DEFAULT_HEIGHT_PERCENT
            + (scaled + Integer.signum(scaled) * HEIGHT_PERCENT_BASE_DP / 2) / HEIGHT_PERCENT_BASE_DP;
    }

    /** 新设计下的高度调整：缺省（{@link Integer#MIN_VALUE}）为 0，其余钳到 −46…110。 */
    public static int designHeightAdjustment(int value) {
        if (value == Integer.MIN_VALUE) return DEFAULT_HEIGHT_ADJUSTMENT_DP;
        return bounded(value, MIN_DESIGN_HEIGHT_ADJUSTMENT_DP, MAX_DESIGN_HEIGHT_ADJUSTMENT_DP);
    }

    /**
     * 当前窗口里实际画出的高度调整：不超过 55 dp（130%）的照画；更高的部分最多到窗口高度的 {@link #WINDOW_HEIGHT_ADJUSTMENT_PERCENT}%，但不低于 55 dp。
     *
     * <p>存着的设置不变，旋转回竖屏或换到更高的窗口时自动恢复；窗口高度未知（0 或负数）时只画到 55 dp。
     *
     * @param adjustment 设置里的调整量（dp）
     * @param windowHeightDp 键盘所在窗口的可用高度（`Configuration.screenHeightDp`）
     */
    public static int windowHeightAdjustment(int adjustment, int windowHeightDp) {
        int value = designHeightAdjustment(adjustment);
        if (value <= LEGACY_MAX_DESIGN_HEIGHT_ADJUSTMENT_DP) return value;
        int windowLimit = windowHeightDp <= 0 ? 0
            : (int) ((long) windowHeightDp * WINDOW_HEIGHT_ADJUSTMENT_PERCENT / 100);
        return Math.min(value, Math.max(LEGACY_MAX_DESIGN_HEIGHT_ADJUSTMENT_DP, windowLimit));
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

    /**
     * Divide the total adjustment across rows without losing a density-independent pixel.
     *
     * <p>调整量按新设计的范围（−46…110）钳制。这里曾用共享偏好的 −12…48 钳制，设置里 126%–130% 与 75%–93% 画出来都一样高，滑块的两端是死区。
     */
    public static int adjustedRowHeight(int baseHeight, int adjustment, int rowCount, int rowIndex) {
        if (baseHeight <= 0 || rowCount <= 0 || rowIndex < 0 || rowIndex >= rowCount)
            throw new IllegalArgumentException("Invalid keyboard height geometry");
        int total = baseHeight * rowCount + designHeightAdjustment(adjustment);
        return total / rowCount + (rowIndex < total % rowCount ? 1 : 0);
    }

    public static String display(int tenths) {
        return NumberPolicy.decimal1(tenths / 10.0);
    }

    public static String displayHeight(int adjustment) {
        int value = heightAdjustment(adjustment);
        return (value > 0 ? "+" : "") + value;
    }

    public static int halfGapPixels(int tenths, float density) {
        if (!Float.isFinite(density) || density <= 0) return 0;
        return BoundsPolicy.nonNegative(Math.round(tenths * density / 20f));
    }

    /** Convert a size to pixels while guaranteeing at least one physical pixel. */
    public static int atLeastOnePixel(Context context, float dp) {
        return BoundsPolicy.atLeast(pixels(context, dp), 1);
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

    /** Create a vertical container for stacked keyboard content. */
    public static LinearLayout column(Context context) {
        return ViewPolicy.newColumn(context);
    }

    /** Create a horizontal container for inline keyboard content. */
    public static LinearLayout row(Context context) {
        return ViewPolicy.newRow(context);
    }

    /** Create linear layout parameters from density-independent dimensions. */
    public static LinearLayout.LayoutParams linearParams(Context context, float widthDp, float heightDp) {
        return new LinearLayout.LayoutParams(pixels(context, widthDp), pixels(context, heightDp));
    }

    /** Create linear layout parameters from already pixel-sized dimensions. */
    public static LinearLayout.LayoutParams linearParamsPx(int widthPixels, int heightPixels) {
        return new LinearLayout.LayoutParams(widthPixels, heightPixels);
    }

    /** Create full-width linear layout parameters with content-sized height. */
    public static LinearLayout.LayoutParams matchWidthWrapParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create linear layout parameters that fill both dimensions. */
    public static LinearLayout.LayoutParams matchParentParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.MATCH_PARENT);
    }

    /** Create linear layout parameters that wrap both dimensions. */
    public static LinearLayout.LayoutParams wrapParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create content-height parameters whose width either wraps or fills the parent. */
    public static LinearLayout.LayoutParams wrapOrMatchWidthParams(boolean wrapWidth) {
        return new LinearLayout.LayoutParams(wrapWidth
            ? LinearLayout.LayoutParams.WRAP_CONTENT : LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create weighted linear layout parameters that fill the parent's height. */
    public static LinearLayout.LayoutParams weightedMatchParentParams(float weight) {
        return new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, weight);
    }

    /** Create weighted linear layout parameters with content-sized height. */
    public static LinearLayout.LayoutParams weightedWrapParams(float weight) {
        return new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, weight);
    }

    /** Create weighted linear layout parameters with an already pixel-sized height. */
    public static LinearLayout.LayoutParams weightedHeightPxParams(int heightPixels, float weight) {
        return new LinearLayout.LayoutParams(0, heightPixels, weight);
    }

    /** Create weighted linear layout parameters that fill width with a zero-height basis. */
    public static LinearLayout.LayoutParams weightedWidthParams(float weight) {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, weight);
    }

    /** Create weighted linear layout parameters with zero width and height bases. */
    public static LinearLayout.LayoutParams weightedZeroParams(float weight) {
        return new LinearLayout.LayoutParams(0, 0, weight);
    }

    /** Fill the cross axis while distributing the main axis by weight. */
    public static LinearLayout.LayoutParams weightedCrossAxisFillParams(boolean horizontal,
            float weight) {
        return horizontal ? weightedMatchParentParams(weight) : weightedWidthParams(weight);
    }

    /** Create full-width linear layout parameters with an already pixel-sized height. */
    public static LinearLayout.LayoutParams matchWidthHeightPx(int heightPixels) {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, heightPixels);
    }

    /** Create linear layout parameters with content-sized width and parent-sized height. */
    public static LinearLayout.LayoutParams wrapMatchParentParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.MATCH_PARENT);
    }

    /** Create frame layout parameters that fill both parent dimensions. */
    public static FrameLayout.LayoutParams frameMatchParentParams() {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT);
    }

    /** Create frame layout parameters that wrap both dimensions. */
    public static FrameLayout.LayoutParams frameWrapParams() {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT,
            FrameLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create frame layout parameters that fill width with an already pixel-sized height. */
    public static FrameLayout.LayoutParams frameMatchWidthHeightPx(int heightPixels) {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, heightPixels);
    }

    /** Create frame layout parameters for a pixel-sized square. */
    public static FrameLayout.LayoutParams squareFrameParamsPx(int size) {
        return new FrameLayout.LayoutParams(size, size);
    }

    /** Create frame layout parameters from already pixel-sized dimensions. */
    public static FrameLayout.LayoutParams frameParamsPx(int widthPixels, int heightPixels) {
        return new FrameLayout.LayoutParams(widthPixels, heightPixels);
    }

    /** Create frame layout parameters from pixel dimensions and explicit gravity. */
    public static FrameLayout.LayoutParams frameParamsPx(int widthPixels, int heightPixels,
            int gravity) {
        return new FrameLayout.LayoutParams(widthPixels, heightPixels, gravity);
    }

    /** Create frame layout parameters that fill width with content-sized height. */
    public static FrameLayout.LayoutParams frameMatchWidthWrapParams() {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create frame layout parameters that fill width with content-sized height and gravity. */
    public static FrameLayout.LayoutParams frameMatchWidthWrapParams(int gravity) {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.WRAP_CONTENT, gravity);
    }

    /** Create scroll-view child parameters that fill both dimensions. */
    public static ScrollView.LayoutParams scrollMatchParentParams() {
        return new ScrollView.LayoutParams(ScrollView.LayoutParams.MATCH_PARENT,
            ScrollView.LayoutParams.MATCH_PARENT);
    }

    /** Create scroll-view child parameters that fill width and wrap content height. */
    public static ScrollView.LayoutParams scrollMatchWidthWrapParams() {
        return new ScrollView.LayoutParams(ScrollView.LayoutParams.MATCH_PARENT,
            ScrollView.LayoutParams.WRAP_CONTENT);
    }

    /** Apply the same start and end margin to a layout parameter. */
    public static void setHorizontalMargins(ViewGroup.MarginLayoutParams params, int margin) {
        params.setMarginStart(margin);
        params.setMarginEnd(margin);
    }

    /** Create weighted linear layout parameters with a fixed height in dp. */
    public static LinearLayout.LayoutParams weightedHeightParams(Context context, float heightDp,
            float weight) {
        return new LinearLayout.LayoutParams(0, pixels(context, heightDp), weight);
    }

    /** Apply symmetric horizontal and vertical padding expressed in dp. */
    public static void setSymmetricPaddingDp(View view, Context context, float horizontalDp,
            float verticalDp) {
        int horizontal = pixels(context, horizontalDp);
        int vertical = pixels(context, verticalDp);
        ViewPolicy.setSymmetricPadding(view, horizontal, vertical);
    }

    /** Apply equal horizontal dp padding with no vertical padding. */
    public static void setHorizontalPaddingDp(View view, Context context, float horizontalDp) {
        int horizontal = pixels(context, horizontalDp);
        ViewPolicy.setHorizontalPadding(view, horizontal);
    }

    /** Apply four-sided padding expressed in density-independent pixels. */
    public static void setPaddingDp(View view, Context context, float leftDp, float topDp,
            float rightDp, float bottomDp) {
        ViewPolicy.setPadding(view, pixels(context, leftDp), pixels(context, topDp),
            pixels(context, rightDp), pixels(context, bottomDp));
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
     * <p>键高、候选行高和工具栏都是固定 dp，而国产机出厂常把字体设成「大」甚至「超大」（1.3–2.0 倍）。不封顶时 22 sp 的字母在 52 dp 的键里放不下：文字超出内边距框时 TextView 不再居中，而是从上内边距处往下排，字母被挤到键底被裁掉，`123` 折成两行，「中」只剩顶上一截。系统键盘（Gboard、iOS）的键面同样不随系统字号无限放大。
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

    /** 键盘里没有另定字号的按键和文字用的字号（sp），与 Material 按钮的默认字号相同。不设时取的是系统主题里的按钮字号：各厂商不同，而且不受 {@link #keyboardFontScale} 封顶，九键的 ABC 在大字体下会折行。 */
    public static final float DEFAULT_KEY_TEXT_SP = 14f;

    /** 以 {@link #keySp} 设置键盘里控件的字号。 */
    public static void setKeyTextSize(android.widget.TextView view, float sp) {
        ViewPolicy.setTextSize(view, keySp(view.getContext(), sp));
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
        ViewPolicy.setHorizontalPadding(key, horizontal);
        ViewPolicy.clearFontPadding(key);
        ViewPolicy.clearMinimumSize(key);
    }

    public static int bounded(int value, int minimum, int maximum) {
        return BoundsPolicy.bounded(value, minimum, maximum);
    }

    public static long bounded(long value, long minimum, long maximum) {
        return BoundsPolicy.bounded(value, minimum, maximum);
    }

    public static double bounded(double value, double minimum, double maximum) {
        return BoundsPolicy.bounded(value, minimum, maximum);
    }

    public static double bounded(double value, double minimum, double maximum, double fallback) {
        return Double.isFinite(value) ? bounded(value, minimum, maximum) : fallback;
    }

    public static float bounded(float value, float minimum, float maximum) {
        return BoundsPolicy.bounded(value, minimum, maximum);
    }

    private static int clamp(int value, int minimum, int maximum, int fallback) {
        if (value < 0) return fallback;
        return bounded(value, minimum, maximum);
    }
}

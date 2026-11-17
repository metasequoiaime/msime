package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.Typeface;
import android.util.TypedValue;
import android.widget.TextView;
import java.util.LinkedHashMap;

/**
 * 带提示的字母键：右上角画符号提示（设计的 q1 … m/，10 sp kbSub，上 3 dp、右 5 dp），底边留给双拼提示。
 *
 * <p>两种提示互不影响，都只绘制、不进节点 text，所以字母键的无障碍文本和描述保持原样。颜色由调用方从皮肤取后传入。{@link ShuangpinHintButton} 是它只用底部提示的子类。
 */
public class KeyHintButton extends KeyboardPressButton {
    /** 右上角提示的字号（sp）与离键边的距离（dp），来自设计令牌 §5。 */
    public static final float CORNER_HINT_SP = 10f;
    public static final float CORNER_HINT_TOP_DP = 3f;
    public static final float CORNER_HINT_RIGHT_DP = 5f;

    private final Paint hintPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint cornerPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final int basePaddingTop;
    private final int basePaddingBottom;
    private final int basePaddingRight;
    private String hintText = "";
    private int hintColor = Color.GRAY;
    private String cornerHint = "";
    private int cornerHintColor = Color.GRAY;
    private float unhintedTextSize;
    private int hintWidth;
    private int hintHeight;
    private final RecentResults<InsetInput, Insets> insetCache = new RecentResults<>(8);

    /**
     * 算内边距和字号上界用到的全部输入：输入相同，结果就相同。
     *
     * <p>14/18 键的分词键在组字开始和结束时换「辅」角标，每次都让整排重新布局；LinearLayout 的 forceUniformWidth 又把每个字母键按两种宽度各量一遍。只记上一次的尺寸时两次都对不上，每个键每次布局都从头量 26 个字母和角标；两种宽度的字号上界又不同，TextView 的自动字号每量一次就重配一次、逐档排版试字号，打字时一帧要一百多毫秒。按输入记下内边距和字号后，重新布局只是查表，值没变就什么都不设。
     */
    private record InsetInput(int width, int height, String label, String hintText, String cornerHint,
            float unhintedTextSize, Typeface typeface, float letterSpacing, float textScaleX,
            float keySp, int paddingLeft) { }

    /** 量出来的上、右、下内边距和主标签的字号（px）。 */
    private record Insets(int top, int right, int bottom, int size) { }

    /** 按输入记最近几份结果，满了丢最久没用的那份。每个键一张（一次布局里两种宽度，加上组字前后角标不同，各占一份），字母宽度上界全键盘一张。只在主线程用。 */
    private static final class RecentResults<K, V> {
        private final int capacity;
        private final LinkedHashMap<K, V> entries;

        RecentResults(int capacity) {
            this.capacity = capacity;
            entries = new LinkedHashMap<>(capacity * 2, 0.75f, true);
        }

        V get(K key) { return entries.get(key); }

        void put(K key, V value) {
            entries.put(key, value);
            if (entries.size() > capacity) entries.remove(entries.keySet().iterator().next());
        }
    }

    /** 26 个字母和它们各种角标的宽度上界只取决于两支画笔，整个键盘共用一份，见 {@link KeyHintWidthPolicy#letterBounds}。 */
    private record LetterBoundsInput(Typeface main, float mainSize, float letterSpacing, float textScaleX,
            Typeface corner, float cornerSize) { }

    private static final RecentResults<LetterBoundsInput, KeyHintWidthPolicy.Widths> LETTER_BOUNDS = new RecentResults<>(16);

    @SuppressWarnings("this-escape")
    public KeyHintButton(Context context) {
        super(context);
        KeyboardTypographyPolicy.normalizeKeyCap(this);
        basePaddingTop = getPaddingTop();
        basePaddingBottom = getPaddingBottom();
        basePaddingRight = getPaddingRight();
        hintPaint.setTextAlign(Paint.Align.CENTER);
        cornerPaint.setTextAlign(Paint.Align.RIGHT);
        ViewPolicy.setAllCapsFalse(this);
    }

    /** 底部的双拼提示；空串表示不画，并收回为它预留的底边距。 */
    public final void setHintText(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (hintText.equals(next)) return;
        hintText = next;
        updateHintInsets();
        invalidate();
    }

    public final void setHintColor(int color) {
        if (hintColor == color) return;
        hintColor = color;
        invalidate();
    }

    /** 右上角的符号提示（下滑输入的那个字符）；空串或 {@code null} 表示不画。 */
    public final void setCornerHint(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (cornerHint.equals(next)) return;
        cornerHint = next;
        updateHintInsets();
        invalidate();
    }

    /** 带提示的键面字号由 {@link #updateHintInsets(int, int)} 按键收过；工厂随后指定的字号记成新的基准，再按它重收。 */
    @Override public void setTextSize(int unit, float size) {
        if (unhintedTextSize > 0) {
            unhintedTextSize = TypedValue.applyDimension(unit, size,
                getResources().getDisplayMetrics());
            setAutoSizeTextTypeWithDefaults(TextView.AUTO_SIZE_TEXT_TYPE_NONE);
            super.setTextSize(unit, size);
            updateHintInsets();
        } else super.setTextSize(unit, size);
    }

    /** 提示占据独立区域；主标签按剩余宽高缩放，不能因底部提示把字母推到右上角符号上。 */
    private void updateHintInsets() {
        updateHintInsets(getWidth(), getHeight());
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int width = MeasureSpec.getSize(widthMeasureSpec);
        int height = MeasureSpec.getSize(heightMeasureSpec);
        if ((!hintText.isEmpty() || !cornerHint.isEmpty()) && (hintWidth != width || hintHeight != height))
            updateHintInsets(width, height);
        super.onMeasure(widthMeasureSpec, heightMeasureSpec);
    }

    private void updateHintInsets(int width, int height) {
        hintWidth = width;
        hintHeight = height;
        if (hintText.isEmpty() && cornerHint.isEmpty()) {
            if (getAutoSizeTextType() != TextView.AUTO_SIZE_TEXT_TYPE_NONE)
                setAutoSizeTextTypeWithDefaults(TextView.AUTO_SIZE_TEXT_TYPE_NONE);
            float restoredSize = unhintedTextSize;
            unhintedTextSize = 0;
            if (restoredSize > 0) super.setTextSize(TypedValue.COMPLEX_UNIT_PX, restoredSize);
            ViewPolicy.setPaddingIfChanged(this, getPaddingLeft(), basePaddingTop, basePaddingRight, basePaddingBottom);
            return;
        }
        if (unhintedTextSize == 0) unhintedTextSize = getTextSize();
        Paint paint = getPaint();
        InsetInput input = new InsetInput(width, height, getText().toString(), hintText, cornerHint, unhintedTextSize,
            paint.getTypeface(), paint.getLetterSpacing(), paint.getTextScaleX(),
            KeyboardTypographyPolicy.keySp(getContext(), 1), getPaddingLeft());
        Insets insets = insetCache.get(input);
        if (insets == null) {
            insets = measureInsets(width, height);
            insetCache.put(input, insets);
        }
        // 只设变了的值：内边距、行数、字号任何一项真的变了才会让这个键（和它所在的那一排）重新布局。
        ViewPolicy.setPaddingIfChanged(this, getPaddingLeft(), insets.top(), insets.right(), insets.bottom());
        if (getMaxLines() != 1) setMaxLines(1);
        setHorizontallyScrolling(false);
        // 字号由 measureInsets 按 TextView 自动字号的同一判定算好并缓存，不开 TextView 的自动字号：它每次重配和每次布局都要逐档排版。
        if (getAutoSizeTextType() != TextView.AUTO_SIZE_TEXT_TYPE_NONE)
            setAutoSizeTextTypeWithDefaults(TextView.AUTO_SIZE_TEXT_TYPE_NONE);
        if (getTextSize() != insets.size()) super.setTextSize(TypedValue.COMPLEX_UNIT_PX, insets.size());
    }

    /** 按当前键面、提示和尺寸量出内边距和主标签字号，只读不设，结果由 {@link #updateHintInsets(int, int)} 缓存。 */
    private Insets measureInsets(int width, int height) {
        float scale = CompactKeyTypography.scale(getContext(), height);
        ViewPolicy.setTextSize(hintPaint, KeyboardTypographyPolicy.keySp(getContext(), 9) * scale);
        Paint.FontMetrics bottom = hintPaint.getFontMetrics();
        int bottomInset = hintText.isEmpty() ? basePaddingBottom
            : (int) Math.ceil(bottom.bottom - bottom.top + DimensionPolicy.floatPixels(getContext(), 4) * scale);
        int topInset = basePaddingTop;
        int rightInset = basePaddingRight;
        Paint main = new Paint(getPaint());
        main.setTextSize(unhintedTextSize * scale);
        float mainWidth = main.measureText(getText().toString());
        if (!cornerHint.isEmpty()) {
            ViewPolicy.setTextSize(cornerPaint, KeyboardTypographyPolicy.keySp(getContext(), CORNER_HINT_SP) * scale);
            Paint.FontMetrics corner = cornerPaint.getFontMetrics();
            // 宽键帽的中央字母与右上角标能横向错开，无需把整行顶部都让给角标。
            KeyHintWidthPolicy.Widths widths = KeyHintWidthPolicy.measure(getText().toString(), cornerHint,
                main::measureText, cornerPaint::measureText, () -> letterBounds(main));
            mainWidth = widths.main();
            float cornerWidth = widths.corner()
                + DimensionPolicy.floatPixels(getContext(), CORNER_HINT_RIGHT_DP + 2) * scale;
            int horizontalInset = (int) Math.ceil(Math.max(0, mainWidth + 2 * cornerWidth - width));
            if (hintText.isEmpty() && width > 0
                    && width - getPaddingLeft() - rightInset - horizontalInset >= Math.ceil(mainWidth)) {
                // 横向错开必须仍放得下完整主标签；窄屏或大字体改为上下分区。
                rightInset += horizontalInset;
            } else if (width <= 0 || mainWidth + 2 * cornerWidth > width) {
                String label = getText().toString();
                if (label.length() == 1 && (label.charAt(0) >= 'A' && label.charAt(0) <= 'Z'
                        || label.charAt(0) >= 'a' && label.charAt(0) <= 'z'))
                    label = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
                Rect ink = new Rect();
                main.getTextBounds(label, 0, label.length(), ink);
                // 字形在排版行顶部自带留白；两层留白不能重复挤掉主标签的高度。
                float leading = Math.max(0, ink.top - main.ascent());
                topInset = Math.max(basePaddingTop, (int) Math.ceil(DimensionPolicy.floatPixels(getContext(), CORNER_HINT_TOP_DP) * scale
                    + corner.descent - corner.ascent + DimensionPolicy.floatPixels(getContext(), 2) * scale
                    - leading));
            }
        }
        float fit = 1f;
        if (width > 0 && height > 0) {
            Paint.FontMetrics metrics = main.getFontMetrics();
            fit = Math.min(1f, Math.min(
                Math.max(1, width - getPaddingLeft() - rightInset) / Math.max(1f, mainWidth),
                Math.max(1, height - topInset - bottomInset) / Math.max(1f, metrics.descent - metrics.ascent)));
        }
        int maximum = Math.max(2, (int) Math.floor(unhintedTextSize * scale * fit));
        int size = width > 0 && height > 0
            ? fittedSize(getText().toString(), maximum, width - getPaddingLeft() - rightInset, height - topInset - bottomInset)
            : maximum;
        return new Insets(topInset, rightInset, bottomInset, size);
    }

    /**
     * 主标签放得进内容区的最大整数字号（px，1 到 {@code maximum}）。和 TextView 自动字号（最小 1 px、步长 1 px）的判定相同：单行排得下（宽按画笔量，含字距），行高不超过内容区（开着字体留白时按 top 到 bottom）。{@code maximum} 已按同组最宽的字母留够宽度，所以同组的键得到同一个字号。
     */
    private int fittedSize(String label, int maximum, int availableWidth, int availableHeight) {
        Paint probe = new Paint(getPaint());
        for (int size = maximum; size > 1; size--) {
            probe.setTextSize(size);
            if (Math.ceil(probe.measureText(label)) > availableWidth) continue;
            Paint.FontMetricsInt metrics = probe.getFontMetricsInt();
            int line = getIncludeFontPadding() ? metrics.bottom - metrics.top : metrics.descent - metrics.ascent;
            if (line <= availableHeight) return size;
        }
        return 1;
    }

    /** 26 个字母和各种角标在这两支画笔下的宽度上界；同一键盘的字母键共用，只在字体、字号变时重量。 */
    private KeyHintWidthPolicy.Widths letterBounds(Paint main) {
        LetterBoundsInput input = new LetterBoundsInput(main.getTypeface(), main.getTextSize(), main.getLetterSpacing(),
            main.getTextScaleX(), cornerPaint.getTypeface(), cornerPaint.getTextSize());
        KeyHintWidthPolicy.Widths bounds = LETTER_BOUNDS.get(input);
        if (bounds == null) {
            bounds = KeyHintWidthPolicy.letterBounds(main::measureText, cornerPaint::measureText);
            LETTER_BOUNDS.put(input, bounds);
        }
        return bounds;
    }

    public final String cornerHint() { return cornerHint; }

    public final void setCornerHintColor(int color) {
        if (cornerHintColor == color) return;
        cornerHintColor = color;
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (getWidth() <= 0 || getHeight() <= 0) return;
        if (!cornerHint.isEmpty()) drawCornerHint(canvas);
        if (!hintText.isEmpty()) drawBottomHint(canvas);
    }

    private void drawCornerHint(Canvas canvas) {
        float scale = CompactKeyTypography.scale(getContext(), getHeight());
        ViewPolicy.setTextSize(cornerPaint, KeyboardTypographyPolicy.keySp(getContext(), CORNER_HINT_SP) * scale);
        cornerPaint.setColor(cornerHintColor);
        cornerPaint.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), 255, 96));
        Paint.FontMetrics metrics = cornerPaint.getFontMetrics();
        float baseline = DimensionPolicy.floatPixels(getContext(), CORNER_HINT_TOP_DP) * scale - metrics.ascent;
        canvas.drawText(cornerHint,
            getWidth() - DimensionPolicy.floatPixels(getContext(), CORNER_HINT_RIGHT_DP) * scale, baseline,
            cornerPaint);
    }

    private void drawBottomHint(Canvas canvas) {
        float scale = CompactKeyTypography.scale(getContext(), getHeight());
        float size = KeyboardTypographyPolicy.keySp(getContext(), 9) * scale;
        float available = BoundsPolicy.bounded(getWidth() - getPaddingLeft() - getPaddingRight()
            - DimensionPolicy.pixels(getContext(), 4), 1f, Float.MAX_VALUE);
        ViewPolicy.setTextSize(hintPaint, size);
        float width = hintPaint.measureText(hintText);
        if (width > available) ViewPolicy.setTextSize(hintPaint, size * available / width);
        hintPaint.setColor(hintColor);
        hintPaint.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), 204, 96));
        Paint.FontMetrics metrics = hintPaint.getFontMetrics();
        float baseline = getHeight() - DimensionPolicy.floatPixels(getContext(), 2) * scale - metrics.bottom;
        canvas.drawText(hintText, getWidth() / 2f, baseline, hintPaint);
    }
}

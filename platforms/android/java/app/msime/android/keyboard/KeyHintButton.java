package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.util.TypedValue;
import android.widget.TextView;

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
        float scale = CompactKeyTypography.scale(getContext(), height);
        if (hintText.isEmpty() && cornerHint.isEmpty()) {
            setAutoSizeTextTypeWithDefaults(TextView.AUTO_SIZE_TEXT_TYPE_NONE);
            if (unhintedTextSize > 0) setTextSize(TypedValue.COMPLEX_UNIT_PX, unhintedTextSize);
            unhintedTextSize = 0;
            ViewPolicy.setPadding(this, getPaddingLeft(), basePaddingTop, basePaddingRight, basePaddingBottom);
            return;
        }
        if (unhintedTextSize == 0) unhintedTextSize = getTextSize();
        ViewPolicy.setTextSize(hintPaint, KeyboardTypographyPolicy.keySp(getContext(), 9) * scale);
        Paint.FontMetrics bottom = hintPaint.getFontMetrics();
        int bottomInset = hintText.isEmpty() ? basePaddingBottom
            : (int) Math.ceil(bottom.bottom - bottom.top + DimensionPolicy.floatPixels(getContext(), 4) * scale);
        int topInset = basePaddingTop;
        int rightInset = basePaddingRight;
        if (!cornerHint.isEmpty()) {
            ViewPolicy.setTextSize(cornerPaint, KeyboardTypographyPolicy.keySp(getContext(), CORNER_HINT_SP) * scale);
            Paint.FontMetrics corner = cornerPaint.getFontMetrics();
            // 宽键帽的中央字母与右上角标能横向错开，无需把整行顶部都让给角标。
            Paint main = new Paint(getPaint());
            main.setTextSize(unhintedTextSize * scale);
            KeyHintWidthPolicy.Widths widths = KeyHintWidthPolicy.measure(getText().toString(), cornerHint,
                main::measureText, cornerPaint::measureText);
            float mainWidth = widths.main();
            float cornerWidth = widths.corner()
                + DimensionPolicy.floatPixels(getContext(), CORNER_HINT_RIGHT_DP + 2) * scale;
            if (hintText.isEmpty()) {
                // 没有底部提示时优先横向错开，保留低键帽里主字母所需的高度。
                rightInset += (int) Math.ceil(Math.max(0, mainWidth + 2 * cornerWidth - width));
            } else if (width <= 0 || mainWidth + 2 * cornerWidth > width)
                topInset = (int) Math.ceil(DimensionPolicy.floatPixels(getContext(), CORNER_HINT_TOP_DP) * scale
                    + corner.descent - corner.ascent + DimensionPolicy.floatPixels(getContext(), 2) * scale);
        }
        ViewPolicy.setPadding(this, getPaddingLeft(), topInset, rightInset, bottomInset);
        setMaxLines(1);
        setHorizontallyScrolling(false);
        int maximum = Math.max(2, Math.round(unhintedTextSize * scale));
        setAutoSizeTextTypeUniformWithConfiguration(1, maximum, 1, TypedValue.COMPLEX_UNIT_PX);
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

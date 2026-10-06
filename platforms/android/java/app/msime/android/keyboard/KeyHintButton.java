package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.util.TypedValue;

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
    private String hintText = "";
    private int hintColor = Color.GRAY;
    private String cornerHint = "";
    private int cornerHintColor = Color.GRAY;

    @SuppressWarnings("this-escape")
    public KeyHintButton(Context context) {
        super(context);
        basePaddingTop = getPaddingTop();
        basePaddingBottom = getPaddingBottom();
        hintPaint.setTextAlign(Paint.Align.CENTER);
        cornerPaint.setTextAlign(Paint.Align.RIGHT);
        setAllCaps(false);
    }

    /** 底部的双拼提示；空串表示不画，并收回为它预留的底边距。 */
    public final void setHintText(String value) {
        String next = value == null ? "" : value;
        if (hintText.equals(next)) return;
        hintText = next;
        // TextView 把文字裁在内边距围出的框里：在按钮默认的上下内边距之外再留出提示的高度，46 dp 的键只剩不到 30 dp，22 sp 的字母连同字体留白放不下，被裁掉下半截、压在提示上。有提示时上边距归零、去掉字体留白，下边只留提示那一行。
        boolean hinted = !hintText.isEmpty();
        setIncludeFontPadding(!hinted);
        setPadding(getPaddingLeft(), hinted ? 0 : basePaddingTop, getPaddingRight(),
            hinted ? dp(11) : basePaddingBottom);
        invalidate();
    }

    public final void setHintColor(int color) {
        if (hintColor == color) return;
        hintColor = color;
        invalidate();
    }

    /** 右上角的符号提示（下滑输入的那个字符）；空串或 {@code null} 表示不画。 */
    public final void setCornerHint(String value) {
        String next = value == null ? "" : value;
        if (cornerHint.equals(next)) return;
        cornerHint = next;
        invalidate();
    }

    public final String cornerHint() { return cornerHint; }

    public final void setCornerHintColor(int color) {
        if (cornerHintColor == color) return;
        cornerHintColor = color;
        invalidate();
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    private float sp(float value) {
        return TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, value,
            getResources().getDisplayMetrics());
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (getWidth() <= 0 || getHeight() <= 0) return;
        if (!cornerHint.isEmpty()) drawCornerHint(canvas);
        if (!hintText.isEmpty()) drawBottomHint(canvas);
    }

    private void drawCornerHint(Canvas canvas) {
        float density = getResources().getDisplayMetrics().density;
        cornerPaint.setTextSize(sp(CORNER_HINT_SP));
        cornerPaint.setColor(cornerHintColor);
        if (!isEnabled()) cornerPaint.setAlpha(96);
        Paint.FontMetrics metrics = cornerPaint.getFontMetrics();
        float baseline = CORNER_HINT_TOP_DP * density - metrics.ascent;
        canvas.drawText(cornerHint, getWidth() - CORNER_HINT_RIGHT_DP * density, baseline,
            cornerPaint);
    }

    private void drawBottomHint(Canvas canvas) {
        float size = sp(9);
        float available = Math.max(1, getWidth() - getPaddingLeft() - getPaddingRight() - dp(4));
        hintPaint.setTextSize(size);
        while (size > sp(6)
                && hintPaint.measureText(hintText) > available) {
            size -= sp(0.5f);
            hintPaint.setTextSize(size);
        }
        hintPaint.setColor(hintColor);
        hintPaint.setAlpha(isEnabled() ? 204 : 96);
        Paint.FontMetrics metrics = hintPaint.getFontMetrics();
        float baseline = getHeight() - dp(2) - metrics.bottom;
        canvas.drawText(hintText, getWidth() / 2f, baseline, hintPaint);
    }
}

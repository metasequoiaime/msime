package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;

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
        KeyboardGeometry.normalizeKeyCap(this);
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
        // 键帽的上下内边距和字体留白在构造时已归零（KeyboardGeometry.normalizeKeyCap）；有提示时下边只留提示那一行，字母在剩下的高度里居中。
        boolean hinted = !hintText.isEmpty();
        setPadding(getPaddingLeft(), hinted ? 0 : basePaddingTop, getPaddingRight(),
            hinted ? KeyboardGeometry.pixels(getContext(), 11) : basePaddingBottom);
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

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (getWidth() <= 0 || getHeight() <= 0) return;
        if (!cornerHint.isEmpty()) drawCornerHint(canvas);
        if (!hintText.isEmpty()) drawBottomHint(canvas);
    }

    private void drawCornerHint(Canvas canvas) {
        cornerPaint.setTextSize(KeyboardGeometry.keySp(getContext(), CORNER_HINT_SP));
        cornerPaint.setColor(cornerHintColor);
        if (!isEnabled()) cornerPaint.setAlpha(96);
        Paint.FontMetrics metrics = cornerPaint.getFontMetrics();
        float baseline = KeyboardGeometry.floatPixels(getContext(), CORNER_HINT_TOP_DP) - metrics.ascent;
        canvas.drawText(cornerHint,
            getWidth() - KeyboardGeometry.floatPixels(getContext(), CORNER_HINT_RIGHT_DP), baseline,
            cornerPaint);
    }

    private void drawBottomHint(Canvas canvas) {
        float size = KeyboardGeometry.keySp(getContext(), 9);
        float available = Math.max(1, getWidth() - getPaddingLeft() - getPaddingRight()
            - KeyboardGeometry.pixels(getContext(), 4));
        hintPaint.setTextSize(size);
        while (size > KeyboardGeometry.keySp(getContext(), 6)
                && hintPaint.measureText(hintText) > available) {
            size -= KeyboardGeometry.keySp(getContext(), 0.5f);
            hintPaint.setTextSize(size);
        }
        hintPaint.setColor(hintColor);
        hintPaint.setAlpha(isEnabled() ? 204 : 96);
        Paint.FontMetrics metrics = hintPaint.getFontMetrics();
        float baseline = getHeight() - KeyboardGeometry.pixels(getContext(), 2) - metrics.bottom;
        canvas.drawText(hintText, getWidth() / 2f, baseline, hintPaint);
    }
}

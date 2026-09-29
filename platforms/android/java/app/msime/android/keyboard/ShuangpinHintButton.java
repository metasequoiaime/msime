package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.util.TypedValue;

/** A letter key that reserves its lower edge for a double-pinyin hint. */
public final class ShuangpinHintButton extends KeyboardPressButton {
    private final Paint hintPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final int basePaddingBottom;
    private String hintText = "";
    private int hintColor = Color.GRAY;

    public ShuangpinHintButton(Context context) {
        super(context);
        basePaddingBottom = getPaddingBottom();
        hintPaint.setTextAlign(Paint.Align.CENTER);
        setAllCaps(false);
    }

    public void setHintText(String value) {
        String next = value == null ? "" : value;
        if (hintText.equals(next)) return;
        hintText = next;
        int extra = hintText.isEmpty() ? 0 : dp(9);
        setPadding(getPaddingLeft(), getPaddingTop(), getPaddingRight(),
            basePaddingBottom + extra);
        invalidate();
    }

    public void setHintColor(int color) {
        if (hintColor == color) return;
        hintColor = color;
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
        if (hintText.isEmpty() || getWidth() <= 0 || getHeight() <= 0) return;
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

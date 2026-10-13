package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;

/** 九键键帽：数字缩成右上角标记，主键面保持居中。 */
public final class NineKeyDigitButton extends KeyboardPressButton {
    private final Paint digitPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private String digitText = "";
    private int digitColor = Color.GRAY;

    public NineKeyDigitButton(Context context) {
        super(context);
        KeyboardTypographyPolicy.normalizeKeyCap(this);
        digitPaint.setTextAlign(Paint.Align.RIGHT);
        ViewPolicy.setAllCapsFalse(this);
    }

    /** 右上角的数字标记；数字键面本身显示数字时为空。 */
    public void setDigitText(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (digitText.equals(next)) return;
        digitText = next;
        invalidate();
    }

    public void setDigitColor(int color) {
        if (digitColor == color) return;
        digitColor = color;
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (digitText.isEmpty() || getWidth() <= 0 || getHeight() <= 0) return;
        ViewPolicy.setTextSize(digitPaint,
            KeyboardTypographyPolicy.keySp(getContext(), KeyboardTypographyPolicy.NINE_KEY_DIGIT_TEXT_SP));
        digitPaint.setColor(digitColor);
        digitPaint.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), 204, 96));
        Paint.FontMetrics metrics = digitPaint.getFontMetrics();
        canvas.drawText(digitText, getWidth() - DimensionPolicy.pixels(getContext(), 6),
            DimensionPolicy.pixels(getContext(), 3) - metrics.top, digitPaint);
    }
}

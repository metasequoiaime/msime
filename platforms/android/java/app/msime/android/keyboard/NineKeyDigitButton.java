package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;

/** 九键键帽：数字缩成右上角标记，字号、位置和颜色与 26 键字母的角标相同（{@link KeyHintButton}：10 sp，距上边 3 dp、右边 5 dp，皮肤的 `hint` 色），主键面不为它让位、保持居中。 */
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
        ViewPolicy.setTextSize(digitPaint, KeyboardTypographyPolicy.keySp(getContext(), KeyHintButton.CORNER_HINT_SP));
        digitPaint.setColor(digitColor);
        digitPaint.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), 255, 96));
        Paint.FontMetrics metrics = digitPaint.getFontMetrics();
        float baseline = DimensionPolicy.floatPixels(getContext(), KeyHintButton.CORNER_HINT_TOP_DP) - metrics.ascent;
        canvas.drawText(digitText,
            getWidth() - DimensionPolicy.floatPixels(getContext(), KeyHintButton.CORNER_HINT_RIGHT_DP), baseline,
            digitPaint);
    }
}

package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;

/**
 * 九键键帽：字母下面是它送进引擎的数字，所以数字印在上沿。
 *
 * <p>The grid sends `2` for the key drawn `ABC`, and a hold offers that digit directly. Printing it
 * above the letters is what makes both readable without a second tap, and it is the same legend the
 * shared design draws on the other platforms.
 */
public final class NineKeyDigitButton extends KeyboardPressButton {
    private final Paint digitPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final int basePaddingTop;
    private String digitText = "";
    private int digitColor = Color.GRAY;

    public NineKeyDigitButton(Context context) {
        super(context);
        basePaddingTop = getPaddingTop();
        digitPaint.setTextAlign(Paint.Align.CENTER);
        setAllCaps(false);
    }

    /** The digit printed above the letters; empty on the digit layer, where the face is the digit. */
    public void setDigitText(String value) {
        String next = value == null ? "" : value;
        if (digitText.equals(next)) return;
        digitText = next;
        float density = getResources().getDisplayMetrics().density;
        setPadding(getPaddingLeft(), basePaddingTop + (digitText.isEmpty() ? 0 : KeyboardGeometry.pixels(10, density)),
            getPaddingRight(), getPaddingBottom());
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
        digitPaint.setTextSize(KeyboardGeometry.sp(getContext(), 10));
        digitPaint.setColor(digitColor);
        digitPaint.setAlpha(isEnabled() ? 204 : 96);
        Paint.FontMetrics metrics = digitPaint.getFontMetrics();
        float density = getResources().getDisplayMetrics().density;
        canvas.drawText(digitText, getWidth() / 2f, KeyboardGeometry.pixels(3, density) - metrics.top, digitPaint);
    }
}

package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;

/** 上半部提示长按符号，下半部显示换行；节点文字与系统按钮的点击、长按生命周期保持一致。 */
public final class TextEditReturnButton extends KeyboardPressButton {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);

    public TextEditReturnButton(Context context) {
        super(context);
        KeyboardTypographyPolicy.normalizeKeyCap(this);
        ViewPolicy.setAllCapsFalse(this);
        paint.setTextAlign(Paint.Align.CENTER);
    }

    @Override protected void onDraw(Canvas canvas) {
        float half = getHeight() / 2f;
        if (getWidth() <= 0 || half <= 0) return;
        paint.setTypeface(getTypeface());
        drawLabel(canvas, "符号", half / 2, half, 14);
        drawLabel(canvas, getText().toString(), half * 1.5f, half, 16);
        paint.setColor(getCurrentTextColor());
        paint.setAlpha(Math.round(Color.alpha(getCurrentTextColor()) * .45f));
        paint.setStrokeWidth(DimensionPolicy.floatPixels(getContext(), 1));
        float inset = DimensionPolicy.floatPixels(getContext(), 4);
        canvas.drawLine(inset, half, Math.max(inset, getWidth() - inset), half, paint);
    }

    private void drawLabel(Canvas canvas, String text, float centerY, float height, float sp) {
        ViewPolicy.setTextSize(paint, KeyboardTypographyPolicy.keySp(getContext(), sp));
        Paint.FontMetrics metrics = paint.getFontMetrics();
        float inset = DimensionPolicy.floatPixels(getContext(), 4);
        float width = Math.max(1, getWidth() - inset * 2);
        float scale = Math.min(1, Math.min(width / Math.max(1, paint.measureText(text)),
            Math.max(1, height - inset) / Math.max(1, metrics.descent - metrics.ascent)));
        ViewPolicy.setTextSize(paint, paint.getTextSize() * scale);
        metrics = paint.getFontMetrics();
        paint.setColor(getCurrentTextColor());
        canvas.drawText(text, getWidth() / 2f, centerY - (metrics.ascent + metrics.descent) / 2, paint);
    }
}

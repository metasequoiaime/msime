package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import java.util.function.IntSupplier;

/** Draws the tintable MSIME brand mark without baking a white square into the shortcut bar. */
public final class KeyboardBrandButton extends KeyboardPressButton {
    /**
     * The mark's share of its touch target.
     *
     * <p>Larger than the stroked glyphs beside it because the drawn path leaves more slack inside
     * its own 110-unit box; the two read at the same weight in the flat toolbar.
     */
    private static final float MARK_SCALE = 0.72f;

    private final IntSupplier accent;
    private final Paint mark = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path path = new Path();

    public KeyboardBrandButton(Context context, IntSupplier accent) {
        super(context);
        this.accent = accent;
        setKeyboardRole(KeyboardKeyRole.GLYPH);
        path.moveTo(74.7234f, 14f);
        path.lineTo(35.1501f, 29.1727f);
        path.lineTo(74.7234f, 40.5522f);
        path.lineTo(35.1501f, 59.518f);
        path.cubicTo(72.562f, 65.84f, 107.728f, 71.024f, 33f, 95f);
        mark.setStyle(Paint.Style.STROKE);
        mark.setStrokeWidth(8f);
        mark.setStrokeCap(Paint.Cap.ROUND);
        mark.setStrokeJoin(Paint.Join.ROUND);
        setContentDescription("更多快捷设置");
    }

    @Override protected void onDraw(Canvas canvas) {
        // The text remains available to accessibility and device smoke tests, while the visible
        // shortcut is the same mark Apple turns into an accent-tinted template.
        int color;
        try {
            color = accent.getAsInt();
        } catch (RuntimeException error) {
            color = Color.WHITE;
        }
        mark.setColor(color);
        mark.setAlpha(isEnabled() ? 255 : 96);
        int width = Math.max(0, getWidth() - getPaddingLeft() - getPaddingRight());
        int height = Math.max(0, getHeight() - getPaddingTop() - getPaddingBottom());
        float size = Math.min(width, height) * MARK_SCALE;
        if (size <= 0) return;
        canvas.save();
        canvas.translate(getPaddingLeft() + (width - size) / 2f,
            getPaddingTop() + (height - size) / 2f);
        canvas.scale(size / 110f, size / 110f);
        canvas.drawPath(path, mark);
        canvas.restore();
    }
}

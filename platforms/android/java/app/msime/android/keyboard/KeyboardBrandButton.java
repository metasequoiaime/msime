package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
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
    private final Paint mark = KeyboardBrandMark.newPaint();

    public KeyboardBrandButton(Context context, IntSupplier accent) {
        super(context);
        this.accent = accent;
        setKeyboardRole(KeyboardKeyRole.GLYPH);
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
        KeyboardBrandMark.draw(canvas, mark, getPaddingLeft(), getPaddingTop(), width, height,
            MARK_SCALE);
    }
}

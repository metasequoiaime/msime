package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.view.View;
import java.util.function.IntSupplier;

/**
 * The MSIME brand mark as a decorative, non-interactive view, and the one place its path is defined.
 *
 * <p>The shortcut bar's {@link KeyboardBrandButton} draws the same mark through {@link #draw}; this view leads the candidate header the way the macOS candidate window leads its top row, so it neither takes touches nor speaks to accessibility services — the header's own text already names the input method.
 */
public final class KeyboardBrandMark extends View {
    /** Side of the square the path coordinates are laid out in. */
    private static final float PATH_BOX = 110f;

    private static final Path PATH = new Path();

    static {
        PATH.moveTo(74.7234f, 14f);
        PATH.lineTo(35.1501f, 29.1727f);
        PATH.lineTo(74.7234f, 40.5522f);
        PATH.lineTo(35.1501f, 59.518f);
        PATH.cubicTo(72.562f, 65.84f, 107.728f, 71.024f, 33f, 95f);
    }

    /** A stroke paint configured for the mark; the caller only sets its colour and alpha. */
    static Paint newPaint() {
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(8f);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setStrokeJoin(Paint.Join.ROUND);
        return paint;
    }

    /** Draws the mark centred in the given content box at {@code scale} of its shorter side. */
    static void draw(Canvas canvas, Paint paint, float left, float top, float width, float height,
            float scale) {
        float size = KeyboardGeometry.shorterSide(width, height) * scale;
        if (size <= 0) return;
        canvas.save();
        canvas.translate(left + (width - size) / 2f, top + (height - size) / 2f);
        canvas.scale(size / PATH_BOX, size / PATH_BOX);
        canvas.drawPath(PATH, paint);
        canvas.restore();
    }

    private final IntSupplier accent;
    private final Paint mark = newPaint();

    public KeyboardBrandMark(Context context, IntSupplier accent) {
        super(context);
        this.accent = accent;
        ViewPolicy.setNonInteractive(this);
        ViewPolicy.hideFromAccessibility(this);
    }

    @Override protected void onDraw(Canvas canvas) {
        int color;
        try {
            color = accent.getAsInt();
        } catch (RuntimeException error) {
            color = Color.WHITE;
        }
        mark.setColor(color);
        int width = KeyboardGeometry.contentWidth(this);
        int height = KeyboardGeometry.contentHeight(this);
        draw(canvas, mark, getPaddingLeft(), getPaddingTop(), width, height, 1f);
    }
}

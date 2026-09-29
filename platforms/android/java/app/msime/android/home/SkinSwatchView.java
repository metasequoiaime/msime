package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.KeyboardSkin;

/**
 * 一款皮肤的缩略：底色、几枚键帽，和那一枚强调键。
 *
 * <p>A catalogue of designs that shows only names and download counts asks the reader to pick a
 * colour scheme by its title. This is the smallest drawing that answers the question the list is
 * for -- what does it look like -- without downloading anything.
 */
public final class SkinSwatchView extends View {
    private static final int ROWS = 3;
    private static final int COLUMNS = 4;

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    @Nullable private KeyboardSkin skin;

    public SkinSwatchView(Context context) { super(context); }

    public SkinSwatchView(Context context, AttributeSet attributes) { super(context, attributes); }

    /** Show one design, or nothing at all when the entry carries none. */
    public void setSkin(@Nullable KeyboardSkin value) {
        skin = value;
        setVisibility(value == null ? GONE : VISIBLE);
        invalidate();
    }

    private float dp(float value) { return value * getResources().getDisplayMetrics().density; }

    private int parse(String colour, int fallback) {
        if (colour == null || colour.isEmpty()) return fallback;
        try {
            return Color.parseColor(colour);
        } catch (IllegalArgumentException error) {
            return fallback;
        }
    }

    @Override protected void onDraw(Canvas canvas) {
        KeyboardSkin value = skin;
        if (value == null || getWidth() <= 0 || getHeight() <= 0) return;
        float radius = dp(8);
        paint.setColor(parse(value.background(), Color.LTGRAY));
        box.set(0, 0, getWidth(), getHeight());
        canvas.drawRoundRect(box, radius, radius, paint);

        float pad = dp(4);
        float gap = dp(2);
        float cellWidth = (getWidth() - pad * 2 - gap * (COLUMNS - 1)) / COLUMNS;
        float cellHeight = (getHeight() - pad * 2 - gap * (ROWS - 1)) / ROWS;
        if (cellWidth <= 0 || cellHeight <= 0) return;
        float capRadius = Math.min(dp((float) value.cornerRadius()) / 2f, cellHeight / 2f);
        int cap = parse(value.keyBackground(), Color.WHITE);
        int accent = parse(value.actionBackground(), Color.DKGRAY);
        for (int row = 0; row < ROWS; row++) {
            for (int column = 0; column < COLUMNS; column++) {
                // The bottom-right key is the emphasized one on every layout this draws, so the
                // swatch shows both faces a skin defines rather than only its key colour.
                boolean emphasized = row == ROWS - 1 && column == COLUMNS - 1;
                paint.setColor(emphasized ? accent : cap);
                float left = pad + column * (cellWidth + gap);
                float top = pad + row * (cellHeight + gap);
                box.set(left, top, left + cellWidth, top + cellHeight);
                canvas.drawRoundRect(box, capRadius, capRadius, paint);
            }
        }
    }
}

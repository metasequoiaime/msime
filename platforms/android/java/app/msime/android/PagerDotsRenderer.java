package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;

/** Shared drawing for page indicators with an animated active dot. */
public final class PagerDotsRenderer {
    private PagerDotsRenderer() {}

    public static void draw(Canvas canvas, Paint paint, RectF rect, int count, int active,
            int previous, float progress, float dot, float wide, float gap,
            int inactiveColor, int activeColor, float startX, float top) {
        float x = startX;
        float blend = BoundsPolicy.bounded(progress, 0f, 1f);
        for (int index = 0; index < count; index++) {
            float weight = index == active ? blend : index == previous ? 1f - blend : 0f;
            float width = dot + (wide - dot) * weight;
            paint.setColor(ColorPolicy.blend(inactiveColor, activeColor, weight));
            rect.set(x, top, x + width, top + dot);
            canvas.drawRoundRect(rect, dot / 2f, dot / 2f, paint);
            x += width + gap;
        }
    }
}

package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;

/** Pressable Apple-style skin card with a miniature keyboard preview. */
public final class KeyboardSkinCard extends KeyboardPressButton {
    private final KeyboardSkin skin;
    private final String title;
    private final float density;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);

    public KeyboardSkinCard(Context context, KeyboardSkin skin, String title) {
        super(context);
        this.skin = skin;
        this.title = title;
        density = getResources().getDisplayMetrics().density;
        setAllCaps(false);
        setBackground(null);
        setText(null);
        setWillNotDraw(false);
        setContentDescription(title);
    }

    @Override protected void onDraw(Canvas canvas) {
        float radius = 12 * density;
        RectF card = new RectF(1, 1, getWidth() - 1, getHeight() - 1);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.parseColor(skin.background()));
        canvas.drawRoundRect(card, radius, radius, paint);
        RectF preview = new RectF(7 * density, 31 * density,
            getWidth() - 7 * density, getHeight() - 7 * density);
        KeyboardSkinPreview.drawPreview(canvas, preview, skin, density);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth((isSelected() ? 2 : 1) * density);
        paint.setColor(Color.parseColor(isSelected() ? skin.keyForeground() : skin.accent()));
        canvas.drawRoundRect(card, radius, radius, paint);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.parseColor(skin.keyForeground()));
        paint.setTextSize(13 * density);
        paint.setTypeface(skin.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        paint.setTextAlign(Paint.Align.LEFT);
        String label = title.length() > 18 ? title.substring(0, 17) + "…" : title;
        canvas.drawText(label, 10 * density, 21 * density, paint);
    }
}

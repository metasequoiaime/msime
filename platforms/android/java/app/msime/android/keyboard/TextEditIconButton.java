package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;

/** 编辑面板共用现有矢量图标，旋转 chevron 得到四向尖括号，旋转文首图标得到文尾。 */
public final class TextEditIconButton extends TextEditPanelButton {
    private final KeyboardIconPaths.Icon icon;
    private final float rotation;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private float sizeDp = 24;

    public TextEditIconButton(Context context, KeyboardIconPaths.Icon icon, float rotation) {
        super(context);
        this.icon = icon;
        this.rotation = rotation;
        KeyboardTypographyPolicy.normalizeKeyCap(this);
        ViewPolicy.setAllCapsFalse(this);
    }

    public void setGlyphSize(float dp) { sizeDp = dp; }

    @Override protected void onDraw(Canvas canvas) {
        drawPanelState(canvas);
        float size = Math.min(Math.min(getWidth(), getHeight()), DimensionPolicy.floatPixels(getContext(), sizeDp));
        int color = getCurrentTextColor();
        if (!isEnabled()) color = ColorPolicy.withAlpha(color, 96f / 255f);
        int saved = canvas.save();
        canvas.rotate(rotation, getWidth() / 2f, getHeight() / 2f);
        KeyboardIconPaths.draw(canvas, paint, icon, (getWidth() - size) / 2f,
            (getHeight() - size) / 2f, size, color);
        canvas.restoreToCount(saved);
    }
}

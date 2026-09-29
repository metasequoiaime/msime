package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.view.View;

/** Small deterministic keyboard miniature used by the in-keyboard skin picker. */
public final class KeyboardSkinPreview extends View {
    private KeyboardSkin skin;

    public KeyboardSkinPreview(Context context, KeyboardSkin skin) {
        super(context);
        this.skin = skin;
        setWillNotDraw(false);
    }

    public void setSkin(KeyboardSkin value) {
        skin = value;
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        drawPreview(canvas, new RectF(0, 0, getWidth(), getHeight()), skin,
            getResources().getDisplayMetrics().density);
    }

    public static void drawPreview(Canvas canvas, RectF bounds, KeyboardSkin skin,
                                   float density) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        KeyboardSkinBackgroundDrawable background =
            new KeyboardSkinBackgroundDrawable(skin, density);
        background.setBounds(Math.round(bounds.left), Math.round(bounds.top),
            Math.round(bounds.right), Math.round(bounds.bottom));
        background.draw(canvas);
        String[][] rows = {
            {"Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"},
            {"A", "S", "D", "F", "G", "H", "J", "K", "L"},
            {"⇧", "Z", "X", "C", "V", "B", "N", "M", "⌫"},
            {"123", "空格", "↵"}
        };
        float gap = Math.max(1, Math.min(bounds.width(), bounds.height()) * .035f);
        float rowHeight = (bounds.height() - gap * 3) / rows.length;
        Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        text.setTextAlign(Paint.Align.CENTER);
        text.setTypeface(skin.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        text.setTextSize(KeyboardGeometry.bounded(rowHeight * .42f, 7f, 14f * density));
        for (int rowIndex = 0; rowIndex < rows.length; rowIndex++) {
            String[] row = rows[rowIndex];
            float rowInset = rowIndex == 1 ? bounds.width() * .04f : 0;
            float keyWidth = (bounds.width() - rowInset * 2 - gap * (row.length - 1))
                / row.length;
            for (int index = 0; index < row.length; index++) {
                float left = bounds.left + rowInset + index * (keyWidth + gap);
                float top = bounds.top + rowIndex * (rowHeight + gap);
                RectF key = new RectF(left, top, left + keyWidth, top + rowHeight);
                boolean action = rowIndex == rows.length - 1 || index == 0 && rowIndex == 2;
                int fill = Color.parseColor(action ? skin.actionBackground() : skin.keyBackground());
                KeyboardSkinKeyDrawable drawable = new KeyboardSkinKeyDrawable(
                    skin, fill, action, density);
                drawable.setBounds(Math.round(key.left), Math.round(key.top),
                    Math.round(key.right), Math.round(key.bottom));
                drawable.draw(canvas);
                text.setColor(Color.parseColor(action ? skin.actionForeground() : skin.keyForeground()));
                Paint.FontMetrics metrics = text.getFontMetrics();
                float baseline = key.centerY() - (metrics.ascent + metrics.descent) / 2;
                canvas.drawText(row[index], key.centerX(), baseline, text);
            }
        }
    }
}

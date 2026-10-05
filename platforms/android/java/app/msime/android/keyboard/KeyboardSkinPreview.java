package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.view.View;

/**
 * Small deterministic keyboard miniature used by the in-keyboard skin picker.
 *
 * <p>{@link #drawTile} 是新皮肤面板瓷砖用的版本：圆角裁切的面板底色上画三行无字的小键帽和一个 accent 回车，瓷砖只有七八十 dp 宽，字已经看不清。{@link #drawPreview} 保留给仍需要带字缩略图的地方。
 */
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

    /** 瓷砖缩略图：裁成圆角，铺皮肤背景，画三行键帽（无字），最后一行右端是 accent 回车。 */
    public static void drawTile(Canvas canvas, RectF bounds, float radius, KeyboardSkin skin,
                                float density) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        int saved = canvas.save();
        android.graphics.Path clip = new android.graphics.Path();
        clip.addRoundRect(bounds, radius, radius, android.graphics.Path.Direction.CW);
        canvas.clipPath(clip);
        KeyboardSkinBackgroundDrawable background =
            new KeyboardSkinBackgroundDrawable(skin, density);
        background.setBounds(Math.round(bounds.left), Math.round(bounds.top),
            Math.round(bounds.right), Math.round(bounds.bottom));
        background.draw(canvas);
        int[] counts = tileRowCounts();
        float pad = bounds.width() * .1f;
        float gap = Math.max(1f, bounds.width() * .035f);
        float area = bounds.height() - pad * 2;
        float rowHeight = (area - gap * (counts.length - 1)) / counts.length;
        Paint key = new Paint(Paint.ANTI_ALIAS_FLAG);
        RectF rect = new RectF();
        float keyRadius = Math.max(1f, rowHeight * .22f);
        for (int row = 0; row < counts.length; row++) {
            int count = counts[row];
            float width = (bounds.width() - pad * 2 - gap * (count - 1)) / count;
            float top = bounds.top + pad + row * (rowHeight + gap);
            for (int index = 0; index < count; index++) {
                float left = bounds.left + pad + index * (width + gap);
                boolean enter = row == counts.length - 1 && index == count - 1;
                key.setColor(Color.parseColor(enter ? skin.accent() : skin.keyBackground()));
                rect.set(left, top, left + width, top + rowHeight);
                canvas.drawRoundRect(rect, keyRadius, keyRadius, key);
            }
        }
        canvas.restoreToCount(saved);
    }

    /** 「跟随系统」瓷砖：135° 对角线把浅色与深色对半分。 */
    public static void drawSplit(Canvas canvas, RectF bounds, float radius, int light, int dark) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        int saved = canvas.save();
        android.graphics.Path clip = new android.graphics.Path();
        clip.addRoundRect(bounds, radius, radius, android.graphics.Path.Direction.CW);
        canvas.clipPath(clip);
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        paint.setColor(light);
        canvas.drawRect(bounds, paint);
        android.graphics.Path half = new android.graphics.Path();
        half.moveTo(bounds.right, bounds.top);
        half.lineTo(bounds.right, bounds.bottom);
        half.lineTo(bounds.left, bounds.bottom);
        half.close();
        paint.setColor(dark);
        canvas.drawPath(half, paint);
        canvas.restoreToCount(saved);
    }

    /** 瓷砖缩略图每行的键数。 */
    public static int[] tileRowCounts() { return new int[] {5, 5, 3}; }
}
